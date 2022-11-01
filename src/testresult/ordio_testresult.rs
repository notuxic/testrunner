use std::sync::Weak;

use sailfish::TemplateOnce;
use serde_derive::Serialize;
use serde_json::json;

use crate::project::definition::ProjectDefinition;
use crate::test::diff::{textdiff_to_html, binarydiff_to_html, iodiff_to_html};
use crate::test::ordio_test::IODiff;
use crate::test::test::{TestcaseType, Diff};
use crate::testrunner::{TestrunnerError, TestrunnerOptions};
use super::testresult::Testresult;


#[derive(Clone, Serialize, TemplateOnce)]
#[template(path = "testreport_testcase_ordio.stpl")]
pub struct OrdIoTestresult {
    pub kind: TestcaseType,
    pub number: i32,
    pub name: String,
    pub description: String,
    pub protected: bool,
    pub add_diff: Option<Diff>,
    pub add_distance: Option<f32>,
    pub add_file_missing: bool,
    #[serde(skip)]
    pub io_diff: Vec<IODiff>,
    pub diff_distance: f32,
    pub truncated_output: bool,
    pub mem_leaks: Option<i32>,
    pub mem_errors: Option<i32>,
    pub mem_logfile: String,
    pub command_used: String,
    pub timeout: bool,
    pub ret: Option<i32>,
    pub exp_ret: Option<i32>,
    pub passed: bool,
    pub input: String,
    #[serde(skip)]
    pub project_definition: Weak<ProjectDefinition>,
    #[serde(skip)]
    pub options: Weak<TestrunnerOptions>,
}

impl Testresult for OrdIoTestresult {
    fn kind(&self) -> TestcaseType {
        TestcaseType::OrdIOTest
    }

    fn number(&self) -> i32 {
        self.number
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn passed(&self) -> bool {
        self.passed
    }

    fn protected(&self) -> bool {
        self.protected
    }

    fn timeout(&self) -> bool {
        self.timeout
    }

    fn truncated_output(&self) -> bool {
        self.truncated_output
    }

    fn mem_leaks(&self) -> Option<i32> {
        self.mem_leaks
    }

    fn mem_errors(&self) -> Option<i32> {
        self.mem_errors
    }

    fn mem_logfile(&self) -> String {
        self.mem_logfile.clone()
    }

    fn exit_code(&self) -> Option<i32> {
        self.ret
    }

    fn expected_exit_code(&self) -> Option<i32> {
        self.exp_ret
    }

    fn diff_distance(&self) -> f32 {
        self.diff_distance
    }

    fn add_diff_distance(&self) -> Option<f32> {
        self.add_distance
    }

    fn get_json_entry(&self) -> Result<serde_json::Value, TestrunnerError> {
        Ok(json!({
            "name": self.name,
            "kind": format!("{}",self.kind),
            "passed": self.passed,
            "distance": self.diff_distance,
            "add_distance": self.add_distance.unwrap_or(-1.0),
            "statuscode": self.ret.unwrap_or(0),
            "mem_leaks": self.mem_leaks,
            "mem_errors": self.mem_errors,
            "timeout": self.timeout,
            "protected" : self.protected,
        }))
    }

    fn get_html_entry_detailed(&self) -> Result<String, TestrunnerError> {
        Ok(self.clone().render_once()?)
    }
}

