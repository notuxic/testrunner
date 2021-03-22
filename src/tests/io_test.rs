use std::fs::{create_dir_all, read_to_string};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use difference::{Changeset, Difference};
use regex::Regex;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use wait_timeout::ChildExt;
use super::test::{Test, TestCaseKind, TestMeta};
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
    fn run(&self) -> Result<TestResult, GenerationError> {
        if self.meta.projdef.protected_mode && self.meta.protected {
            println!("\nStarting testcase {}: ********", self.meta.number);
        }
        else {
            println!("\nStarting testcase {}: {}", self.meta.number, self.meta.name);
        }

        let mut stdinstring: String = String::new();
        if !self.in_file.is_empty() {
            match read_to_string(&self.in_file) {
                Ok(content) => {
                    stdinstring.clone_from(&content);
                }
                Err(err) => {
                    println!("Cannot open stdinfile, fallback to none \n{:?}", err);
                }
            }
        } else if !self.in_string.is_empty() {
            stdinstring.clone_from(&self.in_string);
        }
        let envs: Vec<(String, String)> = match &self.env_vars {
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
        };
        // same for expected stdout
        let mut stdoutstring: String = String::new();
        if !self.exp_file.is_empty() {
            match read_to_string(&self.exp_file) {
                Ok(content) => {
                    stdoutstring = content;
                }
                Err(err) => {
                    println!("Cannot open stdout, fallback to none \n{:?}", err);
                }
            }
        } else if !self.exp_string.is_empty() {
            stdoutstring = self.exp_string.clone();
        }

        create_dir_all(format!("{}/valgrind_logs/{}", &self.meta.projdef.makefile_path.as_ref().unwrap_or(&String::from(".")), &self.meta.number)).expect("could not create valgrind_log folder");
        let vg_filepath = format!("{}/valgrind_logs/{}/vg_log.txt", &self.meta.projdef.makefile_path.as_ref().unwrap_or(&String::from(".")), &self.meta.number);

        let mut vg_flags = match &self.meta.projdef.valgrind_flags {
            Some(to) => to.clone(),
            None => vec![String::from("--leak-check=full"), String::from("--track-origins=yes")],
        };

        vg_flags.push(format!("--log-file={}", &vg_filepath ));
        vg_flags.push(format!("./{}", &self.meta.projdef.project_name));
        vg_flags.append(&mut self.argv.clone() ); //.push(self.argv.clone());

        // // run assignment file compiled with fsanitize
        // let mut run_cmd = Command::new(format!("./{}", &self.meta.projdata.project_name))
        //     //assuming makefile_path = project path
        //     .current_dir(
        //         &self
        //             .meta
        //             .projdata
        //             .makefile_path
        //             .as_ref()
        //             .unwrap_or(&String::from("./")),
        //     )
        //     .args([
        //         &self.argv,
        //     ].iter().filter(|s| !s.is_empty()))
        //     .stdin(Stdio::piped())
        //     .stdout(Stdio::piped())
        //     .stderr(Stdio::piped())
        //     .envs(envs)
        //     .spawn()
        //     .expect("could not spawn process");

        let starttime = Instant::now();

        let mut run_cmd = Command::new("valgrind")
            // run valgrind with the given program name
            //assuming makefile_path = project path
            .current_dir(
                &self
                .meta
                .projdef
                .makefile_path
                .as_ref()
                .unwrap_or(&String::from("./")))
            .args(vg_flags.iter().filter(|s| !s.is_empty()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(envs)
            .spawn()
            .expect("could not spawn process");

        if !stdinstring.is_empty() {
            let stdin = run_cmd.stdin.as_mut().expect("failed to get stdin");
            stdin
                .write_all(&stdinstring.clone().into_bytes())
                .expect("could not send input");
        }

        let global_timeout = self.meta.projdef.global_timeout.unwrap_or(5);
        let timeout = self.meta.timeout.unwrap_or(global_timeout);

        let (mut given_output, retvar) = command_timeout(run_cmd, timeout, self.meta.number);
        println!("Got output from testcase {}", self.meta.number);

        let mut had_timeout = true;
        if retvar.is_some() {
            had_timeout = false;
        }
        else {
            if given_output.len() > stdoutstring.len() * 4 {
                let output_length = std::cmp::min( stdoutstring.len()  * 4 ,  given_output.len() );
                given_output = given_output.chars().take(output_length).collect();
                println!("Reducing output length because of endless loop!");
            }

        }

        // make changeset
        let changeset = Changeset::new(&stdoutstring, &given_output, &self.meta.projdef.diff_mode);

        let distance = changeset.distance;//get_distance(&stdoutstring, &given_output.0);
        let status = retvar; // TODO refactor
        let passed: bool = self.exp_retvar.is_some() && status.is_some() && status.unwrap() == self.exp_retvar.unwrap() && distance == 0 && !had_timeout; //TODO check if there are not diffs

        if self.meta.projdef.verbose && distance != 0
        {
            println!("Diff-Distance: {:?}", distance);
            println!("------ START Reference ------");
            println!("Reference Output:\n{:?}", stdoutstring);
            println!("------ END Reference ------");
            println!("------ START Yours ------");
            println!("Your Output:\n{:?}", given_output);
            println!("------ END Yours ------");
        }

        // prints diff with colors to terminal
        // green = ok
        // blue = reference (our solution)
        // red = wrong (students solution) / too much

        if changeset.distance > 0 &&  self.meta.projdef.verbose
        {
            let mut colored_stdout = StandardStream::stdout(ColorChoice::Always);

            for c in &changeset.diffs
            {
                match *c
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


        let valgrind = parse_vg_log(&vg_filepath).unwrap_or((-1, -1));
        println!("Valgrind warnings: {:?}\nValgrind errors: {:?}", valgrind.0, valgrind.1);

        let endtime = Instant::now();
        println!("Testcase took {:?}", endtime.duration_since(starttime));
        if self.meta.projdef.protected_mode && self.meta.protected {
            println!("Finished testcase {}: ********", self.meta.number);
        }
        else {
            println!("Finished testcase {}: {}", self.meta.number, self.meta.name);
        }


        Ok(TestResult {
            diff : Some(changeset),
            //diff: Some(diff),
            implemented: None,
            passed,
            result: given_output.clone(),
            ret: status,
            exp_ret: self.exp_retvar,
            vg_warnings: valgrind.0,
            vg_errors: valgrind.1,
            vg_logfile: vg_filepath,
            command_used: String::from(format!("./{} {}", &self.meta.projdef.project_name, &self.argv.clone().join(" "))),
            used_input: stdinstring,
            timeout: had_timeout,
            name: self.meta.name.clone(),
            description: self.meta.desc.clone().unwrap_or(String::from("")),
            number: self.meta.number,
            kind: self.meta.kind,
            distance_percentage: Some(percentage_from_levenstein(
                    distance,
                    &stdoutstring,
                    &given_output,
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
        let meta = TestMeta {
            kind: TestCaseKind::IOTest,
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
            argv: testcase.args.as_ref().unwrap_or(&vec![String::new()]).clone(), //testcase.args.as_ref().unwrap_or(&String::new()).clone(),
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

pub fn percentage_from_levenstein(steps: i32, source: &String, target: &String) -> f32 {
    if (source.len() == 0) || (target.len() == 0) {
        return 0.0;
    } else {
        return 1.0 - ((steps as f32) / (source.len() as f32).max(target.len() as f32));
    }
}

pub fn parse_vg_log(filepath: &String) -> Result<(i32, i32), GenerationError> {
    let re = Regex::new(r"ERROR SUMMARY: (?P<err>[0-9]+) errors? from (?P<warn>[0-9]+) contexts?")
        .unwrap();
    let mut retvar = (-1, 1);
    match read_to_string(filepath) {
        Ok(content) => match re.captures_iter(&content).last() {
            Some(cap) => {
                retvar.0 = cap["warn"].parse().unwrap_or(-1);
                retvar.1 = cap["err"].parse().unwrap_or(-1);
                return Ok(retvar);
            }
            None => {
                return Err(GenerationError::VgLogParseError);
            }
        },
        Err(err) => {
            println!("Cannot open valgrind log: {}\n{}", filepath, err);
            return Err(GenerationError::VgLogNotFound);
        }
    }
}

fn command_timeout(cmd: Child, timeout: i32, number: i32) -> (String, Option<i32>) {
    let mut cmd = cmd;

    let mut output = String::new();
    let mut _retvar = Some(-99);
    let mut tmp : Vec<u8> =  Vec::new();

    match cmd.wait_timeout(Duration::from_secs(timeout as u64)).unwrap() {
        Some(expr) => {
            _retvar = Some(expr.code().unwrap_or(-99));
        }
        None => {
            _retvar = None;
            println!("Killing testcase {} because of timeout", number);
            cmd.kill().expect("Upps, can't kill this one");
        }
    }

    cmd.stdout.as_mut().unwrap().read_to_end(&mut tmp).expect("could not read stdout");
    output = format!("{}{}", output, String::from_utf8_lossy(&tmp));

    return (output, _retvar);
}

