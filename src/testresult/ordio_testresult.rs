use std::sync::Weak;

use horrorshow::Raw;
use serde_derive::Serialize;
use serde_json::json;

use crate::project::definition::ProjectDefinition;
use crate::test::diff::iodiff_to_html;
use crate::test::ordio_test::IODiff;
use crate::test::test::TestcaseType;
use crate::testrunner::{TestrunnerError, TestrunnerOptions};
use super::testresult::Testresult;


#[derive(Serialize)]
pub struct OrdIoTestresult {
    pub kind: TestcaseType,
    pub number: i32,
    pub name: String,
    pub description: String,
    pub protected: bool,
    #[serde(skip)]
    pub add_diff: Option<String>,
    #[serde(skip)]
    pub io_diff: Option<Vec<IODiff>>,
    pub distance_percentage: Option<f32>,
    pub add_distance_percentage: Option<f32>,
    pub truncated_output: bool,
    pub mem_leaks: i32,
    pub mem_errors: i32,
    pub mem_logfile: String,
    pub command_used: String,
    pub timeout: bool,
    pub ret: Option<i32>,
    pub exp_ret: Option<i32>,
    pub passed: bool,
    pub input: String,
    pub output: String, // thought about any type?
    #[serde(skip)]
    pub project_definition: Weak<ProjectDefinition>,
    #[serde(skip)]
    pub options: Weak<TestrunnerOptions>,
}

impl Testresult for OrdIoTestresult {
    fn get_testcase_type(&self) -> TestcaseType {
        TestcaseType::OrdIOTest
    }

    fn passed(&self) -> bool {
        self.passed
    }

    fn protected(&self) -> bool {
        self.protected
    }

    fn get_json_entry(&self) -> Result<serde_json::Value, TestrunnerError> {
        Ok(json!({
            "name": self.name,
            "kind": format!("{}",self.kind),
            "passed": self.passed,
            "distance": self.distance_percentage.unwrap_or(-1.0),
            "add_distance": self.add_distance_percentage.unwrap_or(-1.0),
            "statuscode": self.ret.unwrap_or(0),
            "mem_leaks": self.mem_leaks,
            "mem_errors": self.mem_errors,
            "timeout": self.timeout,
            "output": self.output.clone(),
            "protected" : self.protected,
        }))
    }

    fn get_html_entry_detailed(&self) -> Result<String, TestrunnerError> {
        let options = self.options.upgrade().unwrap();

        let retvar = box_html! {
            div(id="long_report") {
                div(id = "title") {
                    h2 {
                        : Raw(format!("#{:0>2}:&nbsp;<a id=\"tc-{}\"></a>{} <a class=\"link-summary\" href=\"#summary\">(back to summary)</a>", &self.number, &self.number, &self.name))
                    }
                }
                div(id="description") {
                    p {
                        : self.description.clone()
                    }
                }
                div(id="shortinfo") {
                    table {
                        tr {
                            th {:"Type"}
                            td {:format!("{}", self.kind)}
                        }
                        tr {
                            th {:"Passed"}
                            td {:Raw(format!("{}", if self.passed { "<span class=\"success\">&#x2714;</span>" } else { "<span class=\"fail\">&#x2718;</span>" }))}
                        }

                        @ if self.distance_percentage.is_some(){
                            tr {
                                th {:"Output-Diff"}
                                td {:format!("{}%", (self.distance_percentage.unwrap_or(0.0) * 1000.0).floor() / 10.0)}
                            }
                        }

                        @ if self.add_distance_percentage.is_some(){
                            tr {
                                th {:"File-Diff"}
                                td {:format!("{}%", (self.add_distance_percentage.unwrap_or(0.0) * 1000.0).floor() / 10.0)}
                            }
                        }

                        tr {
                            th {:"Timeout"}
                            td {:format!("{}", if self.timeout { "yes" } else { "no" })}
                        }

                        @ if self.exp_ret.is_some(){
                            tr {
                                th{:"Commandline"}
                                td{:Raw(format!("<span class=\"inline-code\">{}</span>", self.command_used))}
                            }
                            tr {
                                th{:"Exit Code"}
                                td{:Raw(format!("expected: <span class=\"inline-code\">{}</span>, got: <span class=\"inline-code\">{}</span>", self.exp_ret.unwrap_or(-1), self.ret.unwrap_or(-99)))}
                            }
                        }

                        tr {
                            th {:"Memory Usage-Errors / Leaks"}
                            @ if options.protected_mode && self.protected {
                                td {:Raw(format!("{} / {}", self.mem_errors, self.mem_leaks))}
                            }
                            else {
                                td {:Raw(format!("{} / {} (<a target=\"_blank\" href=\"{}\">Open Log</a>)", self.mem_errors, self.mem_leaks, self.mem_logfile))}
                            }
                        }
                    }

                    @ if self.truncated_output {
                        div(id="failed") {
                            span(class="warning") {:"Your output has been truncated, as it is a lot longer than the reference output!"}
                        }
                    }

                    @ if self.io_diff.is_some() {
                        |templ| {
                            &mut *templ << Raw(iodiff_to_html(&self.io_diff.as_ref().unwrap(), &options.diff_delim, options.ws_hints, "Output").unwrap_or(r"<div>Error cannot get changelist</div>".to_owned()));
                        }
                    }

                    @ if self.add_diff.is_some() {
                       |templ| {
                            &mut *templ << Raw(self.add_diff.clone().unwrap_or(r"<div>Error cannot get changelist</div>".to_owned()));
                        }
                    }
                }
            }
        };
        Ok(format!("{}", retvar))
    }

    fn get_html_entry_summary(&self, protected_mode: bool) -> Result<String, TestrunnerError> {
        let name = self.name.replace("\"", "");
        let distance = if self.add_distance_percentage.is_some() {
            (self.distance_percentage.unwrap_or(-1.0) + self.add_distance_percentage.unwrap_or(-1.0)) / 2.0
        }
        else {
            self.distance_percentage.unwrap_or(-1.0)
        };

        let retvar = box_html! {
            tr{
                td{@ if protected_mode && self.protected { i{:"redacted"} } else { :  Raw(format!("<a href=\"#tc-{}\">#{:0>2}:&nbsp;{}</a>", &self.number, &self.number, &name)) }}
                td{:Raw(format!("{}", if self.passed { "<span class=\"success\">&#x2714;</span>" } else { "<span class=\"fail\">&#x2718;</span>" }))}
                td{:format!("{}%", (distance * 1000.0).floor() / 10.0)}
                td{:format!("{}", if self.ret.unwrap_or(-99) == self.exp_ret.unwrap_or(-1) { "correct" } else { "incorrect" })}
                td{:format!("{}", if self.timeout { "yes" } else { "no" })}
                td{:format!("{}", self.mem_errors)}
                td{:format!("{}", self.mem_leaks)}
                td{@ if self.mem_logfile.is_empty() || (protected_mode && self.protected) { : ""} else { : Raw(format!("<a target=\"_blank\" href=\"{}\">Open</a>", &self.mem_logfile ))  } }
            }
        };
        Ok(format!("{}", retvar))
    }
}

