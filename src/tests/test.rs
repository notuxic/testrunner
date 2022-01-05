use std::{fmt, io::Read};
use std::fs::File;
use difference::Changeset;
use serde_derive::Serialize;
use super::diff::{changeset_to_html, diff_binary_to_html};
use super::io_test::percentage_from_levenstein;
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
    pub timeout: Option<u64>,
    pub projdef: ProjectDefinition, // use lifetime ref?
    pub kind: TestCaseKind,
    pub add_diff_kind: DiffKind,
    pub add_out_file: Option<String>,
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

    fn get_add_diff(&self) -> Option<(String, i32, f32)> {
        let test_meta = self.get_test_meta();

        if test_meta.add_out_file.is_some() && test_meta.add_exp_file.is_some() {

            let fd_user = File::open(test_meta.add_out_file.as_ref().unwrap());//.expect(&format!("Cannot open file `{}`", test_meta.add_out_file.as_ref().unwrap()));
            let mut fd_ref = File::open(test_meta.add_exp_file.as_ref().unwrap()).expect(&format!("Cannot open file `{}`", test_meta.add_exp_file.as_ref().unwrap()));

            let mut buf_user = Vec::<u8>::new();
            let mut buf_ref = Vec::<u8>::new();

            match fd_user {
                Ok(_) => {
                    match fd_user.unwrap().read_to_end(&mut buf_user) {
                        Ok(_) => (),
                        Err(e) => {
                            println!("Could not read file {}.\n{}",test_meta.add_out_file.as_ref().unwrap(), e);
                        }
                    }
                }
                Err(e) => {
                    println!("Could not open File {}\n{}", test_meta.add_out_file.as_ref().unwrap(), e);
                }
            }

            match fd_ref.read_to_end(&mut buf_ref) {
                Ok(_) => (),
                Err(e) => panic!("{}", e),
            }


            match test_meta.add_diff_kind {
                DiffKind::PlainText => {
                    let orig = format!("{}", String::from_utf8_lossy(&buf_ref));
                    let edit = format!("{}", String::from_utf8_lossy(&buf_user));

                    let changeset = Changeset::new(&orig, &edit, &test_meta.projdef.diff_delim);
                    return match changeset_to_html(&changeset, &test_meta.projdef.diff_delim, test_meta.projdef.ws_hints, "File") {
                        Ok(text) => Some((text, changeset.distance, percentage_from_levenstein(changeset.distance, buf_ref.len(), buf_user.len()))),
                        Err(_) => None,
                    }
                }
                DiffKind::Binary => {
                    return match diff_binary_to_html(&buf_ref, &buf_user) {
                        Ok(text) => Some((text.0, text.1, percentage_from_levenstein(text.1, buf_ref.len(), buf_user.len()))),
                        Err(_) => None,
                    }
                }

            }
        };
        None
    }
}
