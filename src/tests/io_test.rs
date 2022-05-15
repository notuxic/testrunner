use std::fs::{copy, create_dir_all, Permissions, read_to_string, remove_dir_all, remove_file, set_permissions};
use std::io;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use difference::{Changeset, Difference};
use regex::Regex;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use uuid::Uuid;
use super::test::{DiffKind, Test, TestCaseKind, TestMeta};
use super::testresult::TestResult;
use super::testcase::Testcase;
use crate::project::binary::{Binary, GenerationError};
use crate::project::definition::ProjectDefinition;


#[allow(dead_code)]
pub struct IoTest {
    meta: TestMeta,
    in_file: String,
    exp_file: String,
    in_string: String,
    exp_string: String,
    binary: Binary,
    argv: Vec<String>,
    exp_retvar: Option<i32>,
    env_vars: Option<String>,
}


impl Test for IoTest {
    fn get_test_meta(&self) -> &TestMeta { &self.meta }

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

        // make changeset
        let changeset = Changeset::new(&reference_output, &given_output, &self.meta.projdef.diff_delim);

        let distance = changeset.distance;
        let add_diff = self.get_add_diff();
        let passed: bool = self.exp_retvar.is_some() && retvar.is_some() && retvar.unwrap() == self.exp_retvar.unwrap()
            && distance == 0 && add_diff.as_ref().unwrap_or(&(String::from(""), 0, 0.0)).1 == 0 && !had_timeout; //TODO check if there are not diffs

        if self.meta.projdef.verbose && distance > 0
        {
            println!("Diff-Distance: {:?}", distance);
            println!("------ START Reference ------");
            println!("Reference Output:\n{:?}", reference_output);
            println!("------ END Reference ------");
            println!("------ START Yours ------");
            println!("Your Output:\n{:?}", given_output);
            println!("------ END Yours ------");

            // prints diff with colors to terminal
            // green = ok
            // blue = reference (our solution)
            // red = wrong (students solution) / too much
            let mut colored_stdout = StandardStream::stdout(ColorChoice::Always);

            for c in &changeset.diffs
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

        if cfg!(unix) && self.meta.projdef.sudo.is_some() && self.meta.protected {
            remove_dir_all(&format!("{}/{}/{}", &basedir, &vg_log_folder, &self.meta.number)).unwrap_or(());
        }
        let vg_filepath = format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &self.meta.number);

        if self.meta.projdef.protected_mode && self.meta.protected {
            println!("Finished testcase {}: ********", self.meta.number);
        }
        else {
            println!("Finished testcase {}: {}", self.meta.number, self.meta.name);
        }


        Ok(TestResult {
            diff: Some(changeset),
            io_diff: None,
            add_distance_percentage: match &add_diff { Some(d) => Some(d.2), None => None },
            add_diff: match add_diff { Some(d) => Some(d.0), None => None },
            truncated_output,
            implemented: None,
            passed,
            output: given_output.clone(),
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
                    reference_output.len(),
                    given_output.len(),
            )),
            protected: self.meta.protected,
        })
    }

    #[allow(unused_variables)]
    fn from_saved_tc(
        number: i32,
        testcase: &Testcase,
        projdef: &ProjectDefinition,
        binary: Option<&Binary>,
    ) -> Result<Self, GenerationError> {
        match binary {
            Some(binary) => {}
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

        let retvar = IoTest {
            meta,
            binary: binary.unwrap().clone(),
            exp_retvar: testcase.exp_retvar,
            argv: testcase.args.as_ref().unwrap_or(&vec![String::new()]).clone(),
            in_file: testcase.in_file.as_ref().unwrap_or(&String::new()).clone(),
            exp_file: testcase.exp_file.as_ref().unwrap_or(&String::new()).clone(),
            in_string: testcase
                .in_string
                .as_ref()
                .unwrap_or(&String::new())
                .clone(),
            exp_string: testcase
                .exp_string
                .as_ref()
                .unwrap_or(&String::new())
                .clone(),
            env_vars: testcase.env_vars.clone(),
        };

        Ok(retvar)
    }
}

