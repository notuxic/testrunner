use crate::UnitTest;
use libloading as lib;

pub fn run(test: &UnitTest) -> Result<(), Box<dyn std::error::Error>> {
    let test_lib = lib::Library::new(test.meta.projdata.lib_path.as_ref().expect("test library unknown"))?;
    unsafe {
        let func: lib::Symbol<unsafe extern fn() -> ()> = test_lib.get(test.fname.as_str().as_bytes())?;
        func();
    }
    Ok(())
}
