use std::clone::Clone;
use std::fs::{copy, File, remove_file};
use std::io::{BufRead, BufReader, Read, self, Write};
use std::time::{Duration, Instant};
use difference::{Changeset, Difference};
use popol::{Events, Sources};
use regex::Regex;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use super::io_test::{prepare_cmdline, prepare_envvars, prepare_valgrind, parse_vg_log, percentage_from_levenstein};
use super::test::{DiffKind, Test, TestCaseKind, TestMeta};
use super::testcase::Testcase;
use super::testresult::TestResult;
use crate::project::binary::{Binary, GenerationError};
use crate::project::definition::ProjectDefinition;


#[derive(Clone)]
pub enum InputOutput {
    Input(String),
    InputFlush(String),
    Output(String),
}

pub enum IODiff {
    Input(String),
    InputFlush(String),
    Output(Changeset),
}

impl InputOutput {
    fn is_input(&self) -> bool {
        match self {
            InputOutput::Input(_) => true,
            InputOutput::InputFlush(_) => true,
            InputOutput::Output(_) => false,
        }
    }

    fn is_output(&self) -> bool {
        match self {
            InputOutput::Input(_) => false,
            InputOutput::InputFlush(_) => false,
            InputOutput::Output(_) => true,
        }
    }

    fn unwrap(self) -> String {
        match self {
            InputOutput::Input(s) => s,
            InputOutput::InputFlush(s) => s,
            InputOutput::Output(s) => s,
        }
    }
}

pub struct OrdIoTest {
    meta: TestMeta,
    binary: Binary,
    io_file: String,
    io: Vec<InputOutput>,
    io_prompt: Regex,
    argv: Vec<String>,
    exp_retvar: Option<i32>,
    env_vars: Option<String>,
}


impl Test for OrdIoTest {
    fn get_test_meta(&self) -> &TestMeta { &self.meta }

    fn from_saved_tc(
        number: i32,
        testcase: &Testcase,
        projdef: &ProjectDefinition,
        binary: Option<&Binary>,
    ) -> Result<Self, GenerationError> {
        match binary {
            Some(_) => {}
            None => {
                return Err(GenerationError::BinaryRequired);
            }
        };
        let add_diff_kind = match &testcase.add_diff_mode {
            Some(text) => {
                if text.eq_ignore_ascii_case("binary") {
                    DiffKind::Binary
                }
                else {
                    DiffKind::PlainText
                }
            },
            None => DiffKind::PlainText,
        };
        let meta = TestMeta {
            kind: TestCaseKind::IOTest,
            add_diff_kind,
            add_out_file: testcase.add_out_file.clone(),
            add_exp_file: testcase.add_exp_file.clone(),
            number,
            name: testcase.name.clone(),
            desc: testcase.description.clone(),
            projdef: projdef.clone(),
            timeout: testcase.timeout,
            protected: testcase.protected.unwrap_or(false),
        };

        let test = OrdIoTest {
            meta,
            binary: binary.unwrap().clone(),
            exp_retvar: testcase.exp_retvar,
            argv: testcase.args.as_ref().unwrap_or(&vec![String::new()]).clone(),
            env_vars: testcase.env_vars.clone(),
            io_file: testcase.io_file.as_ref().unwrap_or(&String::new()).clone(),
            io: OrdIoTest::parse_io_file(testcase.io_file.as_ref().unwrap())?,
            io_prompt: Regex::new(testcase.io_prompt.as_ref().unwrap()).unwrap(),
        };
        Ok(test)
    }

