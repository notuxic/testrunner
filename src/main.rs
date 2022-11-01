#[macro_use]
extern crate lazy_static;

mod project;
mod test;
mod testresult;
mod testrunner;

use std::fs::write;

use clap::{App, Arg, crate_authors, crate_description, crate_version, ArgMatches};

use crate::testrunner::{Testrunner, TestrunnerOptions, TestrunnerError};


fn main() {
    let cli_args = App::new("testrunner")
        .version(crate_version!())
        .author(crate_authors!(",\n"))
        .about(crate_description!())
        .global_setting(clap::AppSettings::DeriveDisplayOrder)
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .takes_value(true)
            .value_name("CONFIG_FILE")
            .default_value("test.toml")
            .help("Sets testcase config file"))
        .arg(Arg::with_name("no-wshints")
            .short("n")
            .long("no-ws-hints")
            .takes_value(false)
            .help("Disables whitespace-hints in HTML report"))
        .arg(Arg::with_name("jobs")
            .short("J")
            .long("jobs")
            .takes_value(true)
            .value_name("JOBS")
            .default_value("0")
            .hide_default_value(true)
            .validator(|num| {
                match num.parse::<usize>() {
                    Ok(_) => Ok(()),
                    Err(_) => Err(format!("not a (positive) number: {}", num)),
                }
            })
            .help("Sets number of tests to run in parallel"))
        .arg(Arg::with_name("prot-mode")
            .short("p")
            .long("protected-mode")
            .help("Runs in protected-mode, with details of protected testcases redacted"))
        .arg(Arg::with_name("html")
            .short("o")
            .long("html-output")
            .takes_value(true)
            .value_name("HTML_OUTPUT")
            .default_value("testreport.html")
            .help("Generates HTML report"))
        .arg(Arg::with_name("json")
            .short("j")
            .long("json-output")
            .takes_value(true)
            .value_name("JSON_OUTPUT")
            .default_value("testreport.json")
            .help("Generates JSON report"))
        .arg(Arg::with_name("sudo")
            .long("sudo")
            .takes_value(true)
            .value_name("USER")
            .hidden(true)
            .help("Runs program through sudo as user <USER>"))
        .get_matches();


    match run(cli_args) {
        Ok(()) => (),
        Err(err) => {
            eprintln!("Error: {}", err.to_string());
            std::process::exit(2);
        },
    }
}

fn run(cli_args: ArgMatches) -> Result<(), TestrunnerError> {
    let options = TestrunnerOptions {
        protected_mode: cli_args.occurrences_of("prot-mode") > 0,
        ws_hints: cli_args.occurrences_of("no-wshints") == 0,
        sudo: cli_args.value_of("sudo").map(|e| e.to_string()),
        jobs: cli_args.value_of("jobs").unwrap().parse().unwrap(),
    };

    let mut runner = Testrunner::from_file(cli_args.value_of("config").unwrap(), options)?;
    runner.run_tests()?;

    if cli_args.occurrences_of("json") > 0 {
        let json_out = cli_args.value_of("json").unwrap();
        let output = runner.generate_json_report()?;
        write(json_out, output)?;
    }

    let html_out = cli_args.value_of("html").unwrap();
    if cli_args.occurrences_of("prot-mode") > 0 {
        let output = runner.generate_html_report(true)?;
        write(html_out, output)?;
    }
    else {
        let output = runner.generate_html_report(false)?;
        write(html_out, output)?;
    }

    Ok(())
}

