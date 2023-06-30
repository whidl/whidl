use std::{error::Error, rc::Rc};

use crate::parser::{ChipHDL, HdlProvider};

/// A trait representing an optimization pass on an HDL Chip.
pub trait OptimizationPass {
    /// Apply the optimization pass to the given HDL chip, returning a new, optimized chip.
    fn apply(&mut self, chip: &ChipHDL, provider: &Rc<dyn HdlProvider>) -> Result<ChipHDL, Box<dyn Error>>;
}