    fn run(&self) -> Result<TestResult, GenerationError> {
        if self.meta.projdef.protected_mode && self.meta.protected {
            println!("\nStarting testcase {}: ********", self.meta.number);
        }
        else {
            println!("\nStarting testcase {}: {}", self.meta.number, self.meta.name);
        }

        let basedir = self.meta.projdef.makefile_path.clone().unwrap_or(String::from("."));
        let (vg_log_folder, vg_filepath) = prepare_valgrind(&self.meta, &basedir);
        let (cmd_name, flags) = prepare_cmdline(&self.meta, &vg_filepath)?;
        let env_vars = prepare_envvars(self.env_vars.as_ref());

        let global_timeout = self.meta.projdef.global_timeout.unwrap_or(5);
        let timeout = self.meta.timeout.unwrap_or(global_timeout);

        let starttime = Instant::now();
        let (mut io, retvar) = match self.run_command_with_timeout(&cmd_name, &flags, &env_vars, timeout) {
            Ok((io, retvar)) => (io, retvar),
            Err(err) => {
                eprintln!("Error talking to executable:\n{:?}", err);
                return Err(GenerationError::ConfigErrorIO);
            }
        };
        let endtime = Instant::now();

        println!("Got output from testcase {}", self.meta.number);

        let had_timeout = !retvar.is_some();

        let ref_output_len = match self.io.iter().rev().rfind(|io_e| io_e.is_output()) {
            Some(io_e) => {
                io_e.clone().unwrap().len()
            },
            None => { 256 },
        };
        if let Some(io_e) = io.iter_mut().rev().rfind(|io_e| io_e.is_output()) {
            match io_e {
                InputOutput::Output(ref mut out) => {
                    if out.len() > ref_output_len * 2 {
                        println!("Reducing your output length because its bigger than 2 * reference output");
                        out.truncate(ref_output_len * 2);
                    }
                },
                _ => {},
            }
        }

        println!("Testcase took {:#?}", endtime.duration_since(starttime));

        let mut len_ref_sum = 0;
        let mut len_user_sum = 0;
        let mut distances = Vec::with_capacity(io.len() / 2 + 2);
        let mut io_mismatch = false;
        let io_diff: Vec<IODiff> = self.io.iter().zip(io.iter()).map(|e| {
            io_mismatch = true;

            match e.0 {
                InputOutput::Input(input) => IODiff::Input(input.to_string()),
                InputOutput::InputFlush(input) => IODiff::InputFlush(input.to_string()),
                InputOutput::Output(output) => {
                    len_ref_sum += output.len();
                    len_user_sum += e.1.clone().unwrap().len();
                    let changeset = Changeset::new(output, &e.1.clone().unwrap(), &self.meta.projdef.diff_delim);
                    distances.push(changeset.distance.abs() * output.len() as i32);
                    IODiff::Output(changeset)
                }
            }
        }).collect();
        if io_mismatch {
            return Err(GenerationError::IOMismatch);
        }
        let distance: i32 = distances.iter().sum::<i32>() / len_ref_sum as i32;

        let add_diff = self.get_add_diff();
        let passed: bool = self.exp_retvar.is_some() && retvar.is_some() && retvar.unwrap() == self.exp_retvar.unwrap()
            && distance == 0 && add_diff.as_ref().unwrap_or(&(String::from(""), 0, 0.0)).1 == 0 && !had_timeout; //TODO check if there are not diffs

        let input = self.io.iter().map(|e| {
            match e {
                InputOutput::Input(input) => input.clone(),
                InputOutput::InputFlush(input) => input.clone(),
                _ => "".to_string(),
            }
        }).collect::<Vec<String>>().join("");

        let output = io.iter().map(|e| {
            match e {
                InputOutput::Output(output) => output.clone(),
                _ => "".to_string(),
            }
        }).collect::<Vec<String>>().join("");

        if self.meta.projdef.verbose && distance > 0
        {
            println!("Diff-Distance: {:?}", distance);
            println!("------ START Reference ------");
            println!("Reference Output:\n");
            self.io.iter().for_each(|e| {
                match e {
                    InputOutput::Output(output) => println!("{}", output),
                    _ => (),
                }
            });
            println!("------ END Reference ------");
            println!("------ START Yours ------");
            println!("Your Output:\n{:?}", output);
            println!("------ END Yours ------");

            // prints diff with colors to terminal
            // green = ok
            // blue = reference (our solution)
            // red = wrong (students solution) / too much
            let mut colored_stdout = StandardStream::stdout(ColorChoice::Always);
            io_diff.iter().for_each(|e| {
                match e {
                    IODiff::Output(cs) => {
                        for c in &cs.diffs
                        {
                            match c
                            {
                                Difference::Same(ref z)=>
                                {
                                    colored_stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green))).unwrap();
                                    writeln!(&mut colored_stdout, "{}", String::from(z) ).unwrap();
                                }
                                Difference::Rem(ref z) =>
                                {
                                    colored_stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue))).unwrap();
                                    writeln!(&mut colored_stdout, "{}", String::from(z)  ).unwrap();
                                }

                                Difference::Add(ref z) =>
                                {
                                    colored_stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red))).unwrap();
                                    writeln!(&mut colored_stdout, "{}", String::from(z)  ).unwrap();
                                }

                            }
                        }
                    },
                    _ => (),
                }
            });
            colored_stdout.reset().unwrap();
        }

        if cfg!(unix) && self.meta.projdef.sudo.is_some() {
            match copy(&vg_filepath, format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &self.meta.number)) {
                Ok(_) => remove_file(&vg_filepath).unwrap_or(()),
                Err(_) => (),
            }
        }
        let valgrind = parse_vg_log(&format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &self.meta.number)).unwrap_or((-1, -1));
        println!("Memory usage errors: {:?}\nMemory leaks: {:?}", valgrind.1, valgrind.0);

        if self.meta.projdef.protected_mode && self.meta.protected {
            println!("Finished testcase {}: ********", self.meta.number);
        }
        else {
            println!("Finished testcase {}: {}", self.meta.number, self.meta.name);
        }


        Ok(TestResult {
            diff: None,
            io_diff: Some(io_diff),
            add_distance_percentage: match &add_diff { Some(d) => Some(d.2), None => None },
            add_diff: match add_diff { Some(d) => Some(d.0), None => None },
            implemented: None,
            passed,
            output,
            ret: retvar,
            exp_ret: self.exp_retvar,
            mem_leaks: valgrind.0,
            mem_errors: valgrind.1,
            mem_logfile: vg_filepath,
            command_used: String::from(format!("./{} {}", &self.meta.projdef.binary_path, &self.argv.clone().join(" "))),
            input,
            timeout: had_timeout,
            name: self.meta.name.clone(),
            description: self.meta.desc.clone().unwrap_or(String::from("")),
            number: self.meta.number,
            kind: self.meta.kind,
            distance_percentage: Some(percentage_from_levenstein(
                    distance,
                    len_ref_sum,
                    len_user_sum,
            )),
            protected: self.meta.protected,
        })
    }
}

