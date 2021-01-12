use serde_derive::{Deserialize, Serialize};
use serde_json::json;
#[macro_use]
extern crate horrorshow;
use horrorshow::helper::doctype;
use horrorshow::Raw;
use regex::Regex;
use std::fmt;
use std::fs::{create_dir_all, read_to_string, write};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use std::vec;
extern crate clap;
use clap::{App, Arg};
use colored::*;
use serde::export::Formatter;
use wait_timeout::ChildExt;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use difference::{Changeset, Difference};

mod unit_test;

static mut COMPARE_MODE :  [&'static str; 1] = ["\n"];
static mut VERBOSE : bool = false;
static NEWLINE : &str = "\n";

static RET_TIMEOUT : i32 = -99;

#[derive(Debug, Clone, Copy, Serialize)]
pub enum TestCaseKind {
    UnitTest,
    IOTest,
}
impl fmt::Display for TestCaseKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}

#[allow(dead_code)]
pub struct TestMeta {
    number: i32,
    name: String,
    desc: Option<String>,
    timeout: Option<i32>,
    projdata: ProjectData, // use lifetime ref?
    kind: TestCaseKind,
    protected: bool,
}

#[allow(dead_code)]
pub struct UnitTest {
    meta: TestMeta,
    subname: String,
    fname: String,
    argv: String,
}

#[allow(dead_code)]
#[derive(Serialize)]
pub struct TestResult {

    kind: TestCaseKind,
    #[serde(skip)]
    diff: Option<Changeset>,
    //#[serde(skip)]
    //diff: Option<Vec<diff::Result<String>>>,
    distance_percentage: Option<f32>,
    vg_warnings: i32,
    vg_errors: i32,
    vg_logfile: String,
    command_used: String,
    used_input: String,
    timeout: bool,
    compile_warnings: Option<String>,
    ret: Option<i32>,
    exp_ret: Option<i32>,
    passed: bool,
    implemented: Option<bool>,
    result: String, // thought about any type?
    name: String,
    description: String,
    number: i32,
    protected: bool,
}

impl TestResult {
    // add code here
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
            div(id="long_report"){
                div(id = "title"){
                    h2{
                        : Raw(format!("#{} : <a id={}></a>{}<a href=\"#ShortReport\"> (back to summary / short report)</a>",&self.number, &self.name, &self.name))
                    }
                }
                div(id="description"){
                    p{
                        : self.description.clone()
                    }
                }
                div(id="shortinfo")
                {
                    table {
                        tr{
                            th{:"kind"}
                            th{:format!("{}",self.kind)}
                        }
                        tr{
                            th{:"passed"}
                            th{:format!("{}",self.passed)}
                        }

                        @ if self.implemented.is_some() {
                            tr{
                                th {:"implemented"}
                                th {:format!("{}",self.implemented.unwrap_or(false))}
                            }
                        }

                        @ if self.distance_percentage.is_some(){
                            tr{
                                th {:"d in percent"}
                                th {:format!("{}",self.distance_percentage.unwrap_or(0.0))}
                            }
                        }

                        @ if self.exp_ret.is_some(){
                            tr{
                                th{:"return value"}
                                th{:format!("expected : {}, got :{}",self.exp_ret.unwrap_or(-1),self.ret.unwrap_or(RET_TIMEOUT))}
                            }
                        }
                        @ if self.compile_warnings.is_some(){
                            tr {
                                th{:"compile warnings"}
                                th{:format!("{}",self.compile_warnings.clone().unwrap())}
                            }
                        }
                        tr{
                            th{: Raw(format!("valgrind warnings / errors  (<a href=file://{}>open vg log</a>)", self.vg_logfile)) }
                            th{: Raw(format!("{} / {}",self.vg_warnings,self.vg_errors)) }
                        }
                        tr{
                            th{:"timeout"}
                            th{:format!("{}",self.timeout)}
                        }                        
                    }


                    @ if self.diff.is_some(){
                        |templ|
                        {
                            &mut *templ << Raw (  changeset_to_html(  &self.diff.as_ref().unwrap(), compare_mode  ).unwrap_or(String::from(r"<div>Error cannot get changelist</div>"))      );

                        }
                    }
                    |templ|{
                    &mut *templ << Raw( format!(
                                    "{}",
                                    box_html! {
                                        div(id="args"){
                                            table(id="differences"){
                                                |templ|
                                                {
                                                    &mut *templ << Raw( format!("<tr><th>command and arguments</th></tr><tr><td id=\"orig\">{}</td></tr>", self.command_used) );

                                                }
                                            }
                                        }
                                    }
                    ));
                    }

                    |templ|{
                    &mut *templ << Raw( format!(
                                    "{}",
                                    box_html! {
                                        div(id="args"){
                                            table(id="differences"){
                                                |templ|
                                                {
                                                    &mut *templ << Raw( format!("<tr><th>testcase input</th></tr><tr><td id=\"orig\">{}</td></tr>", self.used_input.replace("\n", "<br>")) );
                                                }
                                            }
                                        }
                                    }
                    ));
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
                th{:format!("{}",self.kind)}
                th{:format!("{}",self.passed)}
                th{:format!("{}",self.distance_percentage.unwrap_or(0.0))}
                th{:format!("{}",self.vg_errors)}
                th{:format!("{}",self.vg_warnings)}
                th{:format!("{}",self.timeout)}
                th{@ if self.vg_logfile.is_empty() { : ""} else { : Raw(format!("<a href=file://{}>open vg log</a>", &self.vg_logfile ))  } }
            }
        };
        Ok(String::from(format!("{}", retvar)))
    }
}

