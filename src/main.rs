//! This is the main command-line utility.

mod busmap;
mod error;
mod expr;
mod modelsim;
mod opt;
mod parser;
mod scanner;
mod simulator;
mod test_parser;
mod test_scanner;
mod test_script;
mod vhdl;

use error::*;
use modelsim::synth_vhdl_test;
use parser::*;
use simulator::{Bus, Chip, Simulator};
use test_script::run_test;
use vhdl::VhdlEntity;

use clap::Parser as ArgParser;
use clap::Subcommand;
use parser::Parser;
use scanner::Scanner;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
use std::rc::Rc;

use crate::vhdl::write_quartus_project;
use crate::opt::portmap_dedupe::PortMapDedupe;
use crate::opt::optimization::OptimizationPass;

#[derive(ArgParser)]
#[clap(version)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Creates VHDL and Quartus TCL.
    /// This command can be used to convert
    /// an HDL file to VHDL, or a nand2tetris test into a Modelsim testbench.
    /// The output is a TCL script for creating a Quartus Prime project
    /// using quartus_sh, or a testbench to run with Modelsim.
    SynthVHDL {
        /// Path to either a top-level HDL file or a .tst test script to
        /// convert from nand2tetris from to VHDL.
        #[clap(index = 1)]
        path: PathBuf,

        /// The synth-vhdl command creates a Quartus Prime project in
        /// a new folder. This is the folder to create for the project.
        #[clap(index = 2)]
        output_dir: PathBuf,
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
}

// TODO: Remove duplication from this function.
fn synth_vhdl_chip(output_dir: &PathBuf, hdl_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    // Standard HDL parsing pipeline.
    let source_code = fs::read_to_string(hdl_path)?;
    let mut scanner = Scanner::new(&source_code, hdl_path.clone());
    let base_path = hdl_path.parent().unwrap();
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
    let chip_vhdl: VhdlEntity = VhdlEntity::try_from(chip)?;

    // Create a Quartus Prime project.
    let quartus_dir = Path::new(&output_dir);
    let project = crate::vhdl::QuartusProject::new(hdl, chip_vhdl, quartus_dir.to_path_buf());
    write_quartus_project(&project)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::SynthVHDL { output_dir, path } => {
            // Try synthesizing a Chip. If that fails, try synthesizing a test.
            match fs::create_dir(output_dir) {
                Ok(_) => (),
                Err(e) => {
                    return Err(Box::new(TransformedError {
                        msg: String::from("Unable to create output directory."),
                        kind: ErrorKind::IOError,
                        source: Some(Box::new(e)),
                    }));
                }
            }

            let vhdl_result = synth_vhdl_chip(output_dir, path);
            if vhdl_result.is_err() {
                let synth_result = synth_vhdl_test(output_dir, path);

                if synth_result.is_err() {
                    println!("Parsing as chip:\n{}", vhdl_result.unwrap_err());
                    println!("Parsing as test script:\n{}", synth_result.unwrap_err());
                    return Err(Box::new(N2VError {
                        msg: format!(
                            "Unable to parse {} as either an HDL file or a test script.",
                            path.display(),
                        ),
                        kind: ErrorKind::Other,
                    }));
                }
            }
        }
        Commands::Check { top_level_file } => {
            let source_code = fs::read_to_string(top_level_file)?;
            let mut scanner = Scanner::new(&source_code, PathBuf::from(&top_level_file));
            let base_path = scanner.path.parent().unwrap();
            let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(base_path));
            let mut parser = Parser::new(&mut scanner, provider.clone());

            let hdl = parser.parse()?;

            let chip = Chip::new(&hdl, ptr::null_mut(), &provider, false, &Vec::new())?;
            let mut simulator = Simulator::new(chip);

            // Get all input ports.
            // Set all input ports to false and simulate.
            let mut inputs = simulator
                .chip
                .get_port_values_for_direction(PortDirection::In);

            // TODO: make it easier to get full bus out of busmap
            for sn in inputs.keys() {
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
                println!(
                    "\t{}: Direction={:?} Width={}",
                    &port_name, port.direction, port.width
                );
            }
            println!("Signals:");
            for signal_name in &simulator.chip.signals.keys() {
                let sig_width = match &simulator.chip.signals.get_width(signal_name) {
                    Some(w) => w.to_string(),
                    None => String::from("?"),
                };
                println!("\t{}: Width={}", &signal_name, &sig_width);
            }
        }
        Commands::Test { test_file } => {
            run_test(&PathBuf::from(test_file))?;
        }
    }
    Ok(())
}
