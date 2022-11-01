use std::clone::Clone;
use std::fs::{copy, File, remove_dir_all, remove_file};
use std::io::{self, BufRead, BufReader, Write};
use std::sync::Weak;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::{Deserializer, Deserialize};
use serde_derive::Serialize;

use crate::project::binary::Binary;
use crate::project::definition::ProjectDefinition;
use crate::test::diff::diff_plaintext;
use crate::test::test::Diff;
use crate::testresult::ordio_testresult::OrdIoTestresult;
use crate::testresult::testresult::Testresult;
use crate::testrunner::{TestrunnerError, TestrunnerOptions};
use super::diff::ChangesetInline;
use super::io_test::{prepare_cmdline, prepare_envvars, prepare_valgrind, parse_vg_log};
use super::test::{Test, TestMeta, TestcaseType, TestingError};


#[derive(Clone, Debug)]
pub enum InputOutput {
    Input(String),
    Output(String),
}

pub enum IODiff {
    Input(String),
    InputUnsent(String),
    Output(Vec<ChangesetInline<String>>),
}

impl InputOutput {
    fn is_input(&self) -> bool {
        match self {
            InputOutput::Input(_) => true,
            InputOutput::Output(_) => false,
        }
    }

    fn is_output(&self) -> bool {
        match self {
            InputOutput::Input(_) => false,
            InputOutput::Output(_) => true,
        }
    }

