use serde_derive::Deserialize;
use crate::project::definition::ProjectDefinition;

#[derive(Debug, Deserialize)]
pub struct Testcase {
    pub name: String,
    pub subname: Option<String>,
    pub testcase_type: String,
    pub description: Option<String>,
    pub args: Option<Vec<String>>,
    pub in_file: Option<String>,
    pub exp_file: Option<String>,
    pub in_string: Option<String>,
    pub exp_string: Option<String>,
    pub io_file: Option<String>,
    pub io_prompt: Option<String>,
    pub exp_retvar: Option<i32>,
    pub add_diff_mode: Option<String>,
    pub add_out_file: Option<String>,
    pub add_exp_file: Option<String>,
    pub timeout: Option<u64>,
    pub env_vars: Option<String>,
    pub protected: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct TestDefinition {
    pub project_definition: ProjectDefinition,
    pub testcases: Vec<Testcase>,
}

