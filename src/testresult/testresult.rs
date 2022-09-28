use crate::test::test::TestcaseType;
use crate::testrunner::TestrunnerError;


pub trait Testresult {
    fn get_testcase_type(&self) -> TestcaseType;

    fn passed(&self) -> bool;

    fn protected(&self) -> bool;

    fn get_json_entry(&self) -> Result<serde_json::Value, TestrunnerError>;

    fn get_html_entry_summary(&self, protected_mode: bool) -> Result<String, TestrunnerError>;

    fn get_html_entry_detailed(&self) -> Result<String, TestrunnerError>;
}

