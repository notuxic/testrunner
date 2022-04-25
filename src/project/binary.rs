use std::fmt;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use regex::Regex;
use serde_derive::Serialize;
use super::definition::ProjectDefinition;


#[derive(Debug)]
pub enum GenerationError {
    ConfigErrorIO,
    BinaryRequired,
    VgLogNotFound,
    VgLogParseError,
    CouldNotMakeBinary,
    MissingCLIDependency(String),
    IOMismatch,
}

#[derive(Debug)]
pub enum CompileError {
    BinaryNotFound,
    CompilationFailed,
    MakefileNotDefined,
    MakefileNotFound,
    MakeFailed,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompileInfo {
    pub warnings: Option<HashMap<String, i32>>,
    pub errors: Option<String>,
    pub compiled: bool,
}

#[derive(Clone, Debug)]
pub struct Binary {
    pub project_definition: ProjectDefinition,
    pub info: CompileInfo,
}

impl Binary {
    pub fn from_project_definition(proj_def: &ProjectDefinition) -> Result<Self, CompileError> {
        let retvar = Binary {
            project_definition: proj_def.clone(),
            info: CompileInfo {
                errors: None,
                warnings: None,
                compiled: false,
            },
        };
        Ok(retvar)
    }

    pub fn exists(&self) -> bool {
        Path::new(&self.project_definition.binary_path).is_file()
    }

    pub fn compile(&mut self) -> Result<(), CompileError> {
        // use pre-compiled binary
        if self.project_definition.make_targets.is_none() {
            if self.exists() {
                return Ok(());
            }
            else {
                return Err(CompileError::BinaryNotFound);
            }
        }
        // use `make`
        else if self.project_definition.make_targets.is_some() {
            self.compile_with_make()
        }
        // satisfy the compiler
        else {
            Ok(())
        }
    }

    pub fn compile_with_make(&mut self) -> Result<(), CompileError> {
        let makefile_path = match &self.project_definition.makefile_path {
            Some(mk_path) => if Path::new(&format!("{}/Makefile", &mk_path)).is_file() {
                    mk_path.clone()
                }
                else {
                    return Err(CompileError::MakefileNotFound);
                },
            None => {
                return Err(CompileError::MakefileNotDefined);
            }
        };

        let mut make_cmd = Command::new("make");
        make_cmd.current_dir(makefile_path);
        make_cmd.stderr(Stdio::piped());
        make_cmd.stdout(Stdio::piped());
        make_cmd.args(self.project_definition.make_targets.clone().unwrap_or(vec![]));

        match make_cmd.output() {
            Ok(res) => {
                let errorstring = String::from_utf8(res.stderr).unwrap_or_default();
                let re_warnings = Regex::new(r"warning: .*? \[-W(?P<warn>[^\]]+)\]").unwrap();
                if res.status.code().unwrap_or(-1) != 0 {
                    self.info.compiled = false;
                    self.info.errors = Some(errorstring);
                    println!("Compilation failed!");
                    Err(CompileError::CompilationFailed)
                }
                else {
                    self.info.compiled = true;
                    println!("Compilation successful!");
                    //checking for warnings...
                    let mut warnings = HashMap::<String, i32>::new();
                    for cap in re_warnings.captures_iter(&errorstring) {
                        let warn = String::from(&cap["warn"]);
                        let entry = warnings.entry(warn).or_insert(0);
                        *entry += 1;
                    }
                    if !warnings.is_empty() {
                        println!("Detected compiler warnings:");
                        for (warn, amount) in warnings.iter_mut() {
                            println!("  {}: {}", warn, *amount);
                        }
                        self.info.warnings = Some(warnings);
                    }
                    Ok(())
                }
            }
            Err(err) => {
                println!("Compilation failed: {:?}", err);
                Err(CompileError::MakeFailed)
            }
        }
    }
}

impl std::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GenerationError: {}",
            match self {
                GenerationError::ConfigErrorIO => "ConfigErrorIO".to_string(),
                GenerationError::BinaryRequired => "BinaryRequired".to_string(),
                GenerationError::VgLogNotFound => "VgLogNotFound".to_string(),
                GenerationError::VgLogParseError => "VgLogParseError".to_string(),
                GenerationError::CouldNotMakeBinary => "CouldNotMakeBinary".to_string(),
                GenerationError::MissingCLIDependency(dep) => format!("MissingCLIDependency({})", &dep),
                GenerationError::IOMismatch => "IOMismatch".to_string(),
            }
        )
    }
}