    fn get_ref(&self) -> &String {
        match self {
            InputOutput::Input(s) => &s,
            InputOutput::Output(s) => &s,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OrdIoTest {
    #[serde(flatten)]
    meta: TestMeta,
    #[serde(skip)]
    project_definition: Weak<ProjectDefinition>,
    #[serde(skip)]
    options: Weak<TestrunnerOptions>,
    #[serde(skip)]
    binary: Weak<Binary>,
    #[serde(skip)]
    io: Vec<InputOutput>,
    #[serde(skip_serializing, deserialize_with = "OrdIoTest::deserialize_regex")]
    io_prompt: Regex,
    io_file: String,
    #[serde(default)]
    argv: Vec<String>,
    exp_retvar: Option<i32>,
    env_vars: Option<Vec<String>>,
}


impl Test for OrdIoTest {
    fn init(&mut self, number: i32, project_definition: Weak<ProjectDefinition>, options: Weak<TestrunnerOptions>, binary: Weak<Binary>) -> Result<(), TestrunnerError> {
        self.meta.number = number;
        self.project_definition = project_definition;
        self.options = options;
        self.binary = binary;
        self.io = OrdIoTest::parse_io_file(&self.io_file)?;
        Ok(())
    }

    fn get_test_meta(&self) -> &TestMeta { &self.meta }

    fn type_id(&self) -> &'static str {
        return "OrdIO";
    }

    fn deserialize_trait<'de, D: ?Sized>(deserializer: &mut dyn erased_serde::Deserializer<'de>) -> Result<Box<dyn Test + Send + Sync>, erased_serde::Error>
        where Self: Sized
    {
        Ok(Box::new(OrdIoTest::deserialize(deserializer)?))
    }

    fn run(&self) -> Result<Box<dyn Testresult + Send + Sync>, TestingError> {
        let options = self.options.upgrade().unwrap();
        let project_definition = self.project_definition.upgrade().unwrap();

        if options.protected_mode && self.meta.protected {
            println!("\nStarting testcase {}: ********", self.meta.number);
        }
        else {
            println!("\nStarting testcase {}: {}", self.meta.number, self.meta.name);
        }

        let basedir = project_definition.makefile_path.clone().unwrap_or(".".to_owned());
        let (vg_log_folder, vg_filepath) = prepare_valgrind(&project_definition, &options, &self.meta, &basedir);
        let (cmd_name, flags) = prepare_cmdline(&project_definition, &options, &vg_filepath)?;
        let env_vars = prepare_envvars(self.env_vars.as_ref());

        let global_timeout = project_definition.global_timeout.unwrap_or(5);
        let timeout = self.meta.timeout.unwrap_or(global_timeout);

        let starttime = Instant::now();
        let (mut io, retvar) = self.run_command_with_timeout(&cmd_name, &flags, &env_vars, timeout)?;
        let endtime = Instant::now();

        println!("Got output from testcase {}", self.meta.number);

        let had_timeout = !retvar.is_some();

        let mut truncated_output = false;
        let ref_output_len = match self.io.iter().rev().rfind(|io_e| io_e.is_output()) {
            Some(io_e) => {
                io_e.get_ref().chars().count()
            },
            None => { 256 },
        };
        if let Some(io_e) = io.iter_mut().rev().rfind(|io_e| io_e.is_output()) {
            match io_e {
                InputOutput::Output(ref mut out) => {
                    if out.chars().count() > ref_output_len * 2 {
                        println!("Truncating your output, because it's much longer than the reference output!");
                        out.truncate(out.char_indices().nth(ref_output_len * 2).unwrap_or((512, ' ')).0);
                        truncated_output = true;
                    }
                },
                _ => {},
            }
        }

        println!("Testcase took {:#?}", endtime.duration_since(starttime));

        // calc diff
        let mut len_ref_sum = 0;
        let mut distances = Vec::with_capacity(io.len() / 2 + 2);
        let mut io_mismatch = false;
        let mut it_ref_io = self.io.iter();
        let mut it_io = io.iter();
        let mut io_diff = Vec::<IODiff>::with_capacity(self.io.len());
        while let Some(ref_io_e) = it_ref_io.next() {
            let io_e = it_io.next();
            if io_e.is_some() && !((ref_io_e.is_input() && io_e.unwrap().is_input()) || (ref_io_e.is_output() && io_e.unwrap().is_output())) {
                io_mismatch = true;
            }

            let diff_e = match io_e {
                Some(io_e) => {
                    match ref_io_e {
                        InputOutput::Input(input) => IODiff::Input(input.to_string()),
                        InputOutput::Output(output) => {
                            len_ref_sum += output.len();
                            let (changeset, distance) = diff_plaintext(output, io_e.get_ref(), Duration::from_secs(timeout));
                            distances.push(distance * output.len() as f32);
                            IODiff::Output(changeset)
                        },
                    }
                },
                None => {
                    match ref_io_e {
                        InputOutput::Input(input) => IODiff::InputUnsent(input.to_string()),
                        InputOutput::Output(output) => {
                            len_ref_sum += output.len();
                            let (changeset, distance) = diff_plaintext(output, "", Duration::from_secs(timeout));
                            distances.push(distance * output.len() as f32);
                            IODiff::Output(changeset)
                        },
                    }
                },
            };
            io_diff.push(diff_e);
        }
        if io_mismatch {
            return Err(TestingError::IOMismatch);
        }
        let distance = distances.iter().sum::<f32>() / len_ref_sum as f32;

        let add_file_missing;
        let add_diff = match self.get_add_diff() {
            Ok(ok) => {
                add_file_missing = false;
                ok
            },
            Err(TestingError::OutFileNotFound(_)) => {
                add_file_missing = true;
                None
            },
            Err(e) => return Err(e),
        };

        let add_distance: f32;
        if let Some(ref diff) = add_diff {
            match diff {
                Diff::PlainText(_, d) => add_distance = *d,
                Diff::Binary(_, d) => add_distance = *d,
            }
        }
        else {
            add_distance = 1.0;
        }
        let passed: bool = self.exp_retvar.is_some() && retvar.is_some() && retvar.unwrap() == self.exp_retvar.unwrap()
            && distance == 1.0 && add_distance == 1.0 && !had_timeout;

        let input = self.io.iter().map(|e| {
            match e {
                InputOutput::Input(input) => input.clone(),
                _ => "".to_owned(),
            }
        }).collect::<Vec<String>>().join("");

        let mem_leaks;
        let mem_errors;
        if project_definition.use_valgrind.unwrap_or(true) {
            if cfg!(unix) && options.sudo.is_some() {
                match copy(&vg_filepath, format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &self.meta.number)) {
                    Ok(_) => remove_file(&vg_filepath),
                    Err(_) => Ok(()),
                };
            }
            let valgrind = parse_vg_log(&format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &self.meta.number))?;
            println!("Memory usage errors: {:?}\nMemory leaks: {:?}", valgrind.1, valgrind.0);

            if cfg!(unix) && options.sudo.is_some() && self.meta.protected {
                remove_dir_all(&format!("{}/{}/{}", &basedir, &vg_log_folder, &self.meta.number)).unwrap_or(());
            }

            mem_leaks = Some(valgrind.0);
            mem_errors = Some(valgrind.1);
        }
        else {
            mem_leaks = None;
            mem_errors = None;
        }

        if options.protected_mode && self.meta.protected {
            println!("Finished testcase {}: ********", self.meta.number);
        }
        else {
            println!("Finished testcase {}: {}", self.meta.number, self.meta.name);
        }


        Ok(Box::new(OrdIoTestresult {
            io_diff,
            diff_distance: distance,
            add_distance: if add_diff.is_some() { Some(add_distance) } else { None },
            add_diff,
            add_file_missing,
            truncated_output,
            passed,
            ret: retvar,
            exp_ret: self.exp_retvar,
            mem_leaks,
            mem_errors,
            mem_logfile: vg_filepath,
            command_used: format!("./{} {}", &project_definition.binary_path, &self.argv.clone().join(" ")),
            input,
            timeout: had_timeout,
            name: self.meta.name.clone(),
            description: self.meta.description.clone().unwrap_or("".to_owned()),
            number: self.meta.number,
            kind: TestcaseType::OrdIOTest,
            protected: self.meta.protected,
            options: self.options.clone(),
            project_definition: self.project_definition.clone(),
        }))
    }
}

