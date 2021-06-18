use difference::Changeset;
use horrorshow::Raw;
use regex::Regex;
use serde_derive::Serialize;
use serde_json::json;
use super::diff::changeset_to_html;
use super::test::TestCaseKind;
use crate::project::binary::GenerationError;

#[derive(Debug)]
pub enum HTMLError {
    None,
}

#[allow(dead_code)]
#[derive(Serialize)]
pub struct TestResult {
    pub kind: TestCaseKind,
    #[serde(skip)]
    pub diff: Option<Changeset>,
    //#[serde(skip)]
    //diff: Option<Vec<diff::Result<String>>>,
    #[serde(skip)]
    pub add_diff: Option<String>,
    pub distance_percentage: Option<f32>,
    pub add_distance_percentage: Option<f32>,
    pub mem_leaks: i32,
    pub mem_errors: i32,
    pub mem_logfile: String,
    pub command_used: String,
    pub used_input: String,
    pub timeout: bool,
    pub ret: Option<i32>,
    pub exp_ret: Option<i32>,
    pub passed: bool,
    pub implemented: Option<bool>,
    pub result: String, // thought about any type?
    pub name: String,
    pub description: String,
    pub number: i32,
    pub protected: bool,
}

impl TestResult {
    pub fn get_json(&self) -> Result<serde_json::Value, GenerationError> {
        Ok(json!({
            "name": self.name,
            "kind": format!("{}",self.kind),
            "passed": self.passed,
            "distance": self.distance_percentage.unwrap_or(-1.0),
            "add_distance": self.add_distance_percentage.unwrap_or(-1.0),
            "implemented": self.implemented.unwrap_or(false),
            "statuscode": self.ret.unwrap_or(0),
            //"diff": format!("{}",self.diff.as_ref().unwrap_or(&Changeset::new("","",""))),
            "vg_warnings": self.mem_leaks,
            "vg_errors": self.mem_errors,
            "timeout": self.timeout,
            "result": self.result.clone(),
            "protected" : self.protected,
        }))
    }

    pub fn get_html_long(&self, compare_mode : &str, with_ws_hints: bool) -> Result<String, GenerationError> {
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

                        @ if self.implemented.is_some() {
                            tr {
                                th {:"Implemented"}
                                td {:format!("{}", if self.implemented.unwrap_or(false) { "yes" } else { "no" })}
                            }
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
                                th{:"Return Value"}
                                td{:Raw(format!("expected: <span class=\"inline-code\">{}</span>, got: <span class=\"inline-code\">{}</span>", self.exp_ret.unwrap_or(-1), self.ret.unwrap_or(-99)))}
                            }
                        }

                        tr {
                            th {:"Memory Usage-Errors / Leaks"}
                            td {:Raw(format!("{} / {} (<a target=\"_blank\" href=\"{}\">Open Log</a>)", self.mem_errors, self.mem_leaks, self.mem_logfile))}
                        }
                    }


                    @ if self.diff.is_some() {
                        |templ| {
                            &mut *templ << Raw(changeset_to_html(&self.diff.as_ref().unwrap(), compare_mode, with_ws_hints, "Output").unwrap_or(String::from(r"<div>Error cannot get changelist</div>")));
                        }
                    }

                    @ if self.add_diff.is_some() {
                        |templ| {
                            &mut *templ << Raw(self.add_diff.clone().unwrap_or(String::from(r"<div>Error cannot get changelist</div>")));
                        }
                    }

                    |templ| {
                        &mut *templ << Raw(format!(
                                "{}",
                                box_html! {
                                    div(id="args") {
                                        table(id="differences") {
                                            |templ| {
                                                let re = Regex::new(r"(?P<m>(?:&middot;|\t|\n|\x00)+)").unwrap();
                                                if with_ws_hints {
                                                    &mut *templ << Raw(format!("<tr><th>Testcase Input</th></tr><tr><td id=\"orig\">{}</td></tr>",
                                                            re.replace_all(&self.used_input.replace(" ", "&middot;"), "<span class=\"whitespace-hint\">${m}</span>")
                                                            .replace("\n", "&#x21b5;<br />")
                                                            .replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;")));
                                                }
                                                else {
                                                    &mut *templ << Raw(format!("<tr><th>Testcase Input</th></tr><tr><td id=\"orig\">{}</td></tr>",
                                                            self.used_input.replace(" ", "&nbsp;").replace("\n", "<br />").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;")));
                                                }
                                            }
                                        }
                                    }
                                }));
                    }
                }
            }
        };
        Ok(String::from(format!("{}", retvar)))
    }

    pub fn get_html_short(&self, protected_mode : bool) -> Result<String, GenerationError> {
        let name = self.name.replace("\"", "");
        let distance = (self.distance_percentage.unwrap_or(1.0) + self.add_distance_percentage.unwrap_or(1.0)) / 2.0;
        let retvar = box_html! {
            tr{
                td{@ if protected_mode && self.protected { i{:"redacted"} } else { :  Raw(format!("<a href=\"#tc-{}\">#{:0>2}:&nbsp;{}</a>", &self.number, &self.number, &name)) }}
                td{:Raw(format!("{}", if self.passed { "<span class=\"success\">&#x2714;</span>" } else { "<span class=\"fail\">&#x2718;</span>" }))}
                td{:format!("{}%", (distance * 1000.0).floor() / 10.0)}
                td{:format!("{}", if self.timeout { "yes" } else { "no" })}
                td{:format!("{}", self.mem_errors)}
                td{:format!("{}", self.mem_leaks)}
                td{@ if self.mem_logfile.is_empty() { : ""} else { : Raw(format!("<a target=\"_blank\" href=\"{}\">Open</a>", &self.mem_logfile ))  } }
            }
        };
        Ok(String::from(format!("{}", retvar)))
    }
}

