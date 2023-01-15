//! This module is responsible for converting nand2tetris test scripts into
//! Modelsim testbenches.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use bitvec::prelude::*;

use crate::error::{ErrorKind, N2VError, TransformedError};
use crate::parser::parse_hdl_path;
use crate::test_parser::TestScript;
use crate::test_script::parse_test;
use crate::{Bus, PortMapping};

/// This structure represents a Modelsim testbench.
pub struct TestBench {
    /// Name of chip being tested.
    chip_name: String,
    /// Signals required for inputs/outputs.
    signals: Vec<Bus>,
    /// Mapping of inputs and outputs from signals to chip ports.
    port_maps: Vec<PortMapping>,
    /// Individual steps to perform.
    instructions: Vec<Instruction>,
}

/// An single action for the simulator to perform.
pub enum Instruction {
    /// (port name, port value) set port name to port value.
    Assign(String, BitVec<u16, Msb0>),
    /// Wait for usize nanonseconds.
    Wait(usize),
    /// (signal name, value, report message)
    Assert(String, BitVec<u16, Msb0>, String),
}

impl TryFrom<TestScript> for TestBench {
    type Error = Box<dyn Error>;

    fn try_from(test_script: TestScript) -> Result<Self, Box<dyn Error>> {
        let (hdl, _) = parse_hdl_path(&test_script.hdl_path)?;

        Ok(TestBench {
            chip_name: hdl.name,
            signals: Vec::new(),
            port_maps: Vec::new(),
            instructions: Vec::new(),
        })
    }
}

/// Converts a TestBench into VHDL.
impl fmt::Display for TestBench {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Libraries
        writeln!(f, "library ieee;")?;
        writeln!(f, "use ieee.std_logic_1164.all;")?;

        // Entity declaration
        let entity_name = self.chip_name.clone() + "_test";
        writeln!(f)?;
        writeln!(f, "-- Empty entity because this is just a test script.")?;
        writeln!(f, "entity {} is", entity_name)?;
        writeln!(f, "end entity {};", entity_name)?;

        // === Begin Architecture ===
        writeln!(f)?;
        writeln!(f, "architecture test_arch of {} is", entity_name)?;
        writeln!(f, "begin")?;

        // Signals. We need to declare inputs and outputs of chip that we are testing.
        for s in &self.signals {
            // writeln!(f, "{}", crate::vhdl::signal_declaration(s))?;
        }

        // === End Architecture ===
        writeln!(f, "end architecture test_arch;")?;

        // End with a newline.
        writeln!(f)
    }
}

/// Converts a nand2tetris test script file to a VHDL testbench to be run
/// with Modelsim. This will convert the test script itself, and the
/// HDL for the chip that is being tested.
///
/// - `output_dir`: The directory to create that will house the generated
///     VHDL files. This directory must exist at the time of calling the
///     function.
/// - `test_script_path`: Path to the test script to convert.
pub fn synth_vhdl_test(output_dir: &Path, test_script_path: &Path) -> Result<(), Box<dyn Error>> {
    let test_script = parse_test(test_script_path)?;
    let test_bench: TestBench = TestBench::try_from(test_script)?;

    let test_script_filename = match test_script_path.file_name() {
        None => {
            return Err(Box::new(N2VError {
                msg: String::from("Invalid file name for source test script"),
                kind: ErrorKind::IOError,
            }))
        }
        Some(x) => x,
    };

    let test_bench_path = output_dir
        .join(test_script_filename)
        .with_extension("tst.vhdl");
    let mut testbench_file = match File::create(&test_bench_path) {
        Err(e) => {
            return Err(Box::new(TransformedError {
                msg: format!(
                    "Error creating test bench file {}",
                    &test_bench_path.display()
                ),
                kind: ErrorKind::IOError,
                source: Some(Box::new(e)),
            }))
        }
        Ok(f) => f,
    };
    let test_bench_vhdl = test_bench.to_string();
    testbench_file.write_all(test_bench_vhdl.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::tempdir;

    #[test]
    // Test that Modelsim simulation passes for And chip.
    fn test_and() {
        // Synthesize Modelsim test bench for input .tst script
        let tst_path = PathBuf::from("resources/tests/nand2tetris/solutions/And.tst");
        let temp_dir = tempdir().unwrap();
        println!("Temp dir: {}", temp_dir.path().display());

        let synth_result = synth_vhdl_test(temp_dir.path(), &tst_path);
        if synth_result.is_err() {
            println!("{}", synth_result.unwrap_err());
            panic!();
        }

        // 2. Run Modelsim and assert that all tests passed.
        let status = Command::new("vcom")
            .args(["And.tst.vhdl"])
            .current_dir(&temp_dir)
            .status()
            .expect("Failed to execute vcom");

       assert!(status.success()); 
    }
}
