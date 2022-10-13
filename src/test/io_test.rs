use std::fs::{copy, create_dir_all, Permissions, read_to_string, remove_dir_all, remove_file, set_permissions};
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::Weak;
use std::time::{Instant, Duration};

use regex::Regex;
use serde::Deserialize;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

use crate::project::binary::Binary;
use crate::project::definition::ProjectDefinition;
use crate::test::test::Diff;
use crate::testresult::io_testresult::IoTestresult;
use crate::testresult::testresult::Testresult;
use crate::testrunner::{TestrunnerError, TestrunnerOptions};
use super::test::{Test, TestMeta, TestcaseType, TestingError};
use super::diff::diff_plaintext;


#[derive(Debug, Deserialize, Serialize)]
pub struct IoTest {
    #[serde(flatten)]
    meta: TestMeta,
    #[serde(skip)]
    project_definition: Weak<ProjectDefinition>,
    #[serde(skip)]
    options: Weak<TestrunnerOptions>,
    #[serde(skip)]
    binary: Weak<Binary>,
    #[serde(default)]
    in_file: String,
    #[serde(default)]
    exp_file: String,
    #[serde(default)]
    in_string: String,
    #[serde(default)]
    exp_string: String,
    #[serde(default)]
    argv: Vec<String>,
    exp_retvar: Option<i32>,
    env_vars: Option<Vec<String>>,
}


impl Test for IoTest {
    fn init(&mut self, number: i32, project_definition: Weak<ProjectDefinition>, options: Weak<TestrunnerOptions>, binary: Weak<Binary>) -> Result<(), TestrunnerError> {
        self.meta.number = number;
        self.project_definition = project_definition;
        self.options = options;
        self.binary = binary;
        Ok(())
    }

    fn get_test_meta(&self) -> &TestMeta { &self.meta }

    fn type_id(&self) -> &'static str {
        return "IO";
    }

    fn deserialize_trait<'de, D: ?Sized>(deserializer: &mut dyn erased_serde::Deserializer<'de>) -> Result<Box<dyn Test + Send + Sync>, erased_serde::Error>
        where Self: Sized
    {
        Ok(Box::new(IoTest::deserialize(deserializer)?))
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
        let (input, reference_output, mut given_output, retvar) = self.run_command_with_timeout(&cmd_name, &flags, &env_vars, timeout);
        let endtime = Instant::now();

        println!("Got output from testcase {}", self.meta.number);

        let had_timeout = !retvar.is_some();

        let truncated_output;
        if given_output.chars().count() > reference_output.chars().count() * 2 {
            println!("Truncating your output, because it's much longer than the reference output!");
            given_output.truncate(given_output.char_indices().nth(reference_output.chars().count() * 2).unwrap_or((512, ' ')).0);
            truncated_output = true;
        }
        else {
            truncated_output = false;
        }

        println!("Testcase took {:#?}", endtime.duration_since(starttime));

        // calc diff
        let (changeset, distance) = diff_plaintext(&reference_output, &given_output, Duration::from_secs(timeout));

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

        if cfg!(unix) && options.sudo.is_some() {
            match copy(&vg_filepath, format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &self.meta.number)) {
                Ok(_) => remove_file(&vg_filepath).unwrap_or(()),
                Err(_) => (),
            }
        }
        let valgrind = parse_vg_log(&format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &self.meta.number)).unwrap_or((-1, -1));
        println!("Memory usage errors: {:?}\nMemory leaks: {:?}", valgrind.1, valgrind.0);

        if cfg!(unix) && options.sudo.is_some() && self.meta.protected {
            remove_dir_all(&format!("{}/{}/{}", &basedir, &vg_log_folder, &self.meta.number)).unwrap_or(());
        }
        let vg_filepath = format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &self.meta.number);

        if options.protected_mode && self.meta.protected {
            println!("Finished testcase {}: ********", self.meta.number);
        }
        else {
            println!("Finished testcase {}: {}", self.meta.number, self.meta.name);
        }


        Ok(Box::new(IoTestresult {
            diff: changeset,
            diff_distance: distance,
            add_distance: if add_diff.is_some() { Some(add_distance) } else { None },
            add_diff,
            add_file_missing,
            truncated_output,
            passed,
            ret: retvar,
            exp_ret: self.exp_retvar,
            mem_leaks: valgrind.0,
            mem_errors: valgrind.1,
            mem_logfile: vg_filepath,
            command_used: format!("./{} {}", &project_definition.binary_path, &self.argv.clone().join(" ")),
            input,
            timeout: had_timeout,
            name: self.meta.name.clone(),
            description: self.meta.description.clone().unwrap_or("".to_owned()),
            number: self.meta.number,
            kind: TestcaseType::IOTest,
            protected: self.meta.protected,
            options: self.options.clone(),
            project_definition: self.project_definition.clone(),
        }))
    }
}

