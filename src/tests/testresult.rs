use difference::{Changeset, Difference};
use horrorshow::Raw;
use serde_derive::Serialize;
use serde_json::json;
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
    pub distance_percentage: Option<f32>,
    pub vg_warnings: i32,
    pub vg_errors: i32,
    pub vg_logfile: String,
    pub command_used: String,
    pub used_input: String,
    pub timeout: bool,
    pub compile_warnings: Option<String>,
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
            "name":self.name,
            "kind":format!("{}",self.kind),
            "passed":self.passed,
            "implemented":self.implemented.unwrap_or(false),
            "statuscode":self.ret.unwrap_or(0),
            //"diff": format!("{}",self.diff.as_ref().unwrap_or(&Changeset::new("","",""))),
            "vg_warnings": self.vg_warnings,
            "vg_errors": self.vg_errors,
            "timeout": self.timeout,
            "result": self.result.clone(),
            "compile_warnings": self.compile_warnings.clone().unwrap_or(String::from("")),
            "protected" : self.protected,
        }))
    }

    pub fn get_html_long(&self, compare_mode : &str) -> Result<String, GenerationError> {
        let retvar = box_html! {
            div(id="long_report") {
                div(id = "title") {
                    h2 {
                        : Raw(format!("#{} : <a id={}></a>{}<a href=\"#ShortReport\"> (back to summary / short report)</a>", &self.number, &self.name, &self.name))
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
                            th {:"kind"}
                            th {:format!("{}", self.kind)}
                        }
                        tr {
                            th {:"passed"}
                            th {:format!("{}", self.passed)}
                        }

                        @ if self.implemented.is_some() {
                            tr {
                                th {:"implemented"}
                                th {:format!("{}", self.implemented.unwrap_or(false))}
                            }
                        }

                        @ if self.distance_percentage.is_some(){
                            tr {
                                th {:"d in percent"}
                                th {:format!("{}", self.distance_percentage.unwrap_or(0.0))}
                            }
                        }

                        @ if self.exp_ret.is_some(){
                            tr {
                                th{:"return value"}
                                th{:format!("expected : {}, got :{}", self.exp_ret.unwrap_or(-1), self.ret.unwrap_or(-99))}
                            }
                        }
                        @ if self.compile_warnings.is_some(){
                            tr {
                                th {:"compile warnings"}
                                th {:format!("{}", self.compile_warnings.clone().unwrap())}
                            }
                        }
                        tr {
                            th {: Raw(format!("valgrind warnings / errors  (<a href=file://{}>open vg log</a>)", self.vg_logfile)) }
                            th {: Raw(format!("{} / {}", self.vg_warnings, self.vg_errors)) }
                        }
                        tr {
                            th {:"timeout"}
                            th {:format!("{}", self.timeout)}
                        }
                    }


                    @ if self.diff.is_some() {
                        |templ| {
                            &mut *templ << Raw(changeset_to_html(&self.diff.as_ref().unwrap(), compare_mode).unwrap_or(String::from(r"<div>Error cannot get changelist</div>")));
                        }
                    }

                    |templ| {
                        &mut *templ << Raw(format!(
                                "{}",
                                box_html! {
                                    div(id="args") {
                                        table(id="differences") {
                                            |templ| {
                                                &mut *templ << Raw( format!("<tr><th>command and arguments</th></tr><tr><td id=\"orig\">{}</td></tr>", self.command_used) );

                                            }
                                        }
                                    }
                                }));
                    }

                    |templ| {
                        &mut *templ << Raw(format!(
                                "{}",
                                box_html! {
                                    div(id="args") {
                                        table(id="differences") {
                                            |templ| {
                                                &mut *templ << Raw(format!("<tr><th>testcase input</th></tr><tr><td id=\"orig\">{}</td></tr>", self.used_input.replace("\n", "<br>")));
                                            }
                                        }
                                    }
                                }));
                    }

                    // table(id="args"){
                    //     |templ|
                    //     {
                    //         &mut *templ << Raw( format!("<tr><th>command and arguments</th></tr><tr><td>{}</td></tr>", self.command_used) );
                    //     }
                    // }
                }
            }
        };
        Ok(String::from(format!("{}", retvar)))
    }

    pub fn get_html_short(&self, protected_mode : bool) -> Result<String, GenerationError> {
        let name = self.name.replace("\"", "");
        let retvar = box_html! {
            tr{
                th{@ if protected_mode && self.protected { i{:"redacted"} } else { :  Raw(format!("<a href=#{}>#{} {}</a>", &name, &self.number, &name)) }}
                th{:format!("{}", self.kind)}
                th{:format!("{}", self.passed)}
                th{:format!("{}", self.distance_percentage.unwrap_or(0.0))}
                th{:format!("{}", self.vg_errors)}
                th{:format!("{}", self.vg_warnings)}
                th{:format!("{}", self.timeout)}
                th{@ if self.vg_logfile.is_empty() { : ""} else { : Raw(format!("<a href={}>open vg log</a>", &self.vg_logfile ))  } }
            }
        };
        Ok(String::from(format!("{}", retvar)))
    }
}

pub fn changeset_to_html(changes: &Changeset, compare_mode : &str) -> Result<String, HTMLError>
{
    let line_end = if compare_mode == "\n" { "\n" } else { "" };

    let retvar = format!(
        "{}",
        box_html! {
            div(id="diff") {
                table(id="differences") {
                    |templ| {
                        let mut diffright = String::new();
                        let mut diffleft = String::new();

                        for c in &changes.diffs {
                            match *c {
                                Difference::Same(ref z)=>
                                {
                                    diffright.push_str(&format!("{}{}", z.replace(" ", "&nbsp"), line_end));//
                                    diffleft.push_str(&format!("{}{}", z.replace(" ", "&nbsp"), line_end));//
                                }
                                Difference::Rem(ref z) =>
                                {
                                    diffleft.push_str(&format!("<span id =\"wrong\">{}{}</span>",
                                            z.replace(" ", "&nbsp"), line_end));//z.replace(" ", "&nbsp").replace("\n", "\\n&nbsp<br>"), line_end));
                                }

                                Difference::Add(ref z) =>
                                {
                                    diffright.push_str(&format!("<span id =\"missing\">{}{}</span>",
                                            z.replace(" ", "&nbsp"), line_end));//
                                }

                            }
                        }

                        &mut *templ << Raw(format!("<tr><th>desired output</th><th>your output</th></tr><tr><td id=\"orig\">{}</td><td id=\"edit\">{}</td></tr>",
                                diffleft.replace("\n", "&nbsp<br>").replace("\0", "\\0"),
                                diffright.replace("\n", "&nbsp<br>").replace("\0", "\\0") ));
                    }
                }
            }
    }
    );
    Ok(String::from(format!("{}", retvar)))
}

