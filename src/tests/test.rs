use std::fmt;
use serde_derive::Serialize;
use super::testcase::Testcase;
use super::testresult::TestResult;
use crate::project::definition::ProjectDefinition;
use crate::project::binary::{Binary, GenerationError};


#[derive(Debug, Clone, Copy, Serialize)]
pub enum TestCaseKind {
    UnitTest,
    IOTest,
}

#[allow(dead_code)]
pub struct TestMeta {
    pub number: i32,
    pub name: String,
    pub desc: Option<String>,
    pub timeout: Option<i32>,
    pub projdef: ProjectDefinition, // use lifetime ref?
    pub kind: TestCaseKind,
    pub protected: bool,
}

impl fmt::Display for TestCaseKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
            // or, alternatively:
            // fmt::Debug::fmt(self, f)
    }
}

pub trait Test {
    fn run(&self) -> Result<TestResult, GenerationError>;
    fn from_saved_tc(
        number: i32,
        testcase: &Testcase,
        projdef: &ProjectDefinition,
        binary: Option<&Binary>,
    ) -> Result<Self, GenerationError>
    where
        Self: Sized;
    //fn report(&self) -> Result<String,GenerationError>;
}

