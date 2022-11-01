use serde_derive::Deserialize;


#[derive(Clone, Debug, Deserialize)]
pub struct ProjectDefinition {
    pub binary_path: String,
    pub makefile_path: Option<String>,
    pub make_targets: Option<Vec<String>>,
    pub global_timeout : Option<u64>,
    pub valgrind_flags : Option<Vec<String>>,
    pub valgrind_log_folder: Option<String>,
    pub diff_table_width: Option<u64>,
    pub use_valgrind: Option<bool>,
}

