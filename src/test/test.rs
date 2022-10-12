use std::fs::{File, read_to_string};
use std::sync::Weak;
use std::time::Duration;
use std::{fmt, io::Read};

use serde_derive::{Deserialize, Serialize};
use thiserror::Error;

use crate::project::binary::Binary;
use crate::project::definition::ProjectDefinition;
use crate::testresult::testresult::Testresult;
use crate::testrunner::{TestrunnerError, TestrunnerOptions};
use super::diff::{diff_plaintext, ChangesetInline, ChangesetFlat, diff_binary};


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
    #[error("reference-file not found: {0}")]
    RefFileNotFound(String),
    #[error("input-file not found: {0}")]
    InFileNotFound(String),
    #[error("output-file not found: {0}")]
    OutFileNotFound(String),
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
    #[serde(alias = "plaintext", alias = "text")]
    PlainText,
    #[serde(alias = "binary")]
    Binary,
}

impl Default for DiffKind {
    fn default() -> DiffKind {
        DiffKind::PlainText
    }
}

#[derive(Debug, Serialize)]
pub enum Diff {
    PlainText(Vec<ChangesetInline<String>>, f32),
    Binary(Vec<ChangesetFlat<Vec<u8>>>, f32),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TestMeta {
    #[serde(skip)]
    pub number: i32,
    pub name: String,
    pub description: Option<String>,
    pub timeout: Option<u64>,
    #[serde(default)]
    pub add_diff_mode: DiffKind,
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

    fn get_add_diff(&self) -> Result<Option<Diff>, TestingError> {
        let test_meta = self.get_test_meta();

        if test_meta.add_out_file.is_some() && test_meta.add_exp_file.is_some() {
            match test_meta.add_diff_mode {
                DiffKind::PlainText => {
                    let ref_file = read_to_string(test_meta.add_exp_file.as_ref().unwrap())
                        .unwrap();
                        // .map_err(|_| TestingError::RefFileNotFound(test_meta.add_exp_file.as_ref().unwrap().clone()))?;
                    let out_file = read_to_string(test_meta.add_out_file.as_ref().unwrap())
                        .map_err(|_| TestingError::OutFileNotFound(test_meta.add_out_file.as_ref().unwrap().clone()))?;

                    let (diff, distance) = diff_plaintext(&ref_file, &out_file, Duration::from_secs(20));
                    Ok(Some(Diff::PlainText(diff, distance)))
                },
                DiffKind::Binary => {
                    let mut ref_fd = File::open(test_meta.add_exp_file.as_ref().unwrap())
                        .unwrap();
                        // .map_err(|_| TestingError::RefFileNotFound(test_meta.add_exp_file.as_ref().unwrap().clone()))?;
                    let mut out_fd = File::open(test_meta.add_out_file.as_ref().unwrap())
                        .map_err(|_| TestingError::OutFileNotFound(test_meta.add_out_file.as_ref().unwrap().clone()))?;

                    let mut ref_buf = Vec::<u8>::new();
                    let mut out_buf = Vec::<u8>::new();
                    #[allow(unused_must_use)]
                    {
                        ref_fd.read_to_end(&mut ref_buf);
                        out_fd.read_to_end(&mut out_buf);
                    }

                    let (diff, distance) = diff_binary(&ref_buf, &out_buf, Duration::from_secs(20));
                    Ok(Some(Diff::Binary(diff, distance)))
                }
            }
        }
        else {
            Ok(None)
        }
    }
}

