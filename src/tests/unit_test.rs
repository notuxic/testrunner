use libloading as lib;
use super::test::{DiffKind, Test, TestCaseKind, TestMeta};
use super::testresult::TestResult;
use super::testcase::Testcase;
use crate::project::binary::{Binary, GenerationError};
use crate::project::definition::ProjectDefinition;

#[allow(dead_code)]
pub struct UnitTest {
    meta: TestMeta,
    subname: String,
    fname: String,
    argv: Vec<String>,
}

impl Test for UnitTest {
    fn get_test_meta(&self) -> &TestMeta { &self.meta }

    fn run(&self) -> Result<TestResult, GenerationError> {
        if self.meta.projdef.protected_mode && self.meta.protected {
            println!("\nStarting testcase {}: ********", self.meta.number);
        }
        else {
            println!("\nStarting testcase {}: {}", self.meta.number, self.meta.name);
        }

        if let Err(e) = run(self) {
            println!("Error running unit test {}: {}", self.meta.number, e);
            return Err(GenerationError::ConfigErrorUnit);
        }

        if self.meta.projdef.protected_mode && self.meta.protected {
            println!("\nFinished testcase {}: ********", self.meta.number);
        }
        else {
            println!("\nFinished testcase {}: {}", self.meta.number, self.meta.name);
        }

        let add_diff = self.get_add_diff();

        Ok(TestResult {
            //diff2 : None,
            distance_percentage: None,
            kind: TestCaseKind::UnitTest,
            diff: None,
            add_diff,
            implemented: None,
            passed: false,
            result: String::from("Not yet implemented"),
            ret: None,
            exp_ret: None,
            vg_errors: 0,
            vg_warnings: 0,
            vg_logfile: String::from(""),
            command_used: String::from(format!("./{} {}", &self.meta.projdef.project_name, &self.argv.clone().join(" "))),
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
        let diff_kind = match &testcase.add_diff_mode {
            Some(text) => {
                if text.eq_ignore_ascii_case("binary") {
                    Some(DiffKind::Binary)
                }
                else {
                    Some(DiffKind::PlainText)
                }
            },
            None => None,
        };
        let retvar = UnitTest {
            meta: TestMeta {
                number,
                name: testcase.name.clone(),
                desc: testcase.description.clone(),
                timeout: testcase.timeout,
                projdef: projdata.clone(),
                kind: TestCaseKind::UnitTest,
                add_diff_kind: diff_kind,
                add_in_file: testcase.add_in_file.clone(),
                add_exp_file: testcase.add_exp_file.clone(),
                protected: testcase.protected.unwrap_or(false),
            },
            fname: testcase.fname.as_ref().unwrap_or(&String::new()).clone(),
            argv: testcase.args.as_ref().unwrap_or(&vec![String::new()]).clone(),
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
