use std::collections::{HashMap, BTreeMap};
use std::fs::read_to_string;
use std::sync::Arc;

use sailfish::{TemplateOnce, RenderError};
use serde::{Deserializer, Deserialize};
use serde_tagged::de::BoxFnSeed;
use thiserror::Error;

use crate::project::binary::{Binary, CompileError};
use crate::project::definition::ProjectDefinition;
use crate::test::io_test::IoTest;
use crate::test::ordio_test::OrdIoTest;
use crate::test::test::{Test, TestingError};
use crate::testresult::testresult::Testresult;


#[derive(Debug, Error)]
pub enum TestrunnerError {
    #[error("config not found: {0}")]
    ConfigNotFound(String),
    #[error("failed parsing config: {0}")]
    ConfigParseError(String),
    #[error(transparent)]
    CompileError(#[from] CompileError),
    #[error(transparent)]
    TestingError(#[from] TestingError),
    #[error("failed rendering testreport: {}", .0.to_string())]
    RenderError(#[from] RenderError),
    #[error("error generating report: {}", .0.to_string())]
    GenerationError(Box<dyn std::error::Error>),
}

#[derive(Debug)]
pub struct TestrunnerOptions {
    pub verbose: bool,
    pub protected_mode: bool,
    pub ws_hints: bool,
    pub sudo: Option<String>,
}

impl Default for TestrunnerOptions {
    fn default() -> Self {
        TestrunnerOptions {
            verbose: false,
            protected_mode: false,
            ws_hints: true,
            sudo: None,
        }
    }
}

#[derive(Deserialize, TemplateOnce)]
#[template(path = "testreport.stpl")]
pub struct Testrunner {
    #[serde(deserialize_with = "Testrunner::deserialize_definition")]
    project_definition: Arc<ProjectDefinition>,
    testcases: Vec<Box<dyn Test + Send + Sync>>,
    #[serde(skip)]
    testresults: Vec<Box<dyn Testresult + Send + Sync>>,
    #[serde(skip)]
    binary: Arc<Binary>,
    #[serde(skip)]
    options: Arc<TestrunnerOptions>,
}

impl Testrunner {
    pub fn from_file(path: &str, options: TestrunnerOptions) -> Result<Self, TestrunnerError> {
        let config = read_to_string(path).map_err(|_| TestrunnerError::ConfigNotFound(path.to_string()))?;
        let mut runner: Self = toml::from_str(&config).map_err(|err| TestrunnerError::ConfigParseError(err.to_string()))?;
        runner.options = Arc::new(options);
        runner.binary = Arc::new(Binary::from_project_definition(&runner.project_definition)?);

        let mut tc_number = 0;
        let project_definition = Arc::downgrade(&runner.project_definition);
        let options = Arc::downgrade(&runner.options);
        let binary = Arc::downgrade(&runner.binary);
        runner.testcases.iter_mut().try_for_each(|tc| {
            tc_number += 1;
            tc.init(tc_number, project_definition.clone(), options.clone(), binary.clone())
        })?;
        Ok(runner)
    }

    pub fn deserialize_definition<'de, D>(deserializer: D) -> Result<Arc<ProjectDefinition>, D::Error>
        where D: Deserializer<'de>
    {
        return Ok(Arc::new(ProjectDefinition::deserialize(deserializer)?));
    }

    pub fn run_tests(&mut self) -> Result<(), TestrunnerError> {
        if !self.binary.info.compiled {
            println!("Compilation failed, skipping tests!");
            return Ok(());
        }

        self.testresults = match self.testcases.iter().try_fold(Vec::with_capacity(self.testcases.len()), |mut acc, tc| {
            acc.push(tc.run()?);
            Ok(acc)
        }) {
            Ok(results) => results,
            Err(err) => return Err(err),
        };
        println!("\nPassed testcases: {} / {}", self.testresults.iter().filter(|test| test.passed()).count(), self.testresults.len());
        Ok(())
    }

    pub fn generate_html_report(self, _protected_mode: bool) -> Result<String, TestrunnerError> {
        Ok(self.render_once()?)
    }

    pub fn generate_json_report(&self) -> Result<String, TestrunnerError> {
        let mut json: HashMap<String, serde_json::Value> = HashMap::new();
        let mut results: Vec<serde_json::Value> = vec![];
        for tc in self.testresults.iter() {
            results.push(tc.get_json_entry()?);
        }
        json.insert("testcases".to_owned(), serde_json::to_value(results).unwrap());
        json.insert("binary".to_owned(), serde_json::to_value(&self.binary.info).unwrap());

        serde_json::to_string_pretty(&json).map_err(|err| TestrunnerError::GenerationError(Box::new(err)))
    }
}

impl<'de> Deserialize<'de> for Box<dyn Test + Send + Sync> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        serde_tagged::de::internal::deserialize(deserializer, "type", get_deserializer_registry())
    }
}

pub type DeserializerRegistry = BTreeMap<&'static str, BoxFnSeed<Box<dyn Test + Send + Sync>>>;
pub fn get_deserializer_registry() -> &'static DeserializerRegistry {
    lazy_static! {
        static ref DESERIALIZER_REGISTRY: DeserializerRegistry = {
            let mut registry = BTreeMap::new();
            registry.insert("IO", BoxFnSeed::new(IoTest::deserialize_trait::<dyn erased_serde::Deserializer>));
            registry.insert("OrdIO", BoxFnSeed::new(OrdIoTest::deserialize_trait::<dyn erased_serde::Deserializer>));
            registry
        };
    }
    &DESERIALIZER_REGISTRY
}

