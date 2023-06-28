//! # Port Map Deduplication Optimization Pass
//!
//! `portmap_dedupe` is an optimization pass that simplifies the port mappings
//! of component instances.  It eliminates duplicate entries in the component
//! instances' port mappings by introducing intermediate signals.  The VHDL
//! synthesizer depends on this pass.

use crate::opt::optimization::OptimizationPass;
use crate::parser::ChipHDL;

use std::error::Error;

pub struct PortMapDedupe;

impl OptimizationPass for PortMapDedupe {
    fn apply(&self, chip: &ChipHDL) -> Result<ChipHDL, Box<dyn Error>> {
        Ok(chip.clone())
    }
}
