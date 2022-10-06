use crate::busmap::BusMap;
use crate::parser::*;
use crate::scanner::Scanner;
use crate::simulator::{Bus, Chip, Port, Simulator};
use crate::test_parser::*;
/// For dealing with nand2tetris tests
use crate::test_scanner::TestScanner;
use bitvec::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::io::{prelude::*, BufReader};
use std::path::PathBuf;
use std::ptr;
use std::rc::Rc;

fn test_input_to_bitvec(input: &InputValue) -> BitVec<u16, Msb0> {
    match input.number_system {
        NumberSystem::Decimal => {
            let num: i16 = input.value.parse().unwrap();
            let mut raw = [0u16; 1];
            raw.view_bits_mut::<Msb0>().store_le(num);
            let bits = raw.view_bits::<Msb0>();
            bits.to_bitvec()
        }
        NumberSystem::Binary => {
            let bools: Vec<bool> = input
                .value
                .chars()
                .map(|c| match c {
                    '0' => false,
                    '1' => true,
                    _ => {
                        panic!("expected 0 or 1");
                    }
                })
                .collect();
            BitVec::from_iter(bools)
        }
        NumberSystem::Hex => {
            panic!("hex number system not supported yet.");
        }
        NumberSystem::String => {
            // this isn't a bit vector - what to do?
            panic!("string number system not supported yet.");
        }
    }
}

fn bitvec_to_vecbool(bv: BitVec<u16, Msb0>) -> Vec<Option<bool>> {
    let mut res = Vec::new();
    for bit in bv {
        res.push(Some(bit));
    }
    res
}

/// Reads a nand2tetris cmp file and returns a busmap of values
fn read_cmp(
    path: &PathBuf,
    test_script: &TestScript,
    ports: &HashMap<String, Port>,
) -> Vec<BusMap> {
    let mut res: Vec<BusMap> = Vec::new();
    let file = fs::File::open(path).unwrap_or_else(|_| panic!("No such cmp file {:?}", path));
    let buf = BufReader::new(file);

    let mut lines = buf.lines();

    // Read header line and determine order of ports
    let mut header = lines.next().unwrap().expect("Corrupted cmp file");
    header.retain(|c| !c.is_whitespace());

    let port_order: Vec<String> = header[1..header.len() - 1]
        .split('|')
        .map(|p| p.to_string())
        .collect();

    while let Some(Ok(l)) = lines.next() {
        if l.is_empty() {
            continue;
        }
        let mut step_result = BusMap::new();
        let mut line = l.clone();
        line.retain(|c| !c.is_whitespace());
        for (i, v) in line[1..line.len() - 1].split('|').enumerate() {
            let number_system = test_script.output_list[i].number_system.clone();
            if number_system == NumberSystem::String {
                continue;
            }

            // Ignore wildcard expected output.
            if v.contains('*') {
                continue;
            }

            let bitvec_value = test_input_to_bitvec(&InputValue {
                number_system: number_system.clone(),
                value: v.to_string(),
            });

            let mut value = bitvec_to_vecbool(bitvec_value);
            value.reverse();

            let port_width = ports
                .get(&port_order[i])
                .unwrap_or_else(|| panic!("Corrupt .cmp file"))
                .width;

            value.truncate(port_width);
            value.reverse();
            step_result.create_bus(&port_order[i], value.len()).unwrap();
            let bus = Bus {
                name: port_order[i].clone(),
                range: Some(0..value.len()),
            };
            step_result.insert_option(&bus, value);
        }
        res.push(step_result);
    }

    res
}