trait Test {
    fn run(&self) -> Result<TestResult, GenerationError>;
    fn from_saved_tc(
        number: i32,
        testcase: &SavedTestcase,
        projdata: &ProjectData,
        binary: Option<&Binary>,
    ) -> Result<Self, GenerationError>
    where
        Self: Sized;
    //fn report(&self) -> Result<String,GenerationError>;
}

#[derive(Debug)]
pub enum HTMLError {
    None,
}

pub fn percentage_from_levenstein(steps: i32, source: &String, target: &String) -> f32 {
    if (source.len() == 0) || (target.len() == 0) {
        return 0.0;
    } else {
        return 1.0 - ((steps as f32) / (source.len() as f32).max(target.len() as f32));
    }
}

#[allow(dead_code)]
pub struct TestCaseGenerator {
    test_cases: Vec<Box<dyn Test + Send + Sync>>,
    test_results: Vec<TestResult>,
    binary: Binary,
    config: TestDefinition,
}
impl TestCaseGenerator {
    pub fn form_string(s: &String) -> Result<Self, GenerationError> {
        let config: TestDefinition = match toml::from_str(s) {
            Ok(c) => c,
            Err(err) => {
                println!("{}", err);
                return Err(GenerationError::ConfigErrorIO);
            }
        };

        let binaray: Binary = match Binary::from_project_data(&config.project_data) {
            Ok(content) => content,
            Err(err) => {
                println!("{:?}", err);
                return Err(GenerationError::CouldNotMakeBinary);
            }
        };
        Ok(TestCaseGenerator {
            config,
            binary: binaray,
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
                        UnitTest::from_saved_tc(n, tc, &self.config.project_data, None).unwrap();
                    self.test_cases.push(Box::new(unit_test));
                }
                "IO" => {
                    let io_test =
                        IoTest::from_saved_tc(n, tc, &self.config.project_data, Some(&self.binary))
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

                    @ if !self.binary.info.compiled{
                        h2 : "Program did not compile, no testcases written"
                    }
                    else{
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
                            |templ|{
                                for tc in self.test_results.iter()
                                {
                                    match tc.get_html_short(protected_mode){
                                        Ok(res)=>{
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
                            for tc in self.test_results.iter()
                            {
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
}

pub fn changeset_to_html(changes: &Changeset, compare_mode : &str) -> Result<String, HTMLError> 
{
    let mut line_end = "";
    if compare_mode.eq(NEWLINE)
    {
        line_end = "\n";
    }

    let retvar = format!(
        "{}",
        box_html! {
            div(id="diff"){
                table(id="differences"){
                    |templ|{
                            let mut diffright = String::new();
                            let mut diffleft = String::new();


                            for c in &changes.diffs
                            {
                                match *c 
                                {
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

pub fn parse_vg_log(filepath: &String) -> Result<(i32, i32), GenerationError> {
    let re = Regex::new(r"ERROR SUMMARY: (?P<err>[0-9]+) errors? from (?P<warn>[0-9]+) contexts?")
        .unwrap();
    let mut retvar = (-1, 1);
    match read_to_string(filepath) {
        Ok(content) => match re.captures_iter(&content).last() {
            Some(cap) => {
                retvar.0 = cap["warn"].parse().unwrap_or(-1);
                retvar.1 = cap["err"].parse().unwrap_or(-1);
                return Ok(retvar);
            }
            None => {
                return Err(GenerationError::VgLogParseError);
            }
        },
        Err(err) => {
            println!("Cannot open vglog :{}\n{}", filepath, err);
            return Err(GenerationError::VgLogNotFound);
        }
    }
}

impl Test for UnitTest {
    fn run(&self) -> Result<TestResult, GenerationError> {
        if let Err(e) = unit_test::run(self) {
            println!("Error running unit test: {}", e);
            return Err(GenerationError::ConfigErrorUnit);
        }

        Ok(TestResult {
            //diff2 : None,
            distance_percentage: None,
            compile_warnings: None,
            kind: TestCaseKind::UnitTest,
            diff: None,
            implemented: None,
            passed: false,
            result: String::from("Not yet implemented"),
            ret: None,
            exp_ret: None,
            vg_errors: 0,
            vg_warnings: 0,
            vg_logfile: String::from(""),
            command_used: String::from(format!("./{} {}", &self.meta.projdata.project_name, &self.argv)),
            used_input: String::from(""),
            timeout: false,
            name: self.meta.name.clone(),
            description: self.meta.desc.clone().unwrap_or(String::from("")),
            number: self.meta.number,
            protected: self.meta.protected,
        })
    }
    fn from_saved_tc(
        number: i32,
        testcase: &SavedTestcase,
        projdata: &ProjectData,
        _binary: Option<&Binary>,
    ) -> Result<Self, GenerationError> {
        let retvar = UnitTest {
            meta: TestMeta {
                number,
                name: testcase.name.clone(),
                desc: testcase.description.clone(),
                timeout: testcase.timeout,
                projdata: projdata.clone(),
                kind: TestCaseKind::UnitTest,
                protected: testcase.protected.unwrap_or(false),
            },
            fname: testcase.fname.as_ref().unwrap_or(&String::new()).clone(),
            argv: testcase.args.as_ref().unwrap_or(&String::new()).clone(),
            subname: testcase
                .subname
                .as_ref()
                .map(|s| s.clone())
                .unwrap_or(String::new()),
        };
        Ok(retvar)
    }
}
#[allow(dead_code)]
pub struct IoTest {
    meta: TestMeta,
    in_file: String,
    exp_file: String,
    in_string: String,
    exp_string: String,
    binary: Binary,
    argv: String,
    exp_retvar: Option<i32>,
    env_vars: Option<String>,
}

#[derive(Debug)]
pub enum ExecuteError {
    ProcessDidntStart,
    Timeout,
    None,
}

fn command_timeout(cmd: Child, timeout: i32, number: i32) -> (String, Option<i32>) {
    let mut cmd = cmd;

    let mut output = String::new();
    let mut _retvar = Some(RET_TIMEOUT);
    let mut tmp : Vec<u8> =  Vec::new(); 

    match cmd.wait_timeout(Duration::from_secs(timeout as u64) ).unwrap() {
        Some(expr) =>
        {
            _retvar = Some(expr.code().unwrap_or(RET_TIMEOUT));
        }
        None => {
            _retvar = None;

            println!("killing {} beacause of timeout", number);
            cmd.kill().expect("Upps, can't kill this one");
        }
    }

    cmd.stdout.as_mut().unwrap().read_to_end(&mut tmp).expect("could not read stdout");
    output = format!("{}{}", output, String::from_utf8_lossy(&tmp));

    return (output, _retvar);

}

impl Test for IoTest {
    fn run(&self) -> Result<TestResult, GenerationError> {
        println!("starting testcase {}", self.meta.name);
        // project name is the binary name
        // argvs
        //get stdin text!
        let mut stdinstring: String = String::new();
        if !self.in_file.is_empty() {
            match read_to_string(&self.in_file) {
                Ok(content) => {
                    stdinstring.clone_from(&content);
                }
                Err(err) => {
                    println!("Cannot open stdinfile, fallback to none \n {:?}", err);
                }
            }
        } else if !self.in_string.is_empty() {
            stdinstring.clone_from(&self.in_string);
        }
        let envs: Vec<(String, String)> = match &self.env_vars {
            Some(var_string) => {
                let mut splits: Vec<(String, String)> = Vec::new();
                for split in var_string.split(",") {
                    if split.contains("=") {
                        let mut m = split.splitn(2, "=");
                        splits.push((
                            m.next().unwrap().clone().to_string(),
                            m.next().unwrap().clone().to_string(),
                        ));
                    } else {
                        splits.push((String::from(split), String::new()));
                    }
                }
                splits
            }
            None => Vec::new(),
        };
        // same for expected stdout
        let mut stdoutstring: String = String::new();
        if !self.exp_file.is_empty() {
            match read_to_string(&self.exp_file) {
                Ok(content) => {
                    stdoutstring = content;
                }
                Err(err) => {
                    println!("Cannot open stdout, fallback to none \n {:?}", err);
                }
            }
        } else if !self.exp_string.is_empty() {
            stdoutstring = self.exp_string.clone();
        }
        //println!("stdoutstring = {}", stdoutstring);
        // create temp folder

        let mut vg_flags = self.meta.projdata.valgrind_flags.as_ref()
                                    .unwrap_or(&vec!["--leak-check=full".to_string(), "--track-origins=yes".to_string() ] ).clone();


        let exe_path = match std::env::current_dir()
        {
            Ok(t) => String::from(t.into_os_string().into_string().unwrap_or(String::new())),
        
            Err(_e) =>  String::from(""),
        };


        //todo make absolute pathfinding somehow better (abspath required for html)
        let tmp_vg_path =  self.meta.projdata.makefile_path.clone().unwrap_or(String::from(".")).clone();

        let mut _vg_filepath: String = String::new();

        if tmp_vg_path != "."
        {
            create_dir_all(format!("{}/valgrind_logs/{}", &self.meta.projdata.makefile_path.clone().unwrap_or(String::from(".")) , 
                                        &self.meta.number) ).expect("could not create tmp folder");
            _vg_filepath = format!("./valgrind_logs/{}/vg_log.txt", &self.meta.number);
            
        }
        else
        {
            create_dir_all(format!("{}/valgrind_logs/{}", exe_path, &self.meta.number) ).expect("could not create tmp folder");
            _vg_filepath = format!("{}/valgrind_logs/{}/vg_log.txt", exe_path, &self.meta.number);
        }

        // let vg_flags = match self.meta.projdata.valgrind_flags
        // {
        //     Some(to) => to,
        //     None => vec![String::from("--leak-check=full")], // default is 10 sec
        // };

        // // run assignment file compiled with fsanitize
        // let mut run_cmd = Command::new(format!("./{}", &self.meta.projdata.project_name))
        //     //assuming makefile_path = project path
        //     .current_dir(
        //         &self
        //             .meta
        //             .projdata
        //             .makefile_path
        //             .as_ref()
        //             .unwrap_or(&String::from("./")),
        //     )
        //     .args([
        //         &self.argv,
        //     ].iter().filter(|s| !s.is_empty()))
        //     .stdin(Stdio::piped())
        //     .stdout(Stdio::piped())
        //     .stderr(Stdio::piped())
        //     .envs(envs)
        //     .spawn()
        //     .expect("could not spawn process");

        let starttime = Instant::now();



        let vg_filepath2 = _vg_filepath.clone();

        vg_flags.push(format!("--log-file={}", &vg_filepath2 ));
        vg_flags.push(format!("./{}", &self.meta.projdata.project_name));
        vg_flags.push(self.argv.clone());


        let mut run_cmd = Command::new("valgrind")
            // run valgrind with the given program name
            //assuming makefile_path = project path
            .current_dir(
                &self
                    .meta
                    .projdata
                    .makefile_path
                    .as_ref()
                    .unwrap_or(&String::from("./")),
                    
            )
            .args(vg_flags.iter().filter(|s| !s.is_empty()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(envs)
            .spawn()
            .expect("could not spawn process");

        if !stdinstring.is_empty() {
            let stdin = run_cmd.stdin.as_mut().expect("failed to get stdin");
            stdin
                .write_all(&stdinstring.clone().into_bytes())
                .expect("could not send input");
        }

        let global_timeout = match self.meta.projdata.global_timeout
        {
            Some(to) => to,
            None => 5, // default is 5 sec
        };
        //println!("global timeout: {:?}", global_timeout);
        //get output
        let timeout = match self.meta.timeout {
            Some(to) => to,
            None => global_timeout,
        
        };

        let mut given_output = command_timeout(run_cmd, timeout, self.meta.number);
        println!(
            "testcase gave output {} {}",
            self.meta.name, self.meta.number
        );

        let mut timeout = true;
        if given_output.1.is_some()
        {
            timeout = false;
        }
        else
        {
            if given_output.0.len() > stdoutstring.len() * 4
            {
                let output_length = std::cmp::min( stdoutstring.len()  * 4 ,  given_output.0.len() );
                given_output.0 = given_output.0.chars().take(output_length).collect();
                println!("reducing output length because of endless loop!");
            }
            
        }
        // TODO options string/array in testcase data
        // TODO ignore if no_trim?
        //let given_output_t = given_output.lines().map(str::trim).collect();
        //let exp_output_t = stdoutstring.lines().map(str::trim).collect();

        // make changeset

        //let now = Instant::now();
        let compare_mode = unsafe { COMPARE_MODE[0] };


        let changeset = Changeset::new(&stdoutstring, &given_output.0, compare_mode );

        let distance = changeset.distance;//get_distance(&stdoutstring, &given_output.0);
        let status = given_output.1; // TODO refactor
        let mut passed: bool = false; //TODO check if there are not diffs

        if self.exp_retvar.is_some() && status.is_some() && status.unwrap() == self.exp_retvar.unwrap() && distance == 0 && !timeout 
        {
            passed = true;            
        }

        // get vg errors and warnings
        // make path to valgrind file
        //let mut exe_path  = String::from("");



        let verbose = unsafe { VERBOSE };

        if verbose && distance != 0
        {   
            println!("--------------------------------");
            println!("Distance: {:?}", distance);
            println!("Wanted Output:\n{:?}", stdoutstring);
            println!("--------------------------------");
            println!("Your Output:\n{:?}", given_output.0);
        }
        
        // prints diff with colors to terminal
        // green = ok
        // blue = reference (our solution)
        // red = wrong (students solution) / too much
        
        if changeset.distance > 0 &&  verbose
        {
            let mut colored_stdout = StandardStream::stdout(ColorChoice::Always);

            for c in &changeset.diffs
            {
                match *c 
                {
                    Difference::Same(ref z)=> 
                    {
                        colored_stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green))).unwrap();
                        writeln!(&mut colored_stdout, "{}", String::from(z) ).unwrap();
                    }
                    Difference::Rem(ref z) =>
                    {
                        colored_stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue))).unwrap();
                        writeln!(&mut colored_stdout, "{}", String::from(z)  ).unwrap();
                    }

                    Difference::Add(ref z) =>
                    {
                        colored_stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red))).unwrap();
                        writeln!(&mut colored_stdout, "{}", String::from(z)  ).unwrap();
                    }

                }
            }
            colored_stdout.reset().unwrap();
        }

            
        let valgrind = parse_vg_log(&_vg_filepath).unwrap_or((-1, -1));
        println!("{:?}", valgrind);

        let new_now = Instant::now();
        println!("testcase took {:?}", new_now.duration_since(starttime));
        println!("done with {}", self.meta.number);

        Ok(TestResult {
            diff : Some(changeset),
            //diff: Some(diff),
            compile_warnings: None,
            implemented: None,
            passed,
            result: given_output.0.clone(),
            ret: status,
            exp_ret: self.exp_retvar,
            vg_warnings: valgrind.0,
            vg_errors: valgrind.1,
            vg_logfile: vg_filepath2,
            command_used: String::from(format!("./{} {}", &self.meta.projdata.project_name, &self.argv)),
            used_input: stdinstring,
            timeout: timeout,
            name: self.meta.name.clone(),
            description: self.meta.desc.clone().unwrap_or(String::from("")),
            number: self.meta.number,
            kind: self.meta.kind,
            distance_percentage: Some(percentage_from_levenstein(
                distance,
                &stdoutstring,
                &given_output.0,
            )),
            protected: self.meta.protected,
        })
    }

    #[allow(unused_variables)]
    fn from_saved_tc(
        number: i32,
        testcase: &SavedTestcase,
        projdata: &ProjectData,
        binary: Option<&Binary>,
    ) -> Result<Self, GenerationError> {
        match binary {
            Some(binary) => {}
            None => {
                return Err(GenerationError::BinaryRequired);
            }
        };
        let meta = TestMeta {
            kind: TestCaseKind::IOTest,
            number,
            name: testcase.name.clone(),
            desc: testcase.description.clone(),
            projdata: projdata.clone(),
            timeout: testcase.timeout,
            protected: testcase.protected.unwrap_or(false),
        };

        let retvar = IoTest {
            meta,
            binary: binary.unwrap().clone(),
            exp_retvar: testcase.exp_retvar,
            argv: testcase.args.as_ref().unwrap_or(&String::new()).clone(),
            in_file: testcase.in_file.as_ref().unwrap_or(&String::new()).clone(),
            exp_file: testcase.exp_file.as_ref().unwrap_or(&String::new()).clone(),
            in_string: testcase
                .in_string
                .as_ref()
                .unwrap_or(&String::new())
                .clone(),
            exp_string: testcase
                .exp_string
                .as_ref()
                .unwrap_or(&String::new())
                .clone(),
            env_vars: testcase.env_vars.clone(),
        };

        Ok(retvar)
    }
}

#[derive(Debug)]
pub enum CompileError {
    None,
    NoMakefile,
    MakeFailed,
    NoIssuesReported,
}

#[derive(Clone, Debug)]
pub struct CompileInfo {
    warnings: i32,
    errors: i32,
    compiled: bool,
}

#[derive(Debug)]
pub enum GenerationError {
    None,
    MakeFileRequired,
    ConfigErrorIO,
    BinaryRequired,
    VgLogNotFound,
    VgLogParseError,
    CouldNotMakeBinary,
    ConfigErrorUnit,
}

impl std::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GenerationError: {}",
            match self {
                GenerationError::None => "None".to_string(),
                GenerationError::MakeFileRequired => "MakeFileRequired".to_string(),
                GenerationError::ConfigErrorIO => "ConfigErrorIO".to_string(),
                GenerationError::BinaryRequired => "BinaryRequired".to_string(),
                GenerationError::VgLogNotFound => "VgLogNotFound".to_string(),
                GenerationError::VgLogParseError => "VgLogParseError".to_string(),
                GenerationError::CouldNotMakeBinary => "CouldNotMakeBinary".to_string(),
                GenerationError::ConfigErrorUnit => "ConfigErrorUnit".to_string()
            }
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct TestDefinition {
    project_data: ProjectData,
    testcases: Vec<SavedTestcase>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProjectData {
    project_name: String,
    makefile_path: Option<String>,
    maketarget: Option<String>,
    lib_path: Option<String>,
    global_timeout : Option<i32>,
    valgrind_flags : Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct Binary {
    project_data: ProjectData,
    info: CompileInfo,
}

impl Binary {
    pub fn from_project_data(projdata: &ProjectData) -> Result<Self, CompileError> {
        let retvar = Binary {
            project_data: projdata.clone(),
            info: CompileInfo {
                errors: 0,
                warnings: 0,
                compiled: false,
            },
        };
        Ok(retvar)
    }

    pub fn compile(&mut self) -> Result<(), CompileError> {
        let makefile_path = match &self.project_data.makefile_path {
            Some(expr) => expr.clone(),
            None => {
                return Err(CompileError::NoMakefile);
            }
        };

        let mut make_cmd = Command::new("make");
        make_cmd.current_dir(makefile_path);
        make_cmd.stderr(Stdio::piped());
        make_cmd.stdout(Stdio::piped());
        if self.project_data.maketarget.is_some() {
            make_cmd.arg(
                self.project_data
                    .maketarget
                    .clone()
                    .unwrap_or(String::new()),
            );
        }
        match make_cmd.output() {
            Ok(res) => {
                let errorstring = String::from_utf8(res.stderr).unwrap_or_default();
                let re = Regex::new(
                    r"(?P<warn>[0-1]*) warnings? and (?P<err>[0-1]*) errors? generated.",
                )
                .unwrap();
                let re2 = Regex::new(r"(?P<warn>[0-1]*) warnings? generated.").unwrap();
                if res.status.code().unwrap_or(-1) != 0 {
                    self.info.compiled = false;
                    let issues = re.captures_iter(&errorstring).last(); // last match is the best
                    match issues {
                        Some(found_issues) => {
                            self.info.warnings = found_issues["warn"].parse().unwrap_or(-1);
                            self.info.errors = found_issues["err"].parse().unwrap_or(-1);
                        }
                        None => {
                            return Err(CompileError::NoIssuesReported);
                        }
                    }
                } else {
                    self.info.compiled = true;
                    println!("looks good");
                    //checking for warnings...
                    let issues = re2.captures_iter(&errorstring).last();
                    match issues {
                        Some(found_warnings) => {
                            //warnings found => parse them
                            println!("{:?}", &found_warnings["warn"]);
                            self.info.warnings = found_warnings["warn"].parse().unwrap_or(-1);
                        }
                        None => {
                            self.info.errors = 0;
                            self.info.warnings = 0;
                        }
                    }
                }
            }
            Err(err) => {
                println!("noo {:?}", err);
                return Err(CompileError::MakeFailed);
            }
        }
        Ok(())
    }
}


#[derive(Debug, Deserialize)]
struct SavedTestcase {
    name: String,
    subname: Option<String>,
    testcase_type: String,
    description: Option<String>,
    args: Option<String>,
    cflags: Option<String>,
    fname: Option<String>,
    // note: if type is mandatory for unit test
    in_file: Option<String>,
    exp_file: Option<String>,
    in_string: Option<String>,
    exp_string: Option<String>,
    exp_retvar: Option<i32>,
    timeout: Option<i32>,
    env_vars: Option<String>,
    protected: Option<bool>,
}

fn main() {
    let cli_args = App::new("testrunner")
        .version("0.2")
        .author("Thomas Brunner t.brunner@student.tugraz.at")
        .about("The new rust based testsystem for esp/oop1")
        .arg(
            Arg::with_name("TESTINPUT")
                .short("t")
                .long("testinput")
                .help("uses the built in test config file. For test purposes only"),
        )
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("CONFIG_FILE")
                .required_unless("TESTINPUT")
                .takes_value(true)
                .help("Toml formated test specification file"),
        )
        .arg(
            Arg::with_name("json")
                .short("j")
                .long("json-output")
                .takes_value(true)
                .value_name("JSON_OUT")
                .default_value("result.json")
                .help("writes testresult in json format to specific file"),
        )
        .arg(
            Arg::with_name("html")
                .short("o")
                .long("html-output")
                .takes_value(true)
                .value_name("HTML_OUTPUT")
                .default_value("result.html")
                .help("writes testresult in pretty html format"),
        )
        .arg(
            Arg::with_name("prot-html")
                .short("p")
                .long("prot-html")
                .takes_value(true)
                .value_name("PROT_HTML_OUTPUT")
                .default_value("prot-result.html")
                .help("writes testresult in pretty html format, with details of protected testcases redacted")
            )
        .arg(
            Arg::with_name("browser")
                .short("b")
                .requires("html")
                .help("opens the html file with xdg-open"),
        )
        .arg(
            Arg::with_name("compare_mode")
                .short("m")
                .long("mode")
                .help("L, l : Compare outputs line by line\nW, w : compare outputs word by word\n C,c : compare outputs char by char")
                .takes_value(true)
                //.default_value("L")
        )
        .arg(
            Arg::with_name("verbosity_level")
                .short("v")
                .long("verbose")
                .takes_value(false)
                //.default_value("false")
                .help("print diff to terminal 0 = off, 1 = on")
        )
        .get_matches();

        match cli_args.is_present("compare_mode")
        {  true =>
            {
                let compare_mode = cli_args.value_of("compare_mode").unwrap().to_uppercase();
                unsafe 
                {
                    if compare_mode.contains("W")
                    {
                        COMPARE_MODE = [ " "   ];
                    }
                    else if compare_mode.contains("C")
                    {
                        COMPARE_MODE = [ "" ];
                    }
                    else
                    {
                        COMPARE_MODE = [ "\n"   ];
                    }
                }
            }
            false =>
            {
                unsafe { COMPARE_MODE = [ "\n" ]; }
            }
        }
        //let mut compare_mode = "";
        let compare_mode = unsafe{ COMPARE_MODE[0] };

        //let verbose = false;
        match cli_args.is_present("verbosity_level")
        {
            true =>
            {
                unsafe{ VERBOSE = true};
            }
            false =>
            {
                unsafe {VERBOSE = false};
            }
        }

        

    let config: String = match cli_args.is_present("TESTINPUT") {
        true => {
            println!("{}", "using testconfig...".blue());
            String::from(
                r#"
            [project_data]
            project_name = "ass"
            makefile_path = "resources/dummy_assignment/ass_test"
        
            [[testcases]]
            name = "tombers"
            testcase_type = "UnitTest"
            description = "hello my lady"
              [testcases.tags]
              fname = "lol"
              result = "yay"
        
            [[testcases]]
            name = "bommers"
            testcase_type = "IO"
            description = "wubwub"
            exp_string = "oi\nhelloyolo"
            in_string = "tom\n1\n"
            protected = true

            [[testcases]]
            name = "timeout"
            testcase_type = "IO"
            description = "wubwub"
            exp_string = "oi\nhelloyolo"
            env_vars = "foo,fo=bar"
          "#,
            )
        }
        false => read_to_string(cli_args.value_of("config").unwrap())
            .expect("cannot open or read config file"),
    };

    let mut generator =
        TestCaseGenerator::form_string(&config).expect("could not parse config file");
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
            .make_html_report(compare_mode, false)
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
            .make_html_report(compare_mode, true)
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
