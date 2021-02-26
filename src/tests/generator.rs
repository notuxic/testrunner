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
        if !self.binary.info.compiled {
            return Ok(String::from("did not compile.."));
        }

        let mut passed = 0;
        let mut failed = 0;
        for res in self.test_results.iter()
        {
            if res.passed == true
            {
                passed += 1;
            }
            else
            {
                failed += 1;
            }
        }
        let percentage_passed = (passed as f32 / self.test_results.len() as f32) * 100.0;

        let result = html! {
            : doctype::HTML;
            html{
                head{
                    title:"testreport";
                }
                //CSS
                style{
                    : Raw(
                        r"
                            @import url('https://fonts.googleapis.com/css2?family=Roboto:wght@300&display=swap');
                            @import url('https://cdn.jsdelivr.net/npm/hack-font@3.3.0/build/web/hack.css');body{font-family:'Roboto',
                            sans-serif;font-weight:300;color:#222;max-width:100em;margin-left:auto;margin-right:auto}body > h1{
                            text-align:center;font-size:3em;text-transform:capitalize}body > h2{font-size:1.8em;border-bottom:0.1em
                            solid #666;margin-top:4em}table{border-collapse:collapse}tr:hover{background:#eee}th{padding-left:1.5em;
                            padding-right:1.5em}#shortreport{margin-top:3em;margin-left:auto;margin-right:auto}#shortreport tr:first-child
                            th{border-bottom:0.1em solid #222}#shortreport tr:first-child th:nth-child(-n+4){text-transform:capitalize}
                            #shortreport tr:hover:first-of-type{background:initial}#long_report{margin-top:5em}#long_report > div{
                            margin-left:5em;margin-right:5em}#long_report > div#description{margin-left:10em;margin-right:10em}#long_report
                            > div#shortinfo table{margin-left:auto;margin-right:auto}#title > h2{border-bottom:0.1em dashed #444}#shortinfo
                            {margin-top:2em}#shortinfo > table tr th:first-child{text-transform:capitalize}#shortinfo > table
                            th:first-of-type{border-right:0.1em solid #222}table td, table td * {vertical-align: top;horizontal-align: top}
                            #differences{background:#eee;margin-top:3em;padding-left:3em;
                            width:initial}#differences tr:first-of-type{border-bottom:0.1em solid #222;text-transform:capitalize}#differences
                            th{padding:0.5em}#differences td{font-family:'Hack', monospace;font-size:0.8em;padding:0.5em;min-width:82ch;max-width:
                            82ch;word-wrap:anywhere}#differences td:first-child,#differences th:first-child{border-right:0.1em dashed #222}
                            #missing{background-color:yellowgreen}#wrong{background-color:IndianRed}

                        ")
                }
                body{
                    h1 : "testreport";

                         @ if !self.binary.info.compiled {
                             h2 : "Program did not compile, no testcases written"
                         }
                         else {
                             // create short report
                             h2: Raw("<a id=ShortReport></a>Short Report");
                                 h3: {
                                     format!("passed {:?} / {:?} ~ {:?} %  (failed: {:?})", passed, self.test_results.len(), percentage_passed , failed)
                                 };
                                     table(id="shortreport"){
                                         th{
                                             : "name"
                                         }
                                         th{
                                             : "kind"
                                         }
                                         th{
                                             : "passed"
                                         }
                                         th{
                                             :"percentage"
                                         }
                                         th{
                                             :"vg_errors"
                                         }
                                         th{
                                             :"vg_warnings"
                                         }
                                         th{
                                             :"timeout"
                                         }
                                         th{
                                             :"valgrind log"
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
                                     h2 : "Detailed Report";

                                          |templ| {
                                              for tc in self.test_results.iter() {
                                                  if !(protected_mode && tc.protected) {
                                                      &mut *templ << Raw(tc.get_html_long(compare_mode).unwrap_or(String::from("<div>Error</div>")));
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
        let mut results: Vec<serde_json::Value> = vec![];
        for tc in self.test_results.iter() {
            results.push(tc.get_json()?);
        }

        serde_json::to_string_pretty(&results).map_err(|_| GenerationError::VgLogParseError)
    }

    pub fn set_verbosity(&mut self, verbose: bool) {
        self.config.project_definition.verbose = verbose;
    }

    pub fn set_diff_mode(&mut self, diff_mode: String) {
        self.config.project_definition.diff_mode = diff_mode;
    }
}

