use std::fmt;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use regex::Regex;
use super::definition::ProjectDefinition;

#[derive(Debug)]
pub enum GenerationError {
    None,
    MakeFileRequired,
    ConfigErrorIO,
    BinaryRequired,
    VgLogNotFound,
    VgLogParseError,
    CouldNotMakeBinary,
    ConfigErrorUnit,
}

#[derive(Debug)]
pub enum CompileError {
    None,
    NoMakefile,
    MakeFailed,
    NoIssuesReported,
}

#[derive(Clone, Debug)]
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

    pub fn compile(&mut self) -> Result<(), CompileError> {
        let makefile_path = match &self.project_definition.makefile_path {
            Some(expr) => expr.clone(),
            None => {
                return Err(CompileError::NoMakefile);
            }
        };

        let mut make_cmd = Command::new("make");
        make_cmd.current_dir(makefile_path);
        make_cmd.stderr(Stdio::piped());
        make_cmd.stdout(Stdio::piped());
        if self.project_definition.maketarget.is_some() {
            make_cmd.arg(
                self.project_definition
                .maketarget
                .clone()
                .unwrap_or(String::new()),
            );
        }
        match make_cmd.output() {
            Ok(res) => {
                let errorstring = String::from_utf8(res.stderr).unwrap_or_default();
                let re_warnings = Regex::new(r"warning: .*? \[-W(?P<warn>[^\]]+)\]").unwrap();
                if res.status.code().unwrap_or(-1) != 0 {
                    self.info.compiled = false;
                    self.info.errors = Some(errorstring);
                    println!("Compilation failed!");
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
                        // each entry was detected twice, thus half the amount now
                        println!("Detected compiler warnings:");
                        for (warn, amount) in warnings.iter_mut() {
                            *amount /= 2;
                            println!("  {}: {}", warn, *amount);
                        }
                        self.info.warnings = Some(warnings);
                    }
                }
            }
            Err(err) => {
                println!("Compilation failed: {:?}", err);
                return Err(CompileError::MakeFailed);
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GenerationError: {}",
            match self {
                GenerationError::None => "None".to_string(),
                GenerationError::MakeFileRequired => "MakeFileRequired".to_string(),
                GenerationError::ConfigErrorIO => "ConfigErrorIO".to_string(),
                GenerationError::BinaryRequired => "BinaryRequired".to_string(),
                GenerationError::VgLogNotFound => "VgLogNotFound".to_string(),
                GenerationError::VgLogParseError => "VgLogParseError".to_string(),
                GenerationError::CouldNotMakeBinary => "CouldNotMakeBinary".to_string(),
                GenerationError::ConfigErrorUnit => "ConfigErrorUnit".to_string()
            }
        )
    }
}

