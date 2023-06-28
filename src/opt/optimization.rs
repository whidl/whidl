use std::error::Error;

use crate::parser::ChipHDL;

/// A trait representing an optimization pass on an HDL Chip.
pub trait OptimizationPass {
    /// Apply the optimization pass to the given HDL chip, returning a new, optimized chip.
    fn apply(&self, chip: &ChipHDL) -> Result<ChipHDL, Box<dyn Error>>;
}
