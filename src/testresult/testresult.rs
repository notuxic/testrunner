use crate::test::test::TestcaseType;
use crate::testrunner::TestrunnerError;


pub trait Testresult {
    fn kind(&self) -> TestcaseType;

    fn number(&self) -> i32;

    fn name(&self) -> String;

    fn passed(&self) -> bool;

    fn protected(&self) -> bool;

    fn timeout(&self) -> bool;

    fn mem_leaks(&self) -> Option<i32>;

    fn mem_errors(&self) -> Option<i32>;

    fn mem_logfile(&self) -> String;

    fn exit_code(&self) -> Option<i32>;

    fn expected_exit_code(&self) -> Option<i32>;

    fn diff_distance(&self) -> f32;

    fn add_diff_distance(&self) -> Option<f32>;

    fn get_json_entry(&self) -> Result<serde_json::Value, TestrunnerError>;

    fn get_html_entry_detailed(&self) -> Result<String, TestrunnerError>;
}

