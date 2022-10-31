use std::fs::{File, read_to_string, copy, remove_file, remove_dir_all};
use std::sync::Weak;
use std::time::Duration;
use std::{fmt, io::Read};

use serde_derive::{Deserialize, Serialize};
use thiserror::Error;

use crate::project::binary::Binary;
use crate::project::definition::ProjectDefinition;
use crate::test::io_test::parse_vg_log;
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
    
    fn print_finish_message(&self, protected: bool, test_number: i32, test_name: String) {
        if protected {
            println!("Finished testcase {}: ********", test_number);
        }
        else {
            println!("Finished testcase {}: {}", test_number, test_name);
        }
    }

    fn get_valgrind_result(&self, is_sudo: bool, vg_filepath: String, basedir: String, vg_log_folder: String, test_number: i32, protected: bool) -> ((i32, i32), String) {
        if cfg!(unix) && is_sudo {
            match copy(&vg_filepath, format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &test_number)) {
                Ok(_) => remove_file(&vg_filepath).unwrap_or(()),
                Err(_) => (),
            }
        }
        let valgrind = parse_vg_log(&format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &test_number)).unwrap_or((-1, -1));
        println!("Memory usage errors: {:?}\nMemory leaks: {:?}", valgrind.1, valgrind.0);

        if cfg!(unix) && is_sudo && protected {
            remove_dir_all(&format!("{}/{}/{}", &basedir, &vg_log_folder, &test_number)).unwrap_or(());
        }
        let vg_filepath = format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &test_number);
        (valgrind, vg_filepath)
    }

    fn did_pass(&self, exp_retvar: Option<i32>, retvar: Option<i32>, distance: f32, add_distance: f32, had_timeout: bool) -> bool {
        exp_retvar.is_some() && retvar.is_some() && retvar.unwrap() == exp_retvar.unwrap()
            && distance == 1.0 && add_distance == 1.0 && !had_timeout
    }

    fn prepare_add_diff(&self) -> Result<(bool, Option<Diff>, f32), TestingError> {
        let add_file_missing;
        let add_diff = match self.get_add_diff() {
            Ok(ok) => {
                add_file_missing = false;
                ok
            },
            Err(TestingError::OutFileNotFound(_)) => {
                add_file_missing = true;
                None
            },
            Err(e) => return Err(e),
        };

        let add_distance: f32;
        if let Some(ref diff) = add_diff {
            match diff {
                Diff::PlainText(_, d) => add_distance = *d,
                Diff::Binary(_, d) => add_distance = *d,
            }
        }
        else {
            add_distance = 1.0;
        }
        Ok((add_file_missing, add_diff, add_distance))
    }

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

