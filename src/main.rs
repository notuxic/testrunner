#[macro_use]
extern crate horrorshow;
#[macro_use]
extern crate lazy_static;

mod project;
mod test;
mod testresult;
mod testrunner;

use std::fs::write;
use clap::{App, Arg, crate_authors, crate_description, crate_version, ArgMatches};
use testrunner::TestrunnerError;
use crate::testrunner::{Testrunner, TestrunnerOptions};


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
            .required_unless("TESTINPUT")
            .help("set testcase config file"))
        .arg(Arg::with_name("no-wshints")
            .short("n")
            .long("no-ws-hints")
            .takes_value(false)
            .help("disable whitespace-hints in diffs of HTML report"))
        .arg(Arg::with_name("html")
            .short("o")
            .long("html-output")
            .takes_value(true)
            .value_name("HTML_OUTPUT")
            .default_value("testreport.html")
            .help("generate HTML report"))
        .arg(Arg::with_name("prot-html")
            .short("p")
            .long("prot-html")
            .takes_value(true)
            .value_name("PROT_HTML_OUTPUT")
            .default_value("testreport_protected.html")
            .help("generate HTML report, with details of protected testcases redacted"))
        .arg(Arg::with_name("json")
            .short("j")
            .long("json-output")
            .takes_value(true)
            .value_name("JSON_OUTPUT")
            .default_value("testreport.json")
            .help("generate JSON report"))
        .arg(Arg::with_name("sudo")
            .long("sudo")
            .takes_value(true)
            .value_name("USER")
            .hidden(true)
            .help("run program through sudo as user <USER>"))
        .arg(Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .takes_value(false)
            .help("print additional information to stdout"))
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
        verbose: cli_args.is_present("verbose"),
        protected_mode: cli_args.occurrences_of("prot-html") > 0,
        ws_hints: cli_args.occurrences_of("no-wshints") == 0,
        sudo: cli_args.value_of("sudo").map(|e| e.to_string()),
    };

    let mut runner = Testrunner::from_file(cli_args.value_of("config").unwrap(), options)?;
    runner.run_tests()?;

    if cli_args.occurrences_of("json") > 0 {
        let json_out = cli_args.value_of("json").unwrap();
        let output = runner.generate_json_report()?;
        write(json_out, output).expect("Cannot write JSON report to file!");
    }

    if cli_args.occurrences_of("prot-html") > 0 {
        let prot_html_out = cli_args.value_of("prot-html").unwrap();
        let output = runner.generate_html_report(true)?;
        write(prot_html_out, output).expect("Cannot write HTML report to file!");
    }
    else if let Some(html_out) = cli_args.value_of("html") {
        if html_out != "NONE" {
            let output = runner.generate_html_report(false)?;
            write(html_out, output).expect("Cannot write HTML report to file!");
        }
    }

    Ok(())
}

