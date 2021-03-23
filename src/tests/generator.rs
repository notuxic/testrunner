use std::collections::HashMap;
use horrorshow::Raw;
use horrorshow::helper::doctype;
use super::test::Test;
use super::testcase::TestDefinition;
use super::testresult::TestResult;
use super::unit_test::UnitTest;
use super::io_test::IoTest;
use crate::project::binary::{Binary, GenerationError};

#[allow(dead_code)]
pub struct TestcaseGenerator {
    test_cases: Vec<Box<dyn Test + Send + Sync>>,
    test_results: Vec<TestResult>,
    binary: Binary,
    config: TestDefinition,
}

impl TestcaseGenerator {
    pub fn from_string(s: &String) -> Result<Self, GenerationError> {
        let config: TestDefinition = match toml::from_str(s) {
            Ok(c) => c,
            Err(err) => {
                println!("{}", err);
                return Err(GenerationError::ConfigErrorIO);
            }
        };

        let binary: Binary = match Binary::from_project_definition(&config.project_definition) {
            Ok(content) => content,
            Err(err) => {
                println!("{:?}", err);
                return Err(GenerationError::CouldNotMakeBinary);
            }
        };
        Ok(TestcaseGenerator {
            config,
            binary,
            test_cases: vec![],
            test_results: vec![],
        })
    }

    pub fn generate_generateables(&mut self) -> Result<(), GenerationError> {
        let mut n: i32 = 1;
        for tc in self.config.testcases.iter() {
            match tc.testcase_type.as_str() {
                "UnitTest" => {
                    let unit_test =
                        UnitTest::from_saved_tc(n, tc, &self.config.project_definition, None).unwrap();
                    self.test_cases.push(Box::new(unit_test));
                }
                "IO" => {
                    let io_test =
                        IoTest::from_saved_tc(n, tc, &self.config.project_definition, Some(&self.binary))
                        .unwrap();
                    self.test_cases.push(Box::new(io_test));
                }
                _ => {}
            }
            n += 1;
        }
        return Ok(());
    }

    pub fn run_generateables(&mut self) -> Result<(), GenerationError> {
        if !self.binary.compile().is_ok() {
            println!("could not compile binary, no tests were run");
            return Err(GenerationError::CouldNotMakeBinary);
        }

        self.test_results = self
            .test_cases
            .iter()
            .map(|tc| match tc.run() {
                Ok(tr) => Some(tr),
                Err(e) => {
                    println!("Error running testcase: {}", e);
                    None
                }
            })
        .filter(|x| x.is_some())
            .map(|x| x.unwrap())
            .collect();

        return Ok(());
    }

