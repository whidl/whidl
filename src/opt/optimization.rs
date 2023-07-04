use crate::parser::{ChipHDL, HdlProvider};
use std::{collections::HashMap, error::Error, rc::Rc};

pub type SequentialFlagMap = HashMap<String, bool>;

/// Extra info that can be returned by an optimization pass.
#[derive(Clone)]
pub enum OptimizationInfo {
    None,
    SequentialFlagMap(SequentialFlagMap),
}

/// All HDL passes must implement this trait.
pub trait OptimizationPass {
    fn apply(
        &mut self, 
        chip: &ChipHDL, 
        provider: &Rc<dyn HdlProvider>
    ) -> Result<(ChipHDL, OptimizationInfo), Box<dyn Error>>;
}
