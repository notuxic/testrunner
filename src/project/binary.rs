use std::fmt;
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
    pub warnings: i32,
    pub errors: i32,
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
                errors: 0,
                warnings: 0,
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
                let re = Regex::new(
                    r"(?P<warn>[0-1]*) warnings? and (?P<err>[0-1]*) errors? generated.",
                )
                    .unwrap();
                let re2 = Regex::new(r"(?P<warn>[0-1]*) warnings? generated.").unwrap();
                if res.status.code().unwrap_or(-1) != 0 {
                    self.info.compiled = false;
                    let issues = re.captures_iter(&errorstring).last(); // last match is the best
                    match issues {
                        Some(found_issues) => {
                            self.info.warnings = found_issues["warn"].parse().unwrap_or(-1);
                            self.info.errors = found_issues["err"].parse().unwrap_or(-1);
                        }
                        None => {
                            return Err(CompileError::NoIssuesReported);
                        }
                    }
                } else {
                    self.info.compiled = true;
                    println!("looks good");
                    //checking for warnings...
                    let issues = re2.captures_iter(&errorstring).last();
                    match issues {
                        Some(found_warnings) => {
                            //warnings found => parse them
                            println!("{:?}", &found_warnings["warn"]);
                            self.info.warnings = found_warnings["warn"].parse().unwrap_or(-1);
                        }
                        None => {
                            self.info.errors = 0;
                            self.info.warnings = 0;
                        }
                    }
                }
            }
            Err(err) => {
                println!("noo {:?}", err);
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