impl OrdIoTest {

    fn deserialize_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
        where D: Deserializer<'de>
    {
        return Ok(Regex::new(&format!("(?m){}", &String::deserialize(deserializer)?)).unwrap());
    }

    fn parse_io_file(path: &str) -> Result<Vec<InputOutput>, TestingError> {
        let file = File::open(&path).map_err(|_| TestingError::IoConfigNotFound(path.to_string()))?;
        let reader = BufReader::new(file);

        Ok(reader.lines().fold(Vec::<InputOutput>::new(), |mut acc, e| {
            if let Ok(e) = e {
                let curr_io: InputOutput;

                if e.starts_with("> ") {
                    curr_io = InputOutput::Output(format!("{}\n", e.strip_prefix("> ").unwrap()));
                }
                else if e.starts_with("? ") {
                    curr_io = InputOutput::Output(format!("{}", e.strip_prefix("? ").unwrap()));
                }
                else if e.starts_with("< ") {
                    curr_io = InputOutput::Input(format!("{}\n", e.strip_prefix("< ").unwrap()));
                }
                else if e.starts_with("#") {
                    return acc;
                }
                else {
                    eprintln!("Ignoring line with invalid prefix in file: {}", &path);
                    return acc;
                }

                if let Some(prev_io) = acc.last_mut() {
                    match curr_io {
                        InputOutput::Output(ref curr_e) => {
                            if let InputOutput::Output(prev_e) = prev_io {
                                prev_e.push_str(&curr_e);
                            }
                            else {
                                acc.push(curr_io);
                            }
                        },
                        InputOutput::Input(ref curr_e) => {
                            if let InputOutput::Input(prev_e) = prev_io {
                                prev_e.push_str(&curr_e);
                            }
                            else {
                                acc.push(curr_io);
                            }
                        }
                    }
                }
                else {
                    if curr_io.is_input() {
                        acc.push(InputOutput::Output("".to_owned()));
                    }
                    acc.push(curr_io);
                }
            }
            acc
        }))
    }

    fn run_command_with_timeout(&self, command: &str, args: &Vec<String>, envs: &Vec<(String, String)>, timeout: u64)-> Result<(Vec<InputOutput>, Option<i32>), io::Error> {
        let project_definition = self.project_definition.upgrade().unwrap();

        let timeout = Duration::from_secs(timeout);
        let mut has_finished = false;
        let mut ref_io = self.io.iter();
        let mut io: Vec<InputOutput> = Vec::with_capacity(self.io.len());

        let mut cmd = subprocess::Exec::cmd(command)
            .cwd(project_definition.makefile_path.as_ref().unwrap_or(&"./".to_owned()))
            .args(args)
            .args(&self.argv)
            .stdin(subprocess::Redirection::Pipe)
            .stdout(subprocess::Redirection::Pipe)
            .stderr(subprocess::NullFile)
            .env_extend(envs)
            .popen()
            .expect("Could not spawn process!");

        let mut stdin = cmd.stdin.as_ref().unwrap().try_clone().unwrap();
        let mut curr_io = ref_io.next().unwrap().clone();

        let mut communicator = cmd.communicate_start(Some("".as_bytes().iter().cloned().collect()))
            .limit_time(Duration::from_millis(250));

        // check for some initial unexpected output
        if curr_io.get_ref().is_empty() {
            let result = communicator.read();
            match result {
                Ok(comm) => {
                    io.push(InputOutput::Output(String::from_utf8_lossy(&comm.0.unwrap_or(vec![])).to_string()));
                },
                Err(err) => {
                    io.push(InputOutput::Output(String::from_utf8_lossy(&err.capture.0.unwrap_or(vec![])).to_string()));
                }
            }
            curr_io = ref_io.next().unwrap().clone();
        }

        let starttime = Instant::now();

        // continiously write input and read output
        let mut retvar = None;
        'io_loop: loop {
            match &curr_io {
                InputOutput::Input(input) => {
                    stdin.write(&input.as_bytes())?;
                    stdin.flush()?;
                },
                InputOutput::Output(_) => {
                    let mut output = String::with_capacity(self.io.iter().filter(|e| e.is_output()).fold(0, |acc, e| acc + e.get_ref().len()));
                    loop {
                        let result = communicator.read();
                        match result {
                            Ok(comm) => {
                                output.push_str(&String::from_utf8_lossy(&comm.0.unwrap_or(vec![])));
                            },
                            Err(err) => {
                                output.push_str(&String::from_utf8_lossy(&err.capture.0.clone().unwrap_or(vec![])));
                                if err.kind() != io::ErrorKind::TimedOut {
                                    break;
                                }
                            }
                        }

                        let currtime = Instant::now();
                        if currtime - starttime > timeout {
                            println!("Testcase {} ran into a timeout!", self.meta.number);
                            let given_retvar = match cmd.wait_timeout(std::time::Duration::new(2, 0)).expect("could not wait on process!") {
                                Some(retvar) => Some(retvar),
                                None => {
                                    println!("Testcase {} is still running, killing testcase!", self.meta.number);
                                    cmd.kill().expect("Could not kill testcase!");
                                    if cmd.wait_timeout(std::time::Duration::new(2, 0)).expect("Could not wait on process!").is_none() {
                                        println!("Testcase {} is still running, failed to kill testcase! Moving on regardless...", self.meta.number);
                                    }
                                    None
                                }
                            };
                            if retvar.is_none() {
                                retvar = given_retvar;
                            }

                            println!("Possibly failed capturing some/all output!");
                            io.push(InputOutput::Output(output));
                            has_finished = true;
                            break 'io_loop;
                        }

                        retvar = cmd.poll();
                        if retvar.is_some() {
                            // check for some final output
                            let result = communicator.read();
                            match result {
                                Ok(comm) => {
                                    output.push_str(&String::from_utf8_lossy(&comm.0.unwrap_or(vec![])));
                                },
                                Err(err) => {
                                    output.push_str(&String::from_utf8_lossy(&err.capture.0.clone().unwrap_or(vec![])));
                                    if err.kind() != io::ErrorKind::TimedOut {
                                        break;
                                    }
                                }
                            }

                            io.push(InputOutput::Output(output));
                            has_finished = true;
                            break 'io_loop;
                        }

                        if self.io_prompt.is_match(&output) {
                            break;
                        }
                    }
                    io.push(InputOutput::Output(output));
                }
            }
            if curr_io.is_input() {
                io.push(curr_io);
            }

            curr_io = match ref_io.next() {
                Some(e) => e.clone(),
                None => break,
            };
        }

