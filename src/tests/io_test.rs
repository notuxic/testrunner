use std::{convert::TryInto, fs::{create_dir_all, read_to_string}};
use std::io::{Read, Write};
use std::time::{ Instant};
use difference::{Changeset, Difference};
use regex::Regex;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
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


        create_dir_all(format!("{}/valgrind_logs/{}", &self.meta.projdef.makefile_path.as_ref().unwrap_or(&String::from(".")), &self.meta.number)).expect("could not create valgrind_log folder");
        let vg_filepath = format!("{}/valgrind_logs/{}/vg_log.txt", &self.meta.projdef.makefile_path.as_ref().unwrap_or(&String::from(".")), &self.meta.number);

        let mut vg_flags = match &self.meta.projdef.valgrind_flags {
            Some(to) => to.clone(),
            None => vec![String::from("--leak-check=full"), String::from("--track-origins=yes")],
        };

        vg_flags.push(format!("--log-file={}", &vg_filepath ));
        vg_flags.push(format!("./{}", &self.meta.projdef.project_name));
        vg_flags.append(&mut self.argv.clone() ); //.push(self.argv.clone());

        let starttime = Instant::now();


        let global_timeout = self.meta.projdef.global_timeout.unwrap_or(5);
        let timeout = self.meta.timeout.unwrap_or(global_timeout);

        // let (mut given_output, retvar) = command_timeout(run_cmd, timeout, self.meta.number);
        let (input, reference_output, mut given_output, retvar) = self.run_command_with_timeout("valgrind", &vg_flags, timeout);
        println!("returned from run_command_with_timeout");

        println!("Got output from testcase {}", self.meta.number);

        let mut had_timeout = true;
        if retvar.is_some() {
            had_timeout = false;
        }
        else {
            if given_output.len() > reference_output.len() * 2 {
                let output_length = std::cmp::min( reference_output.len()  * 2 ,  given_output.len() );
                given_output = given_output.chars().take(output_length).collect();
                println!("Reducing output length because of endless loop!");
            }

        }

        // make changeset
        let changeset = Changeset::new(&reference_output, &given_output, &self.meta.projdef.diff_mode);

        let distance = changeset.distance;//get_distance(&reference_output, &given_output.0);
        let status = retvar; // TODO refactor
        let add_diff = self.get_add_diff();
        let passed: bool = self.exp_retvar.is_some() && status.is_some() && status.unwrap() == self.exp_retvar.unwrap()
            && distance == 0 && add_diff.as_ref().unwrap_or(&(String::from(""), 0)).1 == 0 && !had_timeout; //TODO check if there are not diffs

        if self.meta.projdef.verbose && distance != 0
        {
            println!("Diff-Distance: {:?}", distance);
            println!("------ START Reference ------");
            println!("Reference Output:\n{:?}", reference_output);
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
            diff: Some(changeset),
            //diff: Some(diff),
            add_diff: match add_diff { Some(d) => Some(d.0), None => None },
            implemented: None,
            passed,
            result: given_output.clone(),
            ret: status,
            exp_ret: self.exp_retvar,
            vg_warnings: valgrind.0,
            vg_errors: valgrind.1,
            vg_logfile: vg_filepath,
            command_used: String::from(format!("./{} {}", &self.meta.projdef.project_name, &self.argv.clone().join(" "))),
            used_input: input,
            timeout: had_timeout,
            name: self.meta.name.clone(),
            description: self.meta.desc.clone().unwrap_or(String::from("")),
            number: self.meta.number,
            kind: self.meta.kind,
            distance_percentage: Some(percentage_from_levenstein(
                    distance,
                    &reference_output,
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

impl IoTest {

    fn run_command_with_timeout(&self, command : &str, args: &Vec<String>,  timeout : u64) -> (String, String, String, Option<i32>) {

        let mut input: String = String::new();
        if !self.in_file.is_empty() {
            match read_to_string(&self.in_file) {
                Ok(content) => {
                    input.clone_from(&content);
                }
                Err(err) => {
                    println!("Cannot open stdinfile, fallback to none \n{:?}", err);
                }
            }
        } else if !self.in_string.is_empty() {
            input.clone_from(&self.in_string);
        }

        let mut reference_output: String = String::new();
        if !self.exp_file.is_empty() {
            match read_to_string(&self.exp_file) {
                Ok(content) => {
                    reference_output = content;
                }
                Err(err) => {
                    println!("Cannot open stdout, fallback to none \n{:?}", err);
                }
            }
        } else if !self.exp_string.is_empty() {
            reference_output = self.exp_string.clone();
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


        let mut command_with_args = String::from(format!("{:?}", command));
        for elem in args.iter() {
            if !elem.is_empty() {
                command_with_args.push_str(&format!(" {:?} ", elem));
            }
        }


        let mut _retvar = Some(-99);
        let mut _output = String::new();

        let mut cmd = subprocess::Exec::shell(command_with_args )
                        .cwd(self.meta.projdef.makefile_path.as_ref().unwrap_or(&String::from("./")))
                        .stdin(subprocess::Redirection::Pipe)
                        .stdout(subprocess::Redirection::Pipe)
                        .env_extend(&envs)
                        .popen()
                        .expect("Could not spawn process");


        let capture = cmd.communicate_start(Some( input.as_bytes().iter().cloned().collect() ) )
                                        .limit_time(std::time::Duration::new(timeout , 0))
                                        .limit_size(reference_output.len() + 200).read();

        if cmd.poll().is_none() {
            println!("Killing testcase {} because of timeout", self.meta.number); //if self.meta.protected {"**"} else {&self.meta.number.to_string()} );
            cmd.kill().expect("Upps, can't kill this one");         
            cmd.wait().expect("failed waiting for kill");  
        }

        match capture {
            Ok(c) => {
                _output = format!("{}", String::from_utf8_lossy(&c.0.unwrap_or(Vec::new())) );
            }
            Err(e) => {
                _output = format!("{}", String::from_utf8_lossy(&e.capture.0.unwrap_or(Vec::new())) );
            }
        }

        match cmd.exit_status().unwrap_or(subprocess::ExitStatus::Undetermined) {
            subprocess::ExitStatus::Exited(code) => {
                _retvar = Some(code as i32);
            }
            _ => {
                _retvar = None;
            }
        }


        return (input, reference_output, _output, _retvar);

    }
        
}

