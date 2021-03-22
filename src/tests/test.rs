use std::{fmt, io::Read};
use std::fs::File;
use difference::Changeset;
use serde_derive::Serialize;
use super::diff::{changeset_to_html, diff_binary_to_html};
use super::testcase::Testcase;
use super::testresult::TestResult;
use crate::project::definition::ProjectDefinition;
use crate::project::binary::{Binary, GenerationError};


#[derive(Debug, Clone, Copy, Serialize)]
pub enum TestCaseKind {
    UnitTest,
    IOTest,
}

pub enum DiffKind {
    PlainText,
    Binary,
}

#[allow(dead_code)]
pub struct TestMeta {
    pub number: i32,
    pub name: String,
    pub desc: Option<String>,
    pub timeout: Option<i32>,
    pub projdef: ProjectDefinition, // use lifetime ref?
    pub kind: TestCaseKind,
    pub add_diff_kind: Option<DiffKind>,
    pub add_in_file: Option<String>,
    pub add_exp_file: Option<String>,
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
    fn get_test_meta(&self) -> &TestMeta;
    fn get_add_diff(&self) -> Option<String> {
        let test_meta = self.get_test_meta();
        if test_meta.add_diff_kind.is_some() {
            let mut fd_user = File::open(test_meta.add_in_file.as_ref().unwrap()).expect(&format!("Cannot open file `{}`", test_meta.add_in_file.as_ref().unwrap()));
            let mut fd_ref = File::open(test_meta.add_exp_file.as_ref().unwrap()).expect(&format!("Cannot open file `{}`", test_meta.add_exp_file.as_ref().unwrap()));

            match test_meta.add_diff_kind.as_ref().unwrap() {
                DiffKind::PlainText => {
                    let mut buf_user = String::new();
                    let mut buf_ref = String::new();
                    match fd_user.read_to_string(&mut buf_user) {
                        Ok(_) => (),
                        Err(e) => panic!(e),
                    }
                    match fd_ref.read_to_string(&mut buf_ref) {
                        Ok(_) => (),
                        Err(e) => panic!(e),
                    }

                    let changeset = Changeset::new(&buf_ref, &buf_user, &test_meta.projdef.diff_mode);
                    return match changeset_to_html(&changeset, &test_meta.projdef.diff_mode) {
                        Ok(text) => Some(text),
                        Err(_) => None,
                    }
                },
                DiffKind::Binary => {
                    let mut buf_user = Vec::<u8>::new();
                    let mut buf_ref = Vec::<u8>::new();
                    match fd_user.read_to_end(&mut buf_user) {
                        Ok(_) => (),
                        Err(e) => panic!(e),
                    }
                    match fd_ref.read_to_end(&mut buf_ref) {
                        Ok(_) => (),
                        Err(e) => panic!(e),
                    }

                    return match diff_binary_to_html(&buf_ref, &buf_user) {
                        Ok(text) => Some(text),
                        Err(_) => None,
                    }
                },
            }
        };
        None
    }
}
