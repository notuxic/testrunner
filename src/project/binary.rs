use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

use regex::Regex;
use serde_derive::Serialize;
use thiserror::Error;

use super::definition::ProjectDefinition;


#[derive(Debug, Error)]
pub enum CompileError {
    #[error("binary not found: {0}")]
    BinaryNotFound(String),
    #[error("no Makefile found at: {0}")]
    MakefileNotFound(String),
    #[error("calling `make` failed: {}", .0.to_string())]
    MakeFailed(std::io::Error),
}

#[derive(Debug, Default, Serialize)]
pub struct CompilationInfo {
    pub warnings: Option<HashMap<String, i32>>,
    pub errors: Option<String>,
    pub compiled: bool,
}

#[derive(Debug, Default)]
pub struct Binary {
    pub info: CompilationInfo,
}

impl Binary {

    pub fn from_project_definition(project_definition: &ProjectDefinition) -> Result<Self, CompileError> {
        // use pre-compiled binary
        if project_definition.makefile_path.is_none() {
            if Self::exists(project_definition) {
                Ok(Binary { info: CompilationInfo { warnings: None, errors: None, compiled: true } })
            }
            else {
                return Err(CompileError::BinaryNotFound(project_definition.binary_path.clone()));
            }
        }
        // use `make`
        else if project_definition.makefile_path.is_some() {
            Ok(Binary { info: Self::compile_with_make(project_definition)? })
        }
        // satisfy the compiler
        else {
            Ok(Binary { info: CompilationInfo { warnings: None, errors: None, compiled: false } })
        }
    }

    fn exists(project_definition: &ProjectDefinition) -> bool {
        Path::new(&project_definition.binary_path).is_file()
    }

    fn compile_with_make(project_definition: &ProjectDefinition) -> Result<CompilationInfo, CompileError> {
        let makefile_path = project_definition.makefile_path.as_ref().unwrap();
        if !Path::new(&format!("{}/Makefile", &makefile_path)).is_file() {
            return Err(CompileError::MakefileNotFound(makefile_path.clone()))
        }

        let mut make_cmd = Command::new("make");
        make_cmd.current_dir(makefile_path);
        make_cmd.stderr(Stdio::piped());
        make_cmd.stdout(Stdio::piped());
        make_cmd.args(project_definition.make_targets.clone().unwrap_or(vec![]));

        let mut warnings: Option<HashMap<String, i32>> = None;
        match make_cmd.output() {
            Ok(res) => {
                let errors = String::from_utf8(res.stderr).unwrap_or_default();
                let re_warnings = Regex::new(r"warning: .*? \[-W(?P<warn>[^\]]+)\]").unwrap();
                if res.status.code().unwrap_or(-1) != 0 {
                    Ok(CompilationInfo{ compiled: false, errors: Some(errors), warnings })
                }
                else {
                    println!("Compilation successful!");
                    //checking for warnings...
                    let mut warns = HashMap::<String, i32>::new();
                    for cap in re_warnings.captures_iter(&errors) {
                        let warn = String::from(&cap["warn"]);
                        let entry = warns.entry(warn).or_insert(0);
                        *entry += 1;
                    }
                    if !warns.is_empty() {
                        println!("Detected compiler warnings:");
                        for (warn, amount) in warns.iter_mut() {
                            println!("  {}: {}", warn, *amount);
                        }
                        warnings = Some(warns);
                    }
                    Ok(CompilationInfo { compiled: true, errors: None, warnings })
                }
            }
            Err(err) => {
                Err(CompileError::MakeFailed(err))
            }
        }
    }
}