    pub fn make_html_report(&self, compare_mode : &str, protected_mode : bool) -> Result<String, GenerationError> {
        let tc_public_num = self.test_results.iter().filter(|test| !test.protected).collect::<Vec<&TestResult>>().len();
        let tc_public_passed = self.test_results.iter().filter(|test| !test.protected && test.passed).collect::<Vec<&TestResult>>().len();
        let tc_private_num = self.test_results.iter().filter(|test| test.protected).collect::<Vec<&TestResult>>().len();
        let tc_private_passed = self.test_results.iter().filter(|test| test.protected && test.passed).collect::<Vec<&TestResult>>().len();
        let tc_all_num = self.test_results.len();
        let tc_all_passed = self.test_results.iter().filter(|test| test.passed).collect::<Vec<&TestResult>>().len();
        let compiler_output = self.binary.info.errors.clone().unwrap_or(String::from("<i>failed fetching compiler output!</i>"));

        let result = html! {
            : doctype::HTML;
            html{
                head{
                    title:"Testreport";
                }
                //CSS
                style{
                    : Raw(
                        r"
                            @import url('https://fonts.googleapis.com/css2?family=Roboto:wght@300&display=swap');
                            @import url('https://cdn.jsdelivr.net/npm/hack-font@3.3.0/build/web/hack.css');
                            body {
                                font-family: 'Roboto', sans-serif;
                                font-weight: 300;
                                color: #222;
                                max-width: 100em;
                                margin-left: auto;
                                margin-right: auto
                            }
                            body > h1 {
                                text-align: center;
                                font-size: 3em
                            }
                            body > h2 {
                                font-size: 1.8em;
                                border-bottom: 0.1em solid #666;
                                margin-top: 4em
                            }
                            table {
                                border-collapse: collapse
                            }
                            tr:hover {
                                background: #eee
                            }
                            th {
                                text-align: right
                            }
                            th, td {
                                padding-left: 1em;
                                padding-right: 1em
                            }
                            a {
                                text-decoration: none;
                            }
                            #shortreport {
                                margin-top: 3em;
                                margin-left: auto;
                                margin-right: auto
                            }
                            #shortreport th, #shortreport td {
                                text-align: center
                            }
                            #shortreport td:first-child {
                                text-align: left;
                            }
                            #shortreport tr:first-child th {
                                border-bottom: 0.1em solid #222
                            }
                            #shortreport tr:hover:first-of-type {
                                background: initial
                            }
                            #long_report {
                                margin-top: 5em
                            }
                            #long_report > div {
                                margin-left: 5em;
                                margin-right: 5em
                            }
                            #long_report > div#description {
                                margin-left: 10em;
                                margin-right: 10em
                            }
                            #title > h2 {
                                display: flex;
                                border-bottom: 0.1em dashed #444
                            }
                            #shortinfo {
                                margin-left: auto;
                                margin-right: auto;
                                margin-top: 2em
                            }
                            div#shortinfo table {
                                margin-left: auto;
                                margin-right: auto
                            }
                            #shortinfo > table th:first-of-type {
                                border-right: 0.1em solid #222
                            }
                            table td, table td * {
                                vertical-align: top;
                                horizontal-align: top
                            }
                            #differences {
                                background: #eee;
                                margin-top: 3em;
                                padding-left: 3em;
                                width: initial
                            }
                            #differences tr:first-of-type {
                                border-bottom: 0.1em solid #222
                            }
                            #differences th {
                                text-align: center;
                                padding: 0.5em
                            }
                            #differences td {
                                font-family: 'Hack', monospace;
                                font-size: 0.82em;
                                padding: 0.5em;
                                min-width: 82ch;
                                max-width: 82ch;
                                word-wrap: anywhere;
                                word-break: break-all
                            }
                            #differences #compiler {
                                min-width: 122ch;
                                max-width: 122ch;
                            }
                            #differences td:nth-child(2), #differences th:nth-child(2) {
                                border-left: 0.1em dashed #222
                            }
                            #diff-add {
                                background-color: yellowgreen
                            }
                            #diff-remove {
                                background-color: IndianRed
                            }
                            .inline-code {
                                background: #eee;
                                font-family: 'Hack', monospace;
                                font-size: 0.84em;
                                font-weight: 300;
                                vertical-align: baseline;
                            }
                            .link-summary {
                                display: inline-block;
                                font-size: 0.8em;
                                font-weight: normal;
                                vertical-align: baseline;
                                margin-left: auto;
                            }
                            .whitespace-hint {
                                color: #bbb
                            }
                            #diff-add .whitespace-hint {
                                color: green
                            }
                            #diff-remove .whitespace-hint {
                                color: darkred
                            }
                            #failed {
                                width: 61em;
                                margin-top: 5em;
                                margin-left: auto;
                                margin-right: auto
                            }
                            .warning {
                                font-size: large;
                                background-color: #ff000033;
                                color: darkred;
                                padding: 0.5em;
                                border-left: darkred 0.4em solid
                            }
                            #flex-container {
                                display: flex;
                                flex-direction: row;
                                justify-content: center;
                                align-items: center
                            }
                        ")
                }
                body{
                    h1 : "Testreport";
                         @ if !self.binary.info.compiled {
                             div(id="failed") {
                                 span(class="warning") {:"Could not compile project, no testcases were run!"}
                                 table(id="differences") {
                                     tr{
                                         th{:"Compiler Output"}
                                     }
                                     tr{
                                         td(id="compiler") {:Raw(format!("{}", compiler_output.replace("\n", "<br>").replace(" ", "&nbsp;")))}
                                     }
                                 }
                             }
                         }
                         else {
                             // create short report
                             h2{:Raw("<a id=\"summary\"></a>Summary")}
                                 div(id="flex-container") {
                                     @ if self.binary.info.warnings.is_some() {
                                         table(id="shortreport") {
                                             tr{
                                                 th{:"Compiler Warning"}
                                                 th{:"Occurences"}
                                             }
                                             |templ| {
                                                 for (warn, amount) in self.binary.info.warnings.clone().unwrap().iter() {
                                                     &mut *templ << Raw(format!("<tr><td>{}</td><td>{}</td></tr>", warn, amount));
                                                 }
                                             }
                                         }
                                     }
                                     div(id="shortinfo"){
                                         table{
                                             tr{
                                                 th{:"Public Testcases"}
                                                 td{:format!("{} / {} ({}%)", tc_public_passed, tc_public_num, ((tc_public_passed as f32 / tc_public_num as f32) * 10000.0).floor() / 100.0)}
                                             }
                                             tr{
                                                 th{:"Private Testcases"}
                                                 td{:format!("{} / {} ({}%)", tc_private_passed, tc_private_num, ((tc_private_passed as f32 / tc_private_num as f32) * 10000.0).floor() / 100.0)}
                                             }
                                             tr{
                                                 th{:"All Testcases"}
                                                 td{:format!("{} / {} ({}%)", tc_all_passed, tc_all_num, ((tc_all_passed as f32 / tc_all_num as f32) * 10000.0).floor() / 100.0)}
                                             }
                                         }
                                     }
                                 }
                                 table(id="shortreport"){
                                     th{
                                         : "Name"
                                     }
                                     th{
                                         : "Type"
                                     }
                                     th{
                                         : "Passed"
                                     }
                                     th{
                                         : "Percentage"
                                     }
                                     th{
                                         : "Timeout"
                                     }
                                     th{
                                         : "Valgrind Warnings"
                                     }
                                     th{
                                         : "Valgrind Errors"
                                     }
                                     th{
                                         : "Valgrind Log"
                                     }
                                     |templ| {
                                         for tc in self.test_results.iter() {
                                             match tc.get_html_short(protected_mode) {
                                                 Ok(res)=> {
                                                     &mut *templ << Raw(res);
                                                 }
                                                 Err(_err) => {
                                                     &mut *templ << Raw(String::from("<th></th><th></th><th></th><th></th><th></th><th></th><th></th>"))
                                                 }
                                             }
                                         }

                                     }
                                 }
                             h2 : "Testcases";

                                  |templ| {
                                      for tc in self.test_results.iter() {
                                          if !(protected_mode && tc.protected) {
                                              &mut *templ << Raw(tc.get_html_long(compare_mode, self.config.project_definition.ws_hints).unwrap_or(String::from("<div>Error</div>")));
                                          }
                                      }
                                  }
                         }
                }
            }
        };

        Ok(String::from(format!("{}", result)))
    }

    pub fn make_json_report(&self) -> Result<String, GenerationError> {
        let mut json: HashMap<String, serde_json::Value> = HashMap::new();
        let mut results: Vec<serde_json::Value> = vec![];
        for tc in self.test_results.iter() {
            results.push(tc.get_json()?);
        }
        json.insert(String::from("testcases"), serde_json::to_value(results).unwrap());
        json.insert(String::from("binary"), serde_json::to_value(self.binary.info.clone()).unwrap());

        serde_json::to_string_pretty(&json).map_err(|_| GenerationError::VgLogParseError)
    }

    pub fn set_verbosity(&mut self, verbose: bool) {
        self.config.project_definition.verbose = verbose;
    }

    pub fn set_diff_mode(&mut self, diff_mode: String) {
        self.config.project_definition.diff_mode = diff_mode;
    }

    pub fn set_protected_mode(&mut self, prot: bool) {
        self.config.project_definition.protected_mode = prot;
    }

    pub fn set_whitespace_hinting(&mut self, hints: bool) {
        self.config.project_definition.ws_hints = hints;
    }
}

