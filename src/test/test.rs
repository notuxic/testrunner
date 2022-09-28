use std::fs::File;
use std::sync::Weak;
use std::{fmt, io::Read};

use difference::Changeset;
use serde_derive::{Deserialize, Serialize};
use thiserror::Error;

use crate::project::binary::Binary;
use crate::project::definition::ProjectDefinition;
use crate::testresult::testresult::Testresult;
use crate::testrunner::{TestrunnerError, TestrunnerOptions};
use super::diff::{changeset_to_html, diff_binary_to_html};
use super::io_test::percentage_from_levenstein;


#[derive(Debug, Error)]
pub enum TestingError {
    #[error("valgrind log not found: {0}")]
    VgLogNotFound(String),
    #[error("failed parsing valgrind log: {0}")]
    VgLogParseError(String),
    #[error("required binary not found: {0}")]
    MissingBinDependency(String),
    #[error("i/o config not found: {0}")]
    IoConfigNotFound(String),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("internal i/o error: i/o mismatch")]
    IOMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum TestcaseType {
    #[serde(alias = "IO")]
    IOTest,
    #[serde(alias = "OrdIO")]
    OrdIOTest,
}

impl fmt::Display for TestcaseType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum DiffKind {
    PlainText,
    Binary,
}

impl Default for DiffKind {
    fn default() -> DiffKind {
        DiffKind::PlainText
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TestMeta {
    #[serde(skip)]
    pub number: i32,
    pub name: String,
    pub description: Option<String>,
    pub timeout: Option<u64>,
    #[serde(default)]
    pub add_diff_kind: DiffKind,
    pub add_out_file: Option<String>,
    pub add_exp_file: Option<String>,
    #[serde(default)]
    pub protected: bool,
}

pub trait Test : erased_serde::Serialize {
    fn init(&mut self, number: i32, project_definition: Weak<ProjectDefinition>, options: Weak<TestrunnerOptions>, binary: Weak<Binary>) -> Result<(), TestrunnerError>;

    fn run(&self) -> Result<Box<dyn Testresult + Send + Sync>, TestingError>;

    fn get_test_meta(&self) -> &TestMeta;

    // needed for deserializing with `serde_tagged`
    fn type_id(&self) -> &'static str;

    fn deserialize_trait<'de, D: ?Sized>(deserializer: &mut dyn erased_serde::Deserializer<'de>) -> Result<Box<dyn Test + Send + Sync>, erased_serde::Error>
        where Self: Sized;

    fn get_add_diff(&self, options: &TestrunnerOptions) -> Option<(String, i32, f32)> {
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

                    let changeset = Changeset::new(&orig, &edit, &options.diff_delim);
                    return match changeset_to_html(&changeset, &options.diff_delim, options.ws_hints, "File") {
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

