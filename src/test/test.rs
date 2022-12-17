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

#[derive(Clone, Debug, Serialize)]
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

    fn did_pass(&self, exp_exit_code: Option<i32>, exit_code: Option<i32>, distance: f32, add_distance: f32, had_timeout: bool) -> bool {
        exit_code.is_some() && exit_code.unwrap() == exp_exit_code.unwrap_or(0)
            && distance == 1.0 && add_distance == 1.0 && !had_timeout
    }

    fn get_valgrind_result(&self, project_definition: &ProjectDefinition, options: &TestrunnerOptions, basedir: &str, vg_log_folder: &str, vg_filepath: &str) -> Result<(Option<i32>, Option<i32>), TestingError> {
        let meta = self.get_test_meta();
        let mem_leaks;
        let mem_errors;
        if project_definition.use_valgrind.unwrap_or(true) {
            #[allow(unused_must_use)]
            if cfg!(unix) && options.sudo.is_some() {
                match copy(&vg_filepath, format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &meta.number)) {
                    Ok(_) => remove_file(&vg_filepath),
                    Err(_) => Ok(()),
                };
            }
            (mem_leaks, mem_errors) = match parse_vg_log(&format!("{}/{}/{}/vg_log.txt", &basedir, &vg_log_folder, &meta.number)) {
                Ok(valgrind) => (Some(valgrind.0), Some(valgrind.1)),
                Err(TestingError::VgLogParseError(path)) => {
                    eprintln!("Warning: failed parsing valgrind log: {}", path);
                    (None, None)
                },
                Err(err) => {
                    return Err(err);
                },
            };

            if cfg!(unix) && options.sudo.is_some() && meta.protected {
                remove_dir_all(&format!("{}/{}/{}", &basedir, &vg_log_folder, &meta.number)).unwrap_or(());
            }
        }
        else {
            mem_leaks = None;
            mem_errors = None;
        }
        Ok((mem_leaks, mem_errors))
    }

    fn get_add_diff(&self) -> Result<(Option<Diff>, f32, bool), TestingError> {
        let add_file_missing;
        let add_diff = match self.calc_add_diff() {
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
        else if add_file_missing {
            add_distance = 0.0;
        }
        else {
            add_distance = 1.0;
        }
        Ok((add_diff, add_distance, add_file_missing))
    }

    fn calc_add_diff(&self) -> Result<Option<Diff>, TestingError> {
        let test_meta = self.get_test_meta();

        if test_meta.add_out_file.is_some() && test_meta.add_exp_file.is_some() {
            match test_meta.add_diff_mode {
                DiffKind::PlainText => {
                    let ref_file = read_to_string(test_meta.add_exp_file.as_ref().unwrap())
                        .map_err(|_| TestingError::RefFileNotFound(test_meta.add_exp_file.as_ref().unwrap().clone()))?;
                    let out_file = read_to_string(test_meta.add_out_file.as_ref().unwrap())
                        .map_err(|_| TestingError::OutFileNotFound(test_meta.add_out_file.as_ref().unwrap().clone()))?;

                    let (diff, distance) = diff_plaintext(&ref_file, &out_file, Duration::from_secs(20));
                    Ok(Some(Diff::PlainText(diff, distance)))
                },
                DiffKind::Binary => {
                    let mut ref_fd = File::open(test_meta.add_exp_file.as_ref().unwrap())
                        .map_err(|_| TestingError::RefFileNotFound(test_meta.add_exp_file.as_ref().unwrap().clone()))?;
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

