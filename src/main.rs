#[macro_use]
extern crate more_asserts;

mod busmap;
mod error;
mod expr;
mod parser;
mod scanner;
pub mod simulator; // hack to deal with dead code warning
mod test_parser;
mod test_scanner;
mod test_script;
mod vhdl;
mod rom;


use crate::parser::*;
use crate::simulator::{Bus, Chip, Simulator};
use crate::test_script::run_test;
use crate::error::{N2VError, ErrorKind};
use clap::Parser as ArgParser;
use clap::Subcommand;
use object::{Object, ObjectSection};
use parser::Parser;
use scanner::Scanner;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
use std::rc::Rc;
use std::error::Error;

#[derive(ArgParser)]
#[clap(version)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Creates VHDL and Quartus TCL.
    SynthVHDL {
        /// lists test values
        #[clap(short, long, action)]
        output_dir: PathBuf,
        top_level_file: String,
    },

    /// Parses chip and simulates a single input, for catching errors.
    Check {
        #[clap(short, long, action)]
        top_level_file: String,
    },

    /// Runs a nand2tetris test
    Test {
        #[clap(short, long, action)]
        test_file: String,
    },

    /// Synthesizes CS 314 ROM from .text section of ELF binary
    /// Does not yet support .data or .bss sections
    Rom {
        thumb_binary: String,
    },

    /// Decodes a thumb binary and prints the .text section as machine cod
    Decode {
        thumb_binary: String,
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::SynthVHDL {
            output_dir,
            top_level_file,
        } => {
            let source_code= fs::read_to_string(&top_level_file)?;
            let mut scanner = Scanner::new(&source_code, PathBuf::from(&top_level_file));
            let mut parser = Parser {
                scanner: &mut scanner,
            };
            let hdl = parser.parse().expect("Parse error");
            let base_path = String::from(
                hdl.path
                    .as_ref()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_str()
                    .unwrap(),
            );
            let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(&base_path));
            let entities = crate::vhdl::synth_vhdl(&hdl, &provider).unwrap();
            let quartus_dir = Path::new(&output_dir);
            crate::vhdl::create_quartus_project(&hdl, entities, quartus_dir)
                .expect("Unable to create project");
        }
        Commands::Check { top_level_file } => {
            let source_code = fs::read_to_string(&top_level_file)?;
            let mut scanner = Scanner::new(&source_code, PathBuf::from(&top_level_file));
            let mut parser = Parser {
                scanner: &mut scanner,
            };

            let hdl = parser.parse()?;

            let base_path = String::from(
                hdl.path
                    .as_ref()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_str()
                    .unwrap(),
            );
            let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(&base_path));
            let chip = Chip::new(&hdl, ptr::null_mut(), &provider, false, &Vec::new())?; 
            let mut simulator = Simulator::new(chip);

            // Get all input ports.
            // Set all input ports to false and simulate.
            let mut inputs = simulator
                .chip
                .get_port_values_for_direction(PortDirection::In);

            // TODO: make it easier to get full bus out of busmap
            for sn in inputs.signals() {
                let sig_width = inputs.get_width(&sn);
                let usig_width = sig_width.as_ref().unwrap_or(&0);
                let b = Bus {
                    name: sn,
                    range: sig_width.map(|x| 0..x),
                };
                inputs.insert_option(&b, vec![Some(false); *usig_width]);
            }

            // We don't care what the outputs are, just want to simulate
            // and trigger any dynamic errors.
            simulator.simulate(&inputs)?; 

            println!("✔️️️    Check Passed"); 
            println!("---------------------");
            println!("Name: {}", &simulator.chip.name);
            println!("Ports:");
            for (port_name, port) in &simulator.chip.ports {
                println!("\t{}: Direction={:?} Width={}", &port_name, port.direction, port.width);
            }
            println!("Signals:");
            for signal_name in &simulator.chip.signals.signals() {
                let sig_width = match &simulator.chip.signals.get_width(signal_name) {
                    Some(w) => w.to_string(),
                    None => String::from("?"),
                };
                println!("\t{}: Width={}", &signal_name, &sig_width);
            }
        }
        Commands::Test { test_file } => {
            run_test(test_file)?;
        }
        Commands::Rom { thumb_binary } => {
            println!("synth a ROM!")
        }
        Commands::Decode { thumb_binary } => {
            let bin_data = fs::read(thumb_binary)?;
            let obj_file = object::File::parse(&*bin_data)?;

            if let Some(section) = obj_file.section_by_name(".text") {
                println!("{:#x?}", section.data()?);
            } else {
                return Err(Box::new(N2VError {
                    msg: String::from("Text section is not available."),
                    kind: ErrorKind::Other,
                }));
            }

            println!("decode a binary!")
        }
    }
    Ok(())
}
