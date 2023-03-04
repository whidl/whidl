// These modules have functions that are only used by main.rs. It is useless
// to warn about dead code here.
#![allow(dead_code)]

mod busmap;
mod error;
mod expr;
mod scanner;
mod simulator;
mod parser;
mod test_scanner;

use crate::busmap::BusMap;
use crate::error::{ErrorKind, N2VError};
use crate::parser::*;
use crate::simulator::{Chip, Simulator};
use expr::*;
use rust_embed::RustEmbed;
use scanner::Scanner;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::ptr;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

#[derive(RustEmbed)]
#[folder = "resources/tests/arm"]
// #[prefix = "prefix/"]
struct HdlAsset;

pub struct EmbedReader;

impl HdlProvider for EmbedReader {
    fn get_hdl(&self, path: &str) -> Result<String, std::io::Error> {
        match HdlAsset::get(path) {
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Unable to get HDL for {}", path),
            )),
            Some(hdl_asset) => Ok(String::from(
                std::str::from_utf8(hdl_asset.data.as_ref()).unwrap(),
            )),
        }
    }

    fn get_path(&self, file_name: &str) -> PathBuf {
        PathBuf::from(file_name)
    }
}

#[wasm_bindgen]
pub fn simulate(s: &str, inputs: &str) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    let mut scanner = Scanner::new(s, PathBuf::from(""));
    let mut parser = Parser {
        scanner: &mut scanner,
    };

    let hdl = match parser.parse() {
        Ok(x) => x,
        Err(e) => return Err(JsValue::from(e.to_string())),
    };

    let provider: Rc<dyn HdlProvider> = Rc::new(EmbedReader);
    let chip = match Chip::new(&hdl, ptr::null_mut(), &provider, false, &Vec::new()) {
        Ok(x) => x,
        Err(e) => return Err(JsValue::from(e.to_string())),
    };
    let mut simulator = Simulator::new(chip);
    let chip_inputs: HashMap<String, Vec<bool>> = serde_json::from_str(inputs)
        .unwrap_or_else(|_| panic!("Unable to parse inputs: {}", inputs));

    let outputs = simulator.simulate(&BusMap::try_from(chip_inputs)
                                     .unwrap_or_else(|_| panic!("A simulation error occured")));
    Ok(format!("{:?}", outputs))
}

#[wasm_bindgen]
pub fn full_table(s: &str) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    let table = match full_table_internal(s, Rc::new(EmbedReader)) {
        Ok(x) => x,
        Err(e) => {
            return Err(JsValue::from(e.to_string()));
        }
    };
    Ok(serde_json::to_string(&table).unwrap())
}

type Table = Vec<Vec<Vec<Option<bool>>>>;

// Returns (column list, row values)
pub fn full_table_internal(
    s: &str,
    provider: Rc<dyn HdlProvider>,
) -> Result<(Vec<String>, Table), Box<dyn Error>> {
    let mut scanner = Scanner::new(s, PathBuf::from(""));
    let mut parser = Parser {
        scanner: &mut scanner,
    };

    let hdl = match parser.parse() {
        Ok(x) => x,
        Err(e) => return Err(e),
    };

    let chip = Chip::new(&hdl, ptr::null_mut(), &provider, false, &Vec::new())?;
    let mut simulator = Simulator::new(chip);

    // get total width of input ports
    let total_width = &hdl
        .ports
        .iter()
        .filter(|p| p.direction == PortDirection::In)
        .fold(0, |acc, p| {
            if let GenericWidth::Terminal(Terminal::Num(w)) = p.width {
                return acc + w;
            }
            panic!("Generic widths not supported");
        });

    let column_names: Vec<String> = hdl.ports.iter().map(|p| p.name.value.clone()).collect();

    let mut column_values: Vec<Vec<Vec<Option<bool>>>> = vec![];

    let base: u32 = 2;
    let total_rows = base.pow(*total_width as u32);

    if total_rows > 1024 {
        return Err(Box::new(N2VError {
            msg: String::from("Too many rows in truth table to display (max 32)."),
            kind: ErrorKind::Other,
        }));
    }

    for i in 0..total_rows {
        let binary_string = format!("{:0total_width$b}", i);
        let mut bools: Vec<bool> = binary_string
            .chars()
            .map(|c| match c {
                '0' => false,
                '1' => true,
                _ => {
                    panic!("expected 0 or 1");
                }
            })
            .collect();

        let mut m: HashMap<String, Vec<bool>> = HashMap::new();
        let mut remaining_width = *total_width;
        for p in &hdl.ports {
            if p.direction == PortDirection::Out {
                continue;
            }
            if let GenericWidth::Terminal(Terminal::Num(w)) = p.width {
                let port_bools = &bools[(bools.len() - w)..];
                m.insert(p.name.value.clone(), port_bools.to_vec());
                remaining_width -= w;
            } else {
                panic!("Generic widths not supported.");
            }
            bools.truncate(remaining_width);
        }

        let inputs = match BusMap::try_from(m) {
            Ok(x) => x,
            Err(s) => {
                return Err(Box::new(N2VError {
                    msg: s,
                    kind: ErrorKind::Other,
                }));
            }
        };
        let outputs = simulator.simulate(&inputs)?;

        let mut row: Vec<Vec<Option<bool>>> = vec![];
        for s in &column_names {
            row.push(outputs.get_name(s));
        }
        column_values.push(row);
    }

    Ok((column_names, column_values))
}

#[wasm_bindgen]
pub fn component_graphs(s: &str) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    let mut scanner = Scanner::new(s, PathBuf::from(""));
    let mut parser = Parser {
        scanner: &mut scanner,
    };

    let hdl = match parser.parse() {
        Ok(x) => x,
        Err(e) => {
            return Err(JsValue::from(&e.to_string()));
        }
    };

    let provider: Rc<dyn HdlProvider> = Rc::new(EmbedReader);
    let chip = match Chip::new(&hdl, ptr::null_mut(), &provider, true, &Vec::new()) {
        Ok(x) => x,
        Err(e) => {
            return Err(JsValue::from(&e.to_string()));
        }
    };

    Ok(serde_json::to_string(&chip.circuit).unwrap())
}

#[cfg(test)]
mod libtest {
    use super::*;
    use crate::parser::FileReader;

    use std::env;
    use std::path::Path;

    #[test]
    fn test_nand2tetris_solution_and() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let base_path = String::from(
            manifest_dir
                .join("resources")
                .join("tests")
                .join("nand2tetris")
                .join("solutions")
                .to_str()
                .unwrap(),
        );
        let provider = Rc::new(FileReader::new(&base_path));
        let contents = provider.get_hdl("And.hdl").unwrap();
        let (_, table) =
            full_table_internal(&contents, Rc::new(FileReader::new(&base_path))).unwrap();
        assert_eq!(table.len(), 4);
    }
}