impl IoTest {
    fn run_command_with_timeout(&self, command : &str, args: &Vec<String>, envs: &Vec<(String, String)>, timeout : u64) -> (String, String, String, Option<i32>) {
        let project_definition = self.project_definition.upgrade().unwrap();

        let mut input: String = String::new();
        if !self.in_file.is_empty() {
            match read_to_string(&self.in_file) {
                Ok(content) => {
                    input.clone_from(&content);
                }
                Err(err) => {
                    eprintln!("Cannot open stdinfile, fallback to none \n{:?}", err);
                }
            }
        }
        else if !self.in_string.is_empty() {
            input.clone_from(&self.in_string);
        }

        let mut reference_output: String = String::new();
        if !self.exp_file.is_empty() {
            match read_to_string(&self.exp_file) {
                Ok(content) => {
                    reference_output = content;
                }
                Err(err) => {
                    eprintln!("Cannot open stdout, fallback to none \n{:?}", err);
                }
            }
        }
        else if !self.exp_string.is_empty() {
            reference_output.clone_from(&self.exp_string);
        }

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


        let given_retvar;
        let given_output;

        let capture = cmd.communicate_start(Some(input.as_bytes().iter().cloned().collect()))
            .limit_time(std::time::Duration::new(timeout , 0))
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

        let given_retvar = match given_retvar {
            Some(v) => match v {
                subprocess::ExitStatus::Exited(retvar) => Some(retvar as i32),
                subprocess::ExitStatus::Other(retvar) => Some(retvar),
                _ => None,
            }
            None => None,
        };

        return (input, reference_output, given_output, given_retvar);
    }
}

pub fn prepare_valgrind(project_definition: &ProjectDefinition, options: &TestrunnerOptions, meta: &TestMeta, basedir: &str) -> (String, String) {
    let vg_log_folder = project_definition.valgrind_log_folder.clone().unwrap_or("valgrind_logs".to_owned());
    let vg_filepath = if cfg!(unix) && options.sudo.is_some() {
        format!("{}/testrunner-{}", std::env::temp_dir().to_str().unwrap(), Uuid::new_v4().to_simple().to_string())
    } else {
        format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, meta.number)
    };

    if project_definition.use_valgrind.unwrap_or(true) {
        create_dir_all(format!("{}/{}/{}", &basedir, &vg_log_folder, &meta.number)).expect("could not create valgrind_log folder");
        #[cfg(unix)] {
            set_permissions(format!("{}/{}", &basedir, &vg_log_folder), Permissions::from_mode(0o750)).unwrap();
            set_permissions(format!("{}/{}/{}", &basedir, &vg_log_folder, &meta.number), Permissions::from_mode(0o750)).unwrap();
        }
    }

    (vg_log_folder, vg_filepath)
}

pub fn prepare_cmdline(project_definition: &ProjectDefinition, options: &TestrunnerOptions, vg_filepath: &str) -> Result<(String, Vec<String>), TestingError> {
    let cmd_name = if options.sudo.is_some() && cfg!(unix) {
        "sudo".to_owned()
    } else if project_definition.use_valgrind.unwrap_or(true) {
        "valgrind".to_owned()
    } else {
        format!("./{}", &project_definition.binary_path)
    };

    let mut flags = Vec::<String>::new();
    if options.sudo.is_some() {
        check_program_availability("sudo")?;
        flags.push("--preserve-env".to_owned());
        flags.push(format!("--user={}", &options.sudo.as_ref().unwrap()));
        if project_definition.use_valgrind.unwrap_or(true) {
            flags.push("valgrind".to_owned());
        }
    }
    if project_definition.use_valgrind.unwrap_or(true) {
        check_program_availability("valgrind")?;
        if let Some(v) = &project_definition.valgrind_flags {
            flags.append(&mut v.clone());
        }
        else {
            flags.push("--leak-check=full".to_owned());
            flags.push("--show-leak-kinds=all".to_owned());
            flags.push("--track-origins=yes".to_owned());
        }
        flags.push(format!("--log-file={}", &vg_filepath ));
        flags.push(format!("./{}", &project_definition.binary_path));
    }

    Ok((cmd_name, flags))
}

pub fn prepare_envvars(env_vars: Option<&Vec<String>>) -> Vec<(String, String)> {
    match env_vars {
        Some(env_vec) => {
            env_vec.iter().map(|var| {
                if var.contains("=") {
                    let mut m = var.splitn(2, "=");
                    (m.next().unwrap().to_string(), m.next().unwrap().to_string())
                } else {
                    (var.clone(), String::new())
                }
            }).collect()
        }
        None => Vec::new(),
    }
}

pub fn parse_vg_log(filepath: &String) -> Result<(i32, i32), TestingError> {
    let re = Regex::new(r"(?s)in use at exit: [0-9,]+ bytes? in (?P<leaks>[0-9,]+) blocks?.*ERROR SUMMARY: (?P<errors>[0-9,]+) errors? from [0-9,]+ contexts?")
        .unwrap();
    let mut retvar = (-1, 1);
    match read_to_string(filepath) {
        Ok(content) => match re.captures_iter(&content).last() {
            Some(cap) => {
                retvar.0 = cap["leaks"].replace(",", "").parse().unwrap_or(-1);
                retvar.1 = cap["errors"].replace(",", "").parse().unwrap_or(-1);
                return Ok(retvar);
            }
            None => {
                return Err(TestingError::VgLogParseError(filepath.clone()));
            }
        },
        Err(_) => {
            return Err(TestingError::VgLogNotFound(filepath.clone()));
        }
    }
}

pub fn check_program_availability(prog: &str) -> Result<(), TestingError> {
    #[allow(unused_must_use)] // we don't care if child process was killed successfully
    match Command::new(prog)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn() {
        Ok(mut child) => {
            child.kill();
            Ok(())
        },
        Err(_) => Err(TestingError::MissingBinDependency(prog.to_string()))
    }
}