        let currtime = Instant::now();
        if currtime - starttime > timeout {
            println!("Testcase {} ran into a timeout!", self.meta.number);
            retvar = None;
        }

        // check for some final output
        if !has_finished {
            let given_retvar;
            let given_output;

            drop(stdin);
            let capture = communicator.read();

            let currtime = Instant::now();
            if currtime - starttime > timeout {
                println!("Testcase {} ran into a timeout!", self.meta.number);
            }

            match capture {
                Ok(c) => {
                    given_retvar = match cmd.wait_timeout(std::time::Duration::new(2, 0)).expect("Could not wait on process!") {
                        Some(retvar) => Some(retvar),
                        None => {
                            println!("Testcase {} is still running, killing testcase!", self.meta.number);
                            cmd.kill().expect("Could not kill testcase!");
                            if cmd.wait_timeout(std::time::Duration::new(2, 0)).expect("Could not wait on process!").is_none() {
                                println!("Testcase {} is still running, failed to kill testcase! Moving on regardless...", self.meta.number);
                            }
                            None
                        }
                    };

                    given_output = format!("{}", String::from_utf8_lossy(&c.0.unwrap_or(Vec::new())));
                }

                Err(e) => {
                    given_retvar = match cmd.wait_timeout(std::time::Duration::new(2, 0)).expect("could not wait on process!") {
                        Some(retvar) => Some(retvar),
                        None => {
                            println!("Testcase {} is still running, killing testcase!", self.meta.number);
                            cmd.kill().expect("Could not kill testcase!");
                            if cmd.wait_timeout(std::time::Duration::new(2, 0)).expect("Could not wait on process!").is_none() {
                                println!("Testcase {} is still running, failed to kill testcase! Moving on regardless...", self.meta.number);
                            }
                            None
                        }
                    };

                    println!("Possibly failed capturing some/all output!");
                    given_output = format!("{}", String::from_utf8_lossy(&e.capture.0.unwrap_or(Vec::new())));
                }
            }

            if retvar.is_none() {
                retvar = given_retvar;
            }

            if let Some(prev_io) = io.last_mut() {
                match prev_io {
                    InputOutput::Output(ref mut prev_e) => {
                        prev_e.push_str(&given_output)
                    },
                    _ => {
                        io.push(InputOutput::Output(given_output));
                    }
                }
            }
        }

        let retvar = match retvar {
            Some(v) => match v {
                subprocess::ExitStatus::Exited(retvar) => Some(retvar as i32),
                subprocess::ExitStatus::Other(retvar) => Some(retvar),
                _ => None,
            },
            None => None,
        };

        Ok((io, retvar))
    }
}

