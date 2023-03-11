//! This module is responsible for converting nand2tetris test scripts into
//! Modelsim testbenches.

use crate::error::{ErrorKind, N2VError, TransformedError};
use crate::parser::{parse_hdl_path, FileReader, HdlProvider, Parser};
use crate::scanner::Scanner;
use crate::test_parser::{OutputFormat, TestScript};
use crate::test_script::{bitvec_to_vecbool, parse_test, test_input_to_bitvec};
use crate::vhdl::{
    keyw, AssertVHDL, AssignmentVHDL, BusVHDL, LiteralVHDL, PortMappingVHDL, Process, Signal,
    SignalRhs, Statement, VhdlComponent, VhdlEntity, WaitVHDL,
};
use crate::ChipHDL;

use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

/// This structure represents a Modelsim testbench.
pub struct TestBench {
    /// Name of chip being tested.
    chip: ChipHDL,
    /// Signals required for inputs/outputs.
    signals: TestbenchSignals,
    /// Individual steps to perform.
    instructions: Vec<Statement>,
}

struct TestbenchSignals {
    value: Vec<Signal>,
}

impl From<&OutputFormat> for Signal {
    fn from(o: &OutputFormat) -> Self {
        Signal {
            name: o.port_name.clone(),
            width: crate::expr::GenericWidth::Terminal(crate::expr::Terminal::Num(
                o.output_columns,
            )),
        }
    }
}

impl From<&Vec<OutputFormat>> for TestbenchSignals {
    fn from(output_list: &Vec<OutputFormat>) -> Self {
        let value = output_list.iter().map(Signal::from).collect();
        TestbenchSignals { value }
    }
}

impl TryFrom<&TestScript> for TestBench {
    type Error = Box<dyn Error>;

    fn try_from(test_script: &TestScript) -> Result<Self, Box<dyn Error>> {
        let (hdl, _) = parse_hdl_path(&test_script.hdl_path)?;
        let cmp = crate::test_script::read_cmp(test_script)?;
        let mut cmp_i = cmp.iter();

        let mut instructions = Vec::new();
        for step in &test_script.steps {
            for inst in &step.instructions {
                match inst {
                    crate::test_parser::Instruction::Set(port_name, port_value) => {
                        // bitvec_to_vecbool
                        let tib = test_input_to_bitvec(port_value);
                        let btv = bitvec_to_vecbool(tib);

                        // FIXME: remove unwrap
                        // FIXME: only works for binary system where length of string
                        //        is width of port.
                        let fixme: Vec<bool> = btv
                            .iter()
                            .rev()
                            .take(port_value.value.len())
                            .map(|x| x.unwrap())
                            .collect();

                        instructions.push(Statement::Assignment(AssignmentVHDL {
                            left: BusVHDL {
                                name: port_name.clone(),
                                start: None,
                                end: None,
                            },
                            right: SignalRhs::Literal(LiteralVHDL { values: fixme }),
                        }));
                    }
                    crate::test_parser::Instruction::Tick => {}
                    crate::test_parser::Instruction::Tock => {}
                    crate::test_parser::Instruction::Output => {
                        let next_cmp = cmp_i.next().unwrap();
                        for b in next_cmp.keys() {
                            let next_bus = next_cmp.get_name(&b);

                            // For some reason cannot impl try_from because
                            // it collides with core try_from.
                            // https://github.com/rust-lang/rust/issues/50133
                            //
                            // So we just make the LiteralVHDL here.
                            //
                            // FIXME:
                            let lvhdl_values: Vec<bool> =
                                next_bus.iter().map(|x| x.unwrap()).collect();
                            let lvhdl = LiteralVHDL {
                                values: lvhdl_values,
                            };

                            instructions.push(Statement::Assert(AssertVHDL {
                                signal_name: b,
                                signal_value: lvhdl,
                                report_msg: String::from("generic report message"),
                            }));
                        }
                    }
                    crate::test_parser::Instruction::Eval => {
                        instructions.push(crate::vhdl::Statement::Wait(WaitVHDL {}));
                    }
                };
            }
        }

        Ok(TestBench {
            chip: hdl,
            signals: TestbenchSignals::from(&test_script.output_list),
            instructions,
        })
    }
}

impl TryFrom<&TestBench> for VhdlEntity {
    type Error = Box<dyn Error>;

    fn try_from(test_bench: &TestBench) -> Result<Self, Box<dyn Error>> {
        let name = test_bench.chip.name.clone() + "_tst";
        let generics = Vec::new();
        let ports = Vec::new();
        let signals = test_bench.signals.value.clone();

        // Dependencies of the test script are the chip being
        // tested + dependencies of the chip being tested.
        let chip_vhdl = VhdlEntity::try_from(&test_bench.chip)?;

        let mut port_mappings = Vec::new();
        for port in &test_bench.chip.ports {
            port_mappings.push(PortMappingVHDL {
                wire_name: port.name.value.clone(),
                port: BusVHDL {
                    name: port.name.value.clone(),
                    start: None,
                    end: None,
                },
                wire: BusVHDL {
                    name: port.name.value.clone(),
                    start: None,
                    end: None,
                },
            });
        }

        // Only component is the chip being tested.
        let mut statements = vec![Statement::Component(VhdlComponent {
            unit: keyw(&test_bench.chip.name),
            generic_params: Vec::new(),
            port_mappings,
        })];

        let mut process_statements = Vec::new();

        for inst in &test_bench.instructions {
            process_statements.push(inst.clone());
        }

        statements.push(Statement::Process(Process {
            statements: process_statements,
        }));

        Ok(VhdlEntity {
            name,
            generics,
            ports,
            statements,
            signals,
            dependencies: HashSet::from([chip_vhdl]),
        })
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
    let test_bench: TestBench = TestBench::try_from(&test_script)?;

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
    let vhdl_entity = VhdlEntity::try_from(&test_bench)?;
    testbench_file.write_all(vhdl_entity.to_string().as_bytes())?;

    let source_code = fs::read_to_string(&test_script.hdl_path)?;
    let mut scanner = Scanner::new(&source_code, test_script.hdl_path);
    let base_path = scanner.path.parent().unwrap();
    let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(base_path));
    let mut parser = Parser::new(&mut scanner, provider);
    let hdl = parser.parse()?;
    let chip_vhdl = VhdlEntity::try_from(&hdl)?;

    let quartus_dir = Path::new(&output_dir);
    let _ = crate::vhdl::QuartusProject::new(hdl, chip_vhdl, quartus_dir.to_path_buf());

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

        // Create the work library.
        let status = Command::new("vlib")
            .args(["work"])
            .current_dir(&temp_dir)
            .status()
            .expect("Failed to execute vlib");
        assert!(status.success());

        // 2. Run Modelsim and assert that all tests passed.
        let status = Command::new("vcom")
            .args(["And.tst.vhdl"])
            .current_dir(&temp_dir)
            .status()
            .expect("Failed to execute vcom");
        assert!(status.success());

        // FIXME: How to pass in length of test?
        let status = Command::new("vsim")
            .args(["-c", "work.and_tst", "-do", "\"run 100\"; quit"])
            .current_dir(&temp_dir)
            .status()
            .expect("Failed to execute vsim");
        assert!(status.success());
    }
}
