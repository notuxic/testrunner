use std::collections::{HashMap, BTreeMap};
use std::fs::read_to_string;
use std::sync::Arc;

use horrorshow::Raw;
use horrorshow::helper::doctype;
use serde::{Deserializer, Deserialize};
use serde_derive::Deserialize;
use serde_tagged::de::BoxFnSeed;
use thiserror::Error;

use crate::project::binary::{Binary, CompileError};
use crate::project::definition::ProjectDefinition;
use crate::test::io_test::IoTest;
use crate::test::ordio_test::OrdIoTest;
use crate::test::test::{Test, TestingError};
use crate::testresult::testresult::Testresult;


#[derive(Debug, Error)]
pub enum TestrunnerError {
    #[error("config not found: {0}")]
    ConfigNotFound(String),
    #[error("failed parsing config: {0}")]
    ConfigParseError(String),
    #[error(transparent)]
    CompileError(#[from] CompileError),
    #[error(transparent)]
    TestingError(#[from] TestingError),
    #[error("error generating report: {}", .0.to_string())]
    GenerationError(Box<dyn std::error::Error>),
}

#[derive(Debug)]
pub struct TestrunnerOptions {
    pub verbose: bool,
    pub protected_mode: bool,
    pub ws_hints: bool,
    pub sudo: Option<String>,
}

impl Default for TestrunnerOptions {
    fn default() -> Self {
        TestrunnerOptions {
            verbose: false,
            protected_mode: false,
            ws_hints: true,
            sudo: None,
        }
    }
}

#[derive(Deserialize)]
pub struct Testrunner {
    #[serde(deserialize_with = "Testrunner::deserialize_definition")]
    project_definition: Arc<ProjectDefinition>,
    testcases: Vec<Box<dyn Test + Send + Sync>>,
    #[serde(skip)]
    testresults: Vec<Box<dyn Testresult + Send + Sync>>,
    #[serde(skip)]
    binary: Arc<Binary>,
    #[serde(skip)]
    options: Arc<TestrunnerOptions>,
}

impl Testrunner {
    pub fn from_file(path: &str, options: TestrunnerOptions) -> Result<Self, TestrunnerError> {
        let config = read_to_string(path).map_err(|_| TestrunnerError::ConfigNotFound(path.to_string()))?;
        let mut runner: Self = toml::from_str(&config).map_err(|err| TestrunnerError::ConfigParseError(err.to_string()))?;
        runner.options = Arc::new(options);
        runner.binary = Arc::new(Binary::from_project_definition(&runner.project_definition)?);

        let mut tc_number = 0;
        let project_definition = Arc::downgrade(&runner.project_definition);
        let options = Arc::downgrade(&runner.options);
        let binary = Arc::downgrade(&runner.binary);
        runner.testcases.iter_mut().try_for_each(|tc| {
            tc_number += 1;
            tc.init(tc_number, project_definition.clone(), options.clone(), binary.clone())
        })?;
        Ok(runner)
    }

    pub fn deserialize_definition<'de, D>(deserializer: D) -> Result<Arc<ProjectDefinition>, D::Error>
        where D: Deserializer<'de>
    {
        return Ok(Arc::new(ProjectDefinition::deserialize(deserializer)?));
    }

    pub fn run_tests(&mut self) -> Result<(), TestrunnerError> {
        if !self.binary.info.compiled {
            println!("Compilation failed, skipping tests!");
            return Ok(());
        }

        self.testresults = match self.testcases.iter().try_fold(Vec::with_capacity(self.testcases.len()), |mut acc, tc| {
            acc.push(tc.run()?);
            Ok(acc)
        }) {
            Ok(results) => results,
            Err(err) => return Err(err),
        };
        println!("\nPassed testcases: {} / {}", self.testresults.iter().filter(|test| test.passed()).count(), self.testresults.len());
        Ok(())
    }

    pub fn generate_html_report(&self, protected_mode: bool) -> Result<String, TestrunnerError> {
        let compiler_output = self.binary.info.errors.clone().unwrap_or("<i>failed fetching compiler output!</i>".to_owned());
        let tc_all_num = self.testresults.len();
        let mut tc_all_passed = 0;
        let mut tc_public_num = 0;
        let mut tc_public_passed = 0;
        let mut tc_private_num = 0;
        let mut tc_private_passed = 0;
        self.testresults.iter().for_each(|tc| {
            if tc.protected() {
                tc_private_num += 1;
                if tc.passed() {
                    tc_private_passed += 1;
                    tc_all_passed += 1;
                }
            }
            else {
                tc_public_num += 1;
                if tc.passed() {
                    tc_public_passed += 1;
                    tc_all_passed += 1;
                }
            }
        });

        let result = html! {
            : doctype::HTML;
            html{
                head{
                    title:"Testreport";
                    meta(charset="UTF-8");
                }
                //CSS
                style{
                    : Raw(
                        format!(r#"
                            @import url('https://fonts.googleapis.com/css2?family=Roboto:wght@300&display=swap');
                            @import url('https://cdn.jsdelivr.net/npm/hack-font@3.3.0/build/web/hack.css');
                            body {{
                                font-family: 'Roboto', sans-serif;
                                font-weight: 300;
                                color: #222;
                                max-width: 100em;
                                margin-left: auto;
                                margin-right: auto
                            }}
                            body > h1 {{
                                text-align: center;
                                font-size: 3em
                            }}
                            body > h2 {{
                                font-size: 1.8em;
                                border-bottom: 0.1em solid #666;
                                margin-top: 4em
                            }}
                            table {{
                                border-collapse: collapse
                            }}
                            tr:hover {{
                                background: #eee
                            }}
                            th {{
                                text-align: right
                            }}
                            th, td {{
                                padding-left: 1em;
                                padding-right: 1em
                            }}
                            a {{
                                text-decoration: none;
                            }}
                            #shortreport {{
                                margin-top: 3em;
                                margin-left: auto;
                                margin-right: auto
                            }}
                            #shortreport th, #shortreport td {{
                                text-align: center
                            }}
                            #shortreport td:first-child {{
                                text-align: left;
                            }}
                            #shortreport tr:first-child th {{
                                border-bottom: 0.1em solid #222
                            }}
                            #shortreport tr:hover:first-of-type {{
                                background: initial
                            }}
                            #long_report {{
                                margin-top: 5em
                            }}
                            #long_report > div {{
                                margin-left: 5em;
                                margin-right: 5em
                            }}
                            #long_report > div#description {{
                                margin-left: 10em;
                                margin-right: 10em
                            }}
                            #title > h2 {{
                                display: flex;
                                border-bottom: 0.1em dashed #444
                            }}
                            #shortinfo {{
                                margin-left: auto;
                                margin-right: auto;
                                margin-top: 2em
                            }}
                            div#shortinfo table {{
                                margin-left: auto;
                                margin-right: auto
                            }}
                            #shortinfo > table th:first-of-type {{
                                border-right: 0.1em solid #222
                            }}
                            table td, table td * {{
                                vertical-align: top;
                                horizontal-align: top
                            }}
                            #differences {{
                                background: #eee;
                                margin-top: 3em;
                                padding-left: 3em;
                                width: initial
                            }}
                            #differences tr:first-of-type {{
                                border-bottom: 0.1em solid #222
                            }}
                            #differences th {{
                                text-align: center;
                                padding: 0.5em
                            }}
                            #differences td {{
                                font-family: 'Hack', monospace;
                                font-size: 0.82em;
                                padding: 0.5em;
                                min-width: {}ch;
                                max-width: {}ch;
                                word-wrap: anywhere;
                                word-break: break-all
                            }}
                            #differences #compiler {{
                                min-width: 122ch;
                                max-width: 122ch;
                            }}
                            #differences td:nth-child(2), #differences th:nth-child(2) {{
                                border-left: 0.1em dashed #222
                            }}
                            #diff-add {{
                                background-color: #9acd32b8
                            }}
                            #diff-remove {{
                                background-color: #cd5c5cb0
                            }}
                            #diff-add-inline {{
                                background-color: #87c608
                            }}
                            #diff-remove-inline {{
                                background-color: IndianRed
                            }}
                            #diff-input {{
                                text-decoration: underline;
                                text-decoration-color: #222;
                                color: #666
                            }}
                            #diff-input-unsent {{
                                text-decoration: underline;
                                text-decoration-color: #222;
                                background-color: turquoise;
                                color: #222
                            }}
                            .inline-code {{
                                background: #eee;
                                font-family: 'Hack', monospace;
                                font-size: 0.84em;
                                font-weight: 300;
                                vertical-align: baseline;
                            }}
                            .link-summary {{
                                display: inline-block;
                                font-size: 0.8em;
                                font-weight: normal;
                                vertical-align: baseline;
                                margin-left: auto;
                            }}
                            .whitespace-hint {{
                                color: #bbb
                            }}
                            #diff-add .whitespace-hint {{
                                color: green
                            }}
                            #diff-remove .whitespace-hint {{
                                color: darkred
                            }}
                            #diff-input-unsent .whitespace-hint {{
                                color: darkcyan
                            }}
                            #failed {{
                                width: 61em;
                                margin-top: 5em;
                                margin-left: auto;
                                margin-right: auto
                            }}
                            .warning {{
                                font-size: large;
                                background-color: #ff000033;
                                color: darkred;
                                padding: 0.5em;
                                border-left: darkred 0.4em solid
                            }}
                            .success {{
                                color: green;
                                font-family: 'Hack', monospace;
                            }}
                            .fail {{
                                color: darkred;
                                font-family: 'Hack', monospace;
                            }}
                            #flex-container {{
                                display: flex;
                                flex-direction: row;
                                justify-content: center;
                                align-items: center
                            }}
                        "#, self.project_definition.diff_table_width.unwrap_or(82), self.project_definition.diff_table_width.unwrap_or(82)))
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
                                         : "Passed"
                                     }
                                     th{
                                         : "Diff"
                                     }
                                     th{
                                         : "Exit Code"
                                     }
                                     th{
                                         : "Timeout"
                                     }
                                     th{
                                         : "Mem Usage Errors"
                                     }
                                     th{
                                         : "Mem Leaks"
                                     }
                                     th{
                                         : "Mem Analyzer Log"
                                     }
                                     |templ| {
                                         for tc in self.testresults.iter() {
                                             match tc.get_html_entry_summary(protected_mode) {
                                                 Ok(res)=> {
                                                     &mut *templ << Raw(res);
                                                 }
                                                 Err(_err) => {
                                                     &mut *templ << Raw("<tr><td></td><td></td><td></td><td></td><td></td><td></td><td></td></tr>".to_owned())
                                                 }
                                             }
                                         }

                                     }
                                 }
                             h2 : "Testcases";

                                  |templ| {
                                      for tc in self.testresults.iter() {
                                          if !(protected_mode && tc.protected()) {
                                              &mut *templ << Raw(tc.get_html_entry_detailed().unwrap_or("<div>Error</div>".to_owned()));
                                          }
                                      }
                                  }
                         }
                }
            }
        };

        Ok(format!("{}", result))
    }

    pub fn generate_json_report(&self) -> Result<String, TestrunnerError> {
        let mut json: HashMap<String, serde_json::Value> = HashMap::new();
        let mut results: Vec<serde_json::Value> = vec![];
        for tc in self.testresults.iter() {
            results.push(tc.get_json_entry()?);
        }
        json.insert("testcases".to_owned(), serde_json::to_value(results).unwrap());
        json.insert("binary".to_owned(), serde_json::to_value(&self.binary.info).unwrap());

        serde_json::to_string_pretty(&json).map_err(|err| TestrunnerError::GenerationError(Box::new(err)))
    }
}

impl<'de> Deserialize<'de> for Box<dyn Test + Send + Sync> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        serde_tagged::de::internal::deserialize(deserializer, "type", get_deserializer_registry())
    }
}

pub type DeserializerRegistry = BTreeMap<&'static str, BoxFnSeed<Box<dyn Test + Send + Sync>>>;
pub fn get_deserializer_registry() -> &'static DeserializerRegistry {
    lazy_static! {
        static ref DESERIALIZER_REGISTRY: DeserializerRegistry = {
            let mut registry = BTreeMap::new();
            registry.insert("IO", BoxFnSeed::new(IoTest::deserialize_trait::<dyn erased_serde::Deserializer>));
            registry.insert("OrdIO", BoxFnSeed::new(OrdIoTest::deserialize_trait::<dyn erased_serde::Deserializer>));
            registry
        };
    }
    &DESERIALIZER_REGISTRY
}

