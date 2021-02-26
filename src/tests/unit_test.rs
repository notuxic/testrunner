use libloading as lib;
use super::test::{Test, TestCaseKind, TestMeta};
use super::testresult::TestResult;
use super::testcase::Testcase;
use crate::project::binary::{Binary, GenerationError};
use crate::project::definition::ProjectDefinition;

#[allow(dead_code)]
pub struct UnitTest {
    meta: TestMeta,
    subname: String,
    fname: String,
    argv: String,
}

impl Test for UnitTest {
    fn run(&self) -> Result<TestResult, GenerationError> {
        if let Err(e) = run(self) {
            println!("Error running unit test: {}", e);
            return Err(GenerationError::ConfigErrorUnit);
        }

        Ok(TestResult {
            //diff2 : None,
            distance_percentage: None,
            compile_warnings: None,
            kind: TestCaseKind::UnitTest,
            diff: None,
            implemented: None,
            passed: false,
            result: String::from("Not yet implemented"),
            ret: None,
            exp_ret: None,
            vg_errors: 0,
            vg_warnings: 0,
            vg_logfile: String::from(""),
            command_used: String::from(format!("./{} {}", &self.meta.projdef.project_name, &self.argv)),
            used_input: String::from(""),
            timeout: false,
            name: self.meta.name.clone(),
            description: self.meta.desc.clone().unwrap_or(String::from("")),
            number: self.meta.number,
            protected: self.meta.protected,
        })
    }

    fn from_saved_tc(
        number: i32,
        testcase: &Testcase,
        projdata: &ProjectDefinition,
        _binary: Option<&Binary>,
    ) -> Result<Self, GenerationError> {
        let retvar = UnitTest {
            meta: TestMeta {
                number,
                name: testcase.name.clone(),
                desc: testcase.description.clone(),
                timeout: testcase.timeout,
                projdef: projdata.clone(),
                kind: TestCaseKind::UnitTest,
                protected: testcase.protected.unwrap_or(false),
            },
            fname: testcase.fname.as_ref().unwrap_or(&String::new()).clone(),
            argv: testcase.args.as_ref().unwrap_or(&String::new()).clone(),
            subname: testcase
                .subname
                .as_ref()
                .map(|s| s.clone())
                .unwrap_or(String::new()),
        };
        Ok(retvar)
    }
}

fn run(test: &UnitTest) -> Result<(), Box<dyn std::error::Error>> {
    let test_lib = lib::Library::new(test.meta.projdef.lib_path.as_ref().expect("test library unknown"))?;
    unsafe {
        let func: lib::Symbol<unsafe extern fn() -> ()> = test_lib.get(test.fname.as_str().as_bytes())?;
        func();
    }
    Ok(())
}

