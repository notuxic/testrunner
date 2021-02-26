#[macro_use]
extern crate horrorshow;

use std::fs::{read_to_string, write};
use std::process::Command;
use clap::{App, Arg, crate_authors, crate_description, crate_version};
use crate::tests::generator::TestcaseGenerator;

mod tests;
mod project;

fn main() {
    let cli_args = App::new("testrunner")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .global_setting(clap::AppSettings::DeriveDisplayOrder)
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .takes_value(true)
            .value_name("CONFIG_FILE")
            .required_unless("TESTINPUT")
            .help("sets the TOML test specification config"))
        .arg(Arg::with_name("diff_mode")
            .short("m")
            .long("diff-mode")
            .takes_value(true)
            .value_name("DIFF_MODE")
            .default_value("l")
            .possible_values(&["l", "w", "c"])
            .help("sets mode of diff-comparison\nl : compare outputs line by line\nw : compare outputs word by word\nc : compare outputs char by char"))
        .arg(Arg::with_name("html")
            .short("o")
            .long("html-output")
            .takes_value(true)
            .value_name("HTML_OUTPUT")
            .default_value("result.html")
            .help("writes testresult in pretty html format"))
        .arg(Arg::with_name("prot-html")
            .short("p")
            .long("prot-html")
            .takes_value(true)
            .value_name("PROT_HTML_OUTPUT")
            .default_value("prot-result.html")
            .help("writes testresult in pretty html format, with details of protected testcases redacted"))
        .arg(Arg::with_name("json")
            .short("j")
            .long("json-output")
            .takes_value(true)
            .value_name("JSON_OUT")
            .default_value("result.json")
            .help("writes testresult in json format to specific file"))
        .arg(Arg::with_name("browser")
            .short("b")
            .requires("html")
            .help("opens the html file with xdg-open"))
        .arg(Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .takes_value(false)
            .help("prints diff to stdout"))
        .get_matches();

    let diff_mode = cli_args.value_of("diff_mode").unwrap();
    let config = read_to_string(cli_args.value_of("config").unwrap()).expect("cannot open or read config file");

    let mut generator = TestcaseGenerator::from_string(&config).expect("could not parse config file");
    generator.set_verbosity(cli_args.is_present("verbose"));
    generator.set_diff_mode(diff_mode.to_string());
    match generator.generate_generateables() {
        Ok(_) => println!("Done generating"),
        Err(e) => eprintln!("Error generating: {}", e),
    };

    match generator.run_generateables() {
        Ok(_) => println!("Done testing"),
        Err(e) => eprintln!("Error running: {}", e),
    };

    if let Some(json_out) = cli_args.value_of("json") {
        let output = generator
            .make_json_report()
            .expect("could not make json report");
        write(json_out, output).expect("cannot write json file");
    }

    if let Some(html_out) = cli_args.value_of("html") {
        let output = generator
            .make_html_report(diff_mode, false)
            .expect("could not make html report");
        write(html_out, output).expect("cannot write html file");

        if cli_args.is_present("browser") {
            println!("open browser");
            Command::new("xdg-open")
                .arg(html_out)
                .spawn()
                .expect("cannot start xdg-open");
        }
    }

    if cli_args.occurrences_of("prot-html") > 0 {
        let prot_html_out = cli_args.value_of("prot-html").unwrap();
        let output = generator
            .make_html_report(diff_mode, true)
            .expect("could not make html report");
        write(prot_html_out, output).expect("cannot write html file");

        if cli_args.is_present("browser") {
            println!("open browser");
            Command::new("xdg-open")
                .arg(prot_html_out)
                .spawn()
                .expect("cannot start xdg-open");
        }
    }
}

