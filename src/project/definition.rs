use serde_derive::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct ProjectDefinition {
    pub project_name: String,
    pub makefile_path: Option<String>,
    pub maketarget: Option<String>,
    pub lib_path: Option<String>,
    pub global_timeout : Option<u64>,
    pub valgrind_flags : Option<Vec<String>>,
    #[serde(skip)]
    pub verbose: bool,
    #[serde(skip)]
    pub diff_mode: String,
    #[serde(skip)]
    pub protected_mode: bool,
    #[serde(skip)]
    pub ws_hints: bool,
    pub table_width: Option<u64>,
}

