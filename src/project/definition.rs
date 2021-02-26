use serde_derive::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct ProjectDefinition {
    pub project_name: String,
    pub makefile_path: Option<String>,
    pub maketarget: Option<String>,
    pub lib_path: Option<String>,
    pub global_timeout : Option<i32>,
    pub valgrind_flags : Option<Vec<String>>,
    #[serde(skip)]
    pub verbose: bool,
    #[serde(skip)]
    pub diff_mode: String,
}

