use serde_derive::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct ProjectDefinition {
    pub binary_path: String,
    pub makefile_path: Option<String>,
    pub make_targets: Option<Vec<String>>,
    pub library_path: Option<String>,
    pub global_timeout : Option<u64>,
    pub valgrind_flags : Option<Vec<String>>,
    pub valgrind_log_folder: Option<String>,
    #[serde(skip)]
    pub verbose: bool,
    #[serde(skip)]
    pub diff_delim: String,
    #[serde(skip)]
    pub protected_mode: bool,
    #[serde(skip)]
    pub ws_hints: bool,
    #[serde(skip)]
    pub sudo: Option<String>,
    pub diff_table_width: Option<u64>,
    pub use_valgrind: Option<bool>,
}

