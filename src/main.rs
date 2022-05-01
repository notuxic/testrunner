#[macro_use]
extern crate horrorshow;
#[macro_use]
extern crate maplit;


use std::fs::{read_to_string, write};
use clap::{App, Arg, crate_authors, crate_description, crate_version};
use crate::tests::generator::TestcaseGenerator;

mod tests;
mod project;

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
        .arg(Arg::with_name("diff_mode")
            .short("m")
            .long("diff-mode")
            .takes_value(true)
            .value_name("DIFF_MODE")
            .default_value("line")
            .possible_values(&["line", "l", "word", "w", "char", "c"])
            .help("set diff-mode. Possible values for <DIFF_MODE>:\nline : compare outputs line by line\nword : compare outputs word by word\nchar : compare outputs char by char\n"))
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


    let config = read_to_string(cli_args.value_of("config").unwrap())
        .expect(&format!("Cannot open or read config file: {}", cli_args.value_of("config").unwrap()));
    let diff_delim = match cli_args.value_of("diff_mode").unwrap() {
        "char" => "",
        "c" => "",
        "word" => " ",
        "w" => " ",
        _ => "\n",
    };

    let mut generator = TestcaseGenerator::from_string(&config).expect("Could not parse config file!");
    generator.set_verbosity(cli_args.is_present("verbose"));
    generator.set_diff_delimiter(diff_delim.to_string());
    generator.set_protected_mode(cli_args.occurrences_of("prot-html") > 0);
    generator.set_whitespace_hinting(cli_args.occurrences_of("no-wshints") == 0);
    if cli_args.is_present("sudo") {
        generator.set_sudo(cli_args.value_of("sudo"));
    }

    match generator.generate_generateables() {
        Ok(_) => println!("Done generating"),
        Err(e) => eprintln!("Error generating:\n{}", e),
    };

    match generator.run_generateables() {
        Ok(_) => {
            println!("\nDone testing");
            println!("Passed testcases: {} / {}", generator.testresults.iter().filter(|test| test.passed).count(), generator.testresults.len());
        },
        Err(e) => {
            eprintln!("Error running:\n{}", e);
            std::process::exit(1);
        },
    };


    if let Some(html_out) = cli_args.value_of("html") {
        if html_out != "NONE" {
            let output = generator
                .make_html_report(diff_delim, false)
                .expect("Failed generating HTML report!");
            write(html_out, output).expect("Cannot write HTML report to file!");
        }
    }

    if cli_args.occurrences_of("prot-html") > 0 {
        let prot_html_out = cli_args.value_of("prot-html").unwrap();
        let output = generator
            .make_html_report(diff_delim, true)
            .expect("Failed generating HTML report!");
        write(prot_html_out, output).expect("Cannot write HTML report to file!");
    }

    if cli_args.occurrences_of("json") > 0 {
        let json_out = cli_args.value_of("json").unwrap();
        let output = generator
            .make_json_report()
            .expect("Failed generating JSON report!");
        write(json_out, output).expect("Cannot write JSON report to file!");
    }
}

