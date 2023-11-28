//! This module is responsible for converting nand2tetris test scripts into
//! Modelsim testbenches.

use crate::error::{ErrorKind, N2VError, TransformedError};
use crate::expr::{GenericWidth, Terminal};
use crate::opt::optimization::{OptimizationPass, OptimizationInfo};
use crate::opt::sequential::SequentialPass;
use crate::parser::{parse_hdl_path, FileReader, HdlProvider, Parser, Part, Component, Identifier};
use crate::scanner::Scanner;
use crate::simulator::Chip;
use crate::test_parser::{OutputFormat, TestScript};
use crate::test_script::{bitvec_to_vecbool, parse_test, test_input_to_bitvec};
use crate::vhdl::write_quartus_project;
use crate::vhdl::SignalRhs::*;
use crate::vhdl::*;
use crate::ChipHDL;
use crate::PortMapDedupe;

use std::cell::RefCell;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::ptr;
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
        let mut value: Vec<Signal>  = output_list.iter().map(Signal::from).collect();
        value.push(Signal {
            name: "clk".to_string(),
            width: GenericWidth::Terminal(Terminal::Num(1)),
        });
        TestbenchSignals { value }
    }
}

impl TryFrom<&TestScript> for TestBench {
    type Error = Box<dyn Error>;