impl OrdIoTest {
    fn parse_io_file(path: &str) -> Result<Vec<InputOutput>, GenerationError> {
        let file = File::open(&path).map_err(|_| GenerationError::ConfigErrorIO)?;
        let reader = BufReader::new(file);

        Ok(reader.lines().fold(Vec::<InputOutput>::new(), |mut acc, e| {
            if let Ok(mut e) = e {
                let curr_io: InputOutput;

                if e.starts_with("> ") {
                    e.strip_prefix("> ").unwrap();
                    e.push('\n');
                    curr_io = InputOutput::Output(e);
                }
                else if e.starts_with("? ") {
                    e.strip_prefix("? ").unwrap();
                    curr_io = InputOutput::Output(e);
                }
                else if e.starts_with("< ") {
                    e.strip_prefix("< ").unwrap();
                    e.push('\n');
                    curr_io = InputOutput::Input(e);
                }
                else if e.starts_with("! ") {
                    e.strip_prefix("! ").unwrap();
                    curr_io = InputOutput::InputFlush(e);
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
                        InputOutput::InputFlush(_) => {
                            acc.push(curr_io);
                        }
                    }
                }
                else {
                    if curr_io.is_input() {
                        acc.push(InputOutput::Output("".to_string()));
                    }
                    acc.push(curr_io);
                }
            }
            acc
        }))
    }

    fn run_command_with_timeout(&self, command: &str, args: &Vec<String>, envs: &Vec<(String, String)>, timeout: u64)-> Result<(Vec<InputOutput>, Option<i32>), io::Error> {
        let timeout = Duration::from_secs(timeout);
        let mut got_timeout = false;
        let mut ref_io = self.io.iter();
        let mut io: Vec<InputOutput> = Vec::with_capacity(self.io.len());

        let mut sources = Sources::with_capacity(1);
        let mut events = Events::new();

        let mut cmd = subprocess::Exec::cmd(command)
            .cwd(self.meta.projdef.makefile_path.as_ref().unwrap_or(&String::from("./")))
            .args(args)
            .args(&self.argv)
            .stdin(subprocess::Redirection::Pipe)
            .stdout(subprocess::Redirection::Pipe)
            .stderr(subprocess::NullFile)
            .env_extend(envs)
            .popen()
            .expect("Could not spawn process!");

        let mut stdin = cmd.stdin.as_ref().unwrap().try_clone().unwrap();
        let mut stdout = cmd.stdout.as_ref().unwrap().try_clone().unwrap();
        let mut curr_io = ref_io.next().unwrap().clone();

        sources.register((), &cmd.stdout.as_ref().unwrap().try_clone().unwrap(), popol::interest::READ);

        // check for some initial unexpected output
        if curr_io.clone().unwrap().is_empty() {
            match sources.wait_timeout(&mut events, Duration::from_millis(250)) {
                Ok(()) => {
                    let mut buffer = [0; 1024];
                    stdout.read(&mut buffer[..])?;
                    io.push(InputOutput::Output(String::from_utf8_lossy(&buffer).to_string()));
                },
                Err(err) => {
                    if err.kind() == io::ErrorKind::TimedOut {
                        io.push(InputOutput::Output("".to_string()));
                    }
                    else {
                        return Err(err)
                    }
                },
            }
            curr_io = ref_io.next().unwrap().clone();
        }

        let starttime = Instant::now();

        // continiously write input and read output
        'io_loop: loop {
            match &curr_io {
                InputOutput::Input(input) => {
                    stdin.write(&input.as_bytes())?;
                    stdin.flush()?;
                },
                InputOutput::InputFlush(input) => {
                    stdin.write(&input.as_bytes())?;
                    stdin.flush()?;
                    stdin.flush()?;
                },
                InputOutput::Output(_) => {
                    let mut output;
                    loop {
                        output = String::from("");
                        match sources.wait_timeout(&mut events, Duration::from_millis(250)) {
                            Ok(()) => {
                                {}
                            },
                            Err(err) => {
                                if err.kind() != io::ErrorKind::TimedOut {
                                    return Err(err)
                                }
                            },
                        }

                        for ((), event) in events.iter() {
                            if event.errored {
                                return Err(io::Error::new(io::ErrorKind::Other, "event.errored"));
                            }

                            if event.readable || event.hangup {
                                let mut buffer = [0; 1024];
                                stdout.read(&mut buffer[..])?;
                                output.push_str(&String::from_utf8_lossy(&buffer));
                            }
                        }

                        let currtime = Instant::now();
                        if currtime - starttime > timeout {
                            io.push(InputOutput::Output(output));
                            got_timeout = true;
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

        // check for some final output
        let mut retvar = None;
        if !got_timeout {
            let given_retvar;
            let given_output;

            let capture = cmd.communicate_start(None)
                .limit_time(Duration::from_millis(250))
                .read();

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
                    if e.kind() == io::ErrorKind::TimedOut {
                        println!("Testcase {} ran into a timeout!", self.meta.number);
                    }

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

            retvar = match given_retvar {
                Some(v) => match v {
                    subprocess::ExitStatus::Exited(retvar) => Some(retvar as i32),
                    subprocess::ExitStatus::Other(retvar) => Some(retvar),
                    _ => None,
                }
                None => None,
            };

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

        Ok((io, retvar))
    }
}