impl IoTest {
    fn run_command_with_timeout(&self, command : &str, args: &Vec<String>, envs: &Vec<(String, String)>, timeout : u64) -> (String, String, String, Option<i32>) {

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
            .cwd(self.meta.projdef.makefile_path.as_ref().unwrap_or(&String::from("./")))
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

pub fn prepare_valgrind(meta: &TestMeta, basedir: &str) -> (String, String) {
    let vg_log_folder = meta.projdef.valgrind_log_folder.clone().unwrap_or(String::from("valgrind_logs"));
    let vg_filepath = if cfg!(unix) && meta.projdef.sudo.is_some() {
        format!("{}/testrunner-{}", std::env::temp_dir().to_str().unwrap(), Uuid::new_v4().to_simple().to_string())
    } else {
        format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, meta.number)
    };

    if meta.projdef.use_valgrind.unwrap_or(true) {
        create_dir_all(format!("{}/{}/{}", &basedir, &vg_log_folder, &meta.number)).expect("could not create valgrind_log folder");
        #[cfg(unix)] {
            set_permissions(format!("{}/{}", &basedir, &vg_log_folder), Permissions::from_mode(0o750)).unwrap();
            set_permissions(format!("{}/{}/{}", &basedir, &vg_log_folder, &meta.number), Permissions::from_mode(0o750)).unwrap();
        }
    }

    (vg_log_folder, vg_filepath)
}

pub fn prepare_cmdline(meta: &TestMeta, vg_filepath: &str) -> Result<(String, Vec<String>), GenerationError> {
    let cmd_name = if meta.projdef.sudo.is_some() && cfg!(unix) {
        String::from("sudo")
    } else if meta.projdef.use_valgrind.unwrap_or(true) {
        String::from("valgrind")
    } else {
        String::from(format!("./{}", &meta.projdef.binary_path))
    };

    let mut flags = Vec::<String>::new();
    if meta.projdef.sudo.is_some() {
        check_program_availability("sudo")?;
        flags.push(String::from("--preserve-env"));
        flags.push(format!("--user={}", &meta.projdef.sudo.as_ref().unwrap()));
        if meta.projdef.use_valgrind.unwrap_or(true) {
            flags.push(String::from("valgrind"));
        }
    }
    if meta.projdef.use_valgrind.unwrap_or(true) {
        check_program_availability("valgrind")?;
        if let Some(v) = &meta.projdef.valgrind_flags {
            flags.append(&mut v.clone());
        }
        else {
            flags.push(String::from("--leak-check=full"));
            flags.push(String::from("--show-leak-kinds=all"));
            flags.push(String::from("--track-origins=yes"));
        }
        flags.push(format!("--log-file={}", &vg_filepath ));
        flags.push(format!("./{}", &meta.projdef.binary_path));
    }

    Ok((cmd_name, flags))
}

pub fn prepare_envvars(env_vars: Option<&String>) -> Vec<(String, String)> {
    match env_vars {
        Some(var_string) => {
            let mut splits: Vec<(String, String)> = Vec::new();
            for split in var_string.split(",") {
                if split.contains("=") {
                    let mut m = split.splitn(2, "=");
                    splits.push((
                            m.next().unwrap().clone().to_string(),
                            m.next().unwrap().clone().to_string(),
                    ));
                } else {
                    splits.push((String::from(split), String::new()));
                }
            }
            splits
        }
        None => Vec::new(),
    }
}

pub fn parse_vg_log(filepath: &String) -> Result<(i32, i32), GenerationError> {
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
                return Err(GenerationError::VgLogParseError);
            }
        },
        Err(err) => {
            eprintln!("Cannot open valgrind log: {}\n{}", filepath, err);
            return Err(GenerationError::VgLogNotFound);
        }
    }
}

pub fn check_program_availability(prog: &str) -> Result<(), GenerationError> {
    match Command::new(prog)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn() {
        Ok(mut child) => {
            child.kill().map_err(|_| ());
            Ok(())
        },
        Err(_) => Err(GenerationError::MissingCLIDependency(prog.to_string()))
    }
}

pub fn percentage_from_levenstein(steps: i32, source_len: usize, target_len: usize) -> f32 {
    if (source_len == 0) || (target_len == 0) {
        return 0.0;
    } else {
        return 1.0 - ((steps as f32) / (source_len as f32).max(target_len as f32));
    }
}