    fn try_from(test_script: &TestScript) -> Result<Self, Box<dyn Error>> {
        let (mut hdl, _) = parse_hdl_path(&test_script.hdl_path)?;

        // Add a part to the HDL for the chip being tested.
        // Hack for now to trigger component declaration.
        let p = Part::Component(Component {
            name: Identifier::from(hdl.name.as_str()),
            mappings: Vec::new(),
            generic_params: Vec::new(),
        });
        hdl.parts.push(p);

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
                            left: SliceVHDL {
                                name: port_name.clone(),
                                start: None,
                                end: None,
                            },
                            right: SignalRhs::Literal(LiteralVHDL { values: fixme }),
                        }));
                    }
                    crate::test_parser::Instruction::Tick => {
                        instructions.push(crate::vhdl::Statement::Assignment(
                            AssignmentVHDL {
                                left: SliceVHDL {
                                    name: "clk".to_string(),
                                    start: None,
                                    end: None,
                                },
                                right: SignalRhs::Literal(LiteralVHDL { values: vec![false] }),
                            },
                        ));
                        instructions.push(crate::vhdl::Statement::Wait(WaitVHDL {}));
                    }
                    crate::test_parser::Instruction::Tock => {
                        instructions.push(crate::vhdl::Statement::Assignment(
                            AssignmentVHDL {
                                left: SliceVHDL {
                                    name: "clk".to_string(),
                                    start: None,
                                    end: None,
                                },
                                right: SignalRhs::Literal(LiteralVHDL { values: vec![true] }),
                            },
                        ));
                        instructions.push(crate::vhdl::Statement::Wait(WaitVHDL {}));
                    }
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
                                next_bus.iter().rev().map(|x| x.unwrap()).collect();
                            let lvhdl = LiteralVHDL {
                                values: lvhdl_values,
                            };

                            let report_msg = format!("Test failure {}", b);

                            instructions.push(Statement::Assert(AssertVHDL {
                                signal_name: b,
                                signal_value: lvhdl,
                                report_msg,
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
        let chip = Chip::new(
            &test_bench.chip,
            ptr::null_mut(),
            &test_bench.chip.provider,
            false,
            &Vec::new(),
        )?;

        let mut port_mappings = Vec::new();
        for port in &test_bench.chip.ports {
            port_mappings.push(PortMappingVHDL {
                wire_name: port.name.value.clone(),
                port: SliceVHDL {
                    name: port.name.value.clone(),
                    start: None,
                    end: None,
                },
                wire: Slice(SliceVHDL {
                    name: port.name.value.clone(),
                    start: None,
                    end: None,
                }),
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


        // Create a clock port if this is a sequential chip.
        let mut sequential_pass = SequentialPass::new();
        let (_, sequential_pass_info_raw) = sequential_pass.apply(chip.hdl.as_ref().unwrap(), &chip.hdl_provider)?;
        let sequential_pass_info = Rc::new(RefCell::new(sequential_pass_info_raw));

        if let OptimizationInfo::SequentialFlagMap(sequential_flag_map) =
            &*sequential_pass_info.borrow()
        {
            if sequential_flag_map.get(&keyw(&test_bench.chip.name)) == Some(&true) {
                let clock_port_mapping = PortMappingVHDL {
                    wire_name: "clk".to_string(),
                    port: SliceVHDL {
                        name: "clk".to_string(),
                        start: None,
                        end: None,
                    },
                    wire: SignalRhs::Slice(SliceVHDL {
                        name: "clk".to_string(),
                        start: None,
                        end: None,
                    }),
                };

                for part in &mut statements {
                    if let Statement::Component(c) = part {
                        c.port_mappings.push(clock_port_mapping.clone());
                    }
                }
            }
        }

        Ok(VhdlEntity {
            name,
            generics,
            ports,
            statements,
            signals,
            optimization_info: Some(Rc::clone(&sequential_pass_info)),
            chip_hdl: chip.hdl.as_ref().unwrap().clone(),
            hdl_provider: chip.hdl_provider.clone(),
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
    let test_script = match parse_test(test_script_path) {
        Err(e) => {
            return Err(Box::new(TransformedError {
                source: Some(e),
                msg: format!("Unable to parse test script {:?}", test_script_path),
                kind: ErrorKind::IOError,
            }))
        }
        Ok(x) => x,
    };

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
    let mut parser = Parser::new(&mut scanner, provider.clone());
    let hdl = parser.parse()?;

    // Convert HDL to VHDL (VHDl synthesis).
    let mut dedupe_pass = PortMapDedupe::new();
    let (chip_hdl, _) = &dedupe_pass.apply(&hdl, &provider)?;

    let chip = Chip::new(
        chip_hdl,
        ptr::null_mut(),
        &chip_hdl.provider,
        true,
        &Vec::new(),
    )?;
    let chip_vhdl: VhdlEntity = VhdlEntity::try_from(&chip)?;

    let quartus_dir = Path::new(&output_dir);
    let project = crate::vhdl::QuartusProject::new(hdl, chip_vhdl, quartus_dir.to_path_buf());

    match write_quartus_project(&project) {
        Ok(()) => (),
        Err(e) => {
            return Err(Box::new(TransformedError {
                source: Some(e),
                msg: String::from("Unable to write quartus project"),
                kind: ErrorKind::IOError,
            }))
        }
    }

    Ok(())
}

// Only run these tests if the modelsim_tests feature is enabled.
// We need to disable these tests sometimes (GitHub actions) because 
// they depend on Quartus Prime, which is huge. 
#[cfg(all(test, feature = "modelsim_tests"))]
mod test {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::tempdir;

    fn run_test(tst_path: PathBuf, test_chip: &str) {
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
            .args(["*.vhdl"])
            .current_dir(&temp_dir)
            .status()
            .expect("Failed to execute vcom");
        assert!(status.success());

        // FIXME: How to pass in length of test?
        let output = Command::new("vsim")
            .args(["-c", test_chip, "-do", "run 100ns; quit"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to execute vsim");
        let output_str = String::from_utf8(output.stdout).unwrap();
        println!("{}", output_str);
        assert!(output_str.contains("Errors: 0"));
    }

    #[test]
    fn test_and() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/And.tst"),
            "work.and_tst",
        );
    }

    #[test]
    fn test_or() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/Or.tst"),
            "work.or_tst",
        );
    }

    #[test]
    fn test_not() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/Not.tst"),
            "work.not_tst",
        );
    }

    #[test]
    fn test_xor() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/Xor.tst"),
            "work.xor_tst",
        );
    }

    #[test]
    fn test_mux() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/Mux.tst"),
            "work.Mux_tst",
        );
    }

    #[test]
    fn test_dmux() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/DMux.tst"),
            "work.dmux_tst",
        );
    }

    #[test]
    fn test_dmux4way() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/DMux4Way.tst"),
            "work.dmux4way_tst",
        );
    }

    #[test]
    fn test_dmux8way() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/DMux8Way.tst"),
            "work.dmux8way_tst",
        );
    }

    #[test]
    fn test_half_adder() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/HalfAdder.tst"),
            "work.HalfAdder_tst",
        );
    }

    #[test]
    fn test_full_adder() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/FullAdder.tst"),
            "work.FullAdder_tst",
        );
    }

    #[test]
    fn test_not16() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/Not16.tst"),
            "work.Not16_tst",
        );
    }

    #[test]
    fn test_or8way() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/Or8Way.tst"),
            "work.or8way_tst",
        );
    }

    #[test]
    fn test_add16() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/Add16.tst"),
            "work.Add16_tst",
        );
    }

    #[test]
    fn test_bit() {
        run_test(
            PathBuf::from("resources/tests/nand2tetris/solutions/Bit.tst"),
            "work.BIT_tst",
        );
    }
}
