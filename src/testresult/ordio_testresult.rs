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
    pub io_diff: Vec<IODiff>,
    pub diff_distance: f32,
    pub truncated_output: bool,
    pub mem_leaks: Option<i32>,
    pub mem_errors: Option<i32>,
    pub mem_logfile: String,
    pub command_used: String,
    pub timeout: bool,
    pub exit_code: Option<i32>,
    pub expected_exit_code: Option<i32>,
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
        self.exit_code
    }

    fn expected_exit_code(&self) -> Option<i32> {
        self.expected_exit_code
    }

    fn diff_distance(&self) -> f32 {
        self.diff_distance
    }

    fn add_diff_distance(&self) -> Option<f32> {
        self.add_distance
    }

    fn get_json_entry(&self) -> Result<serde_json::Value, TestrunnerError> {
        Ok(json!({
            "kind": format!("{}",self.kind),
            "name": self.name,
            "description": self.description,
            "passed": self.passed,
            "diff": self.io_diff,
            "diff_distance": self.diff_distance,
            "add_diff": self.add_diff,
            "add_diff_distance": self.add_distance.unwrap_or(-1.0),
            "add_file_missing": self.add_file_missing,
            "truncated_output": self.truncated_output,
            "command_used": self.command_used,
            "exit_code": self.exit_code.unwrap_or(0),
            "mem_leaks": self.mem_leaks.unwrap_or(-1),
            "mem_errors": self.mem_errors.unwrap_or(-1),
            "timeout": self.timeout,
            "input": self.input,
            "protected" : self.protected,
        }))
    }

    fn get_html_entry_detailed(&self) -> Result<String, TestrunnerError> {
        Ok(self.clone().render_once()?)
    }
}