pub fn run_test(test_script_path: &str) {
    // Parse the test script
    let test_pathbuf = PathBuf::from(test_script_path);
    let test_contents = read_test(&test_pathbuf);
    let mut test_scanner = TestScanner::new(test_contents.as_str(), test_pathbuf.clone());
    let mut test_parser = TestParser {
        scanner: &mut test_scanner,
    };
    let test_script = test_parser.parse().expect("Parse failure");
    let hdl_path = test_pathbuf.parent().unwrap().join(&test_script.hdl_file);

    // Create simulator for HDL file referenced by test script.
    let base_path = hdl_path.parent().unwrap().to_str().unwrap();
    let hdl_file = hdl_path.file_name().unwrap().to_str().unwrap();
    let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(base_path));
    let contents = provider.get_hdl(hdl_file).unwrap();
    let mut scanner = Scanner::new(contents.as_str(), provider.get_path(hdl_file));
    let mut parser = Parser {
        scanner: &mut scanner,
    };
    let hdl = match parser.parse() {
        Ok(x) => x,
        Err(x) => {
            println!("{}", x);
            std::process::exit(1);
        }
    };

    let chip = match Chip::new(
        &hdl,
        ptr::null_mut(),
        &provider,
        false,
        &test_script.generics,
    ) {
        Ok(x) => x,
        Err(x) => {
            println!("{}", x);
            std::process::exit(1);
        }
    };

    let mut simulator = Simulator::new(chip);

    let hdl_contents = fs::read_to_string(hdl_path.clone()).expect("Unable to read HDL file.");
    let mut scanner = Scanner::new(hdl_contents.as_str(), hdl_path);
    let mut parser = Parser {
        scanner: &mut scanner,
    };
    let hdl = parser.parse().expect("Parse error");
    let chip = Chip::new(
        &hdl,
        ptr::null_mut(),
        &provider,
        false,
        &test_script.generics,
    )
    .expect("Chip creation error");

    let ports = chip.ports;
    let compare_path = test_pathbuf
        .parent()
        .unwrap()
        .join(&test_script.compare_file);
    let expected = read_cmp(&compare_path, &test_script, &ports);

    let mut inputs = BusMap::new();
    let mut cmp_idx = 0;
    for step in test_script.steps {
        let mut outputs = BusMap::new();
        for instruction in &step.instructions {
            match instruction {
                Instruction::Set(port, value) => {
                    let width = ports
                        .get(port)
                        .unwrap_or_else(|| panic!("No width for port {}", port))
                        .width;
                    let mut bool_values = bitvec_to_vecbool(test_input_to_bitvec(value));
                    bool_values.reverse();
                    bool_values.truncate(width);
                    bool_values.reverse();
                    inputs.create_bus(port, bool_values.len()).unwrap();
                    inputs.insert_option(&Bus::from(port.clone()), bool_values);
                }
                Instruction::Eval => {
                    outputs = match simulator.simulate(&inputs) {
                        Ok(x) => x,
                        Err(x) => {
                            println!("{}", x);
                            std::process::exit(1);
                        }
                    };
                    print!(".");
                }
                Instruction::Output => {
                    assert_le!(expected[cmp_idx], outputs.clone(), "Step: {}", cmp_idx + 1);
                    cmp_idx += 1;
                }
                Instruction::Tick => {
                    outputs = simulator.simulate(&inputs).expect("simulation failure");
                }
                Instruction::Tock => {
                    simulator.tick().expect("Tick failure");
                    outputs = simulator.simulate(&inputs).expect("simulation failure");
                }
            }
        }
    }

    println!("OK!");
}

fn read_test(path: &PathBuf) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| panic!("Unable to read test file {:?}.", path))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::Path;

    fn construct_path(path: &PathBuf) -> PathBuf {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.join("resources").join("tests").join(path)
    }

    #[test]
    fn test_nand2tetris_solution_not() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Not.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_and() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/And.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_or() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Or.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_xor() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Xor.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_mux() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Mux.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_dmux() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/DMux.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_not16() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Not16.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_and16() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/And16.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_mux16() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Mux16.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_dmux4way() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/DMux4Way.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_dmux8way() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/DMux4Way.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_mux4way16() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Mux4Way16.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_or8way() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Or8Way.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_halfadder() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/HalfAdder.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_fulladder() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/HalfAdder.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_alu() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/ALU.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_bit() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Bit.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_register() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Register.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_ram8() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/RAM8.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_ram512() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/RAM512.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_ram4k() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/RAM4K.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_ram16k() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/RAM16K.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_add16() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Add16.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_inc16() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/Inc16.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_pc() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/PC.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_nand2tetris_solution_cpu() {
        let path = construct_path(&PathBuf::from("nand2tetris/solutions/CPU.tst"));
        run_test(path.to_str().unwrap());
    }

    // #[test]
    // fn test_arm_ops_sasmc() {
    //     let path = construct_path(&PathBuf::from("arm/OpsSASMC.tst"));
    //     run_test(path.to_str().unwrap());
    // }

    #[test]
    fn test_arm_add16() {
        let path = construct_path(&PathBuf::from("arm/Add16.tst"));
        run_test(path.to_str().unwrap());
    }

    #[test]
    fn test_arm_ops_mux8way3() {
        let path = construct_path(&PathBuf::from("arm/Mux8Way3.tst"));
        run_test(path.to_str().unwrap());
    }

    // #[test]
    // fn test_arm_cpu() {
    //     let path = construct_path(&PathBuf::from("arm/CPU.tst"));
    //     run_test(path.to_str().unwrap());
    // }
}
