//! # Sequential Chip Identification pass.
//!
//! This pass identifies which components are sequential.

use std::error::Error;
use std::{collections::HashMap, rc::Rc};

use crate::parser::{get_hdl, Component, Part};
use crate::{
    opt::optimization::OptimizationPass,
    parser::{ChipHDL, HdlProvider},
};

use super::optimization::OptimizationInfo;

pub type SequentialFlagMap = HashMap<String, bool>;

pub struct SequentialPass {
    sequential_flag_map: SequentialFlagMap,
}

impl OptimizationPass for SequentialPass {
    fn apply(
        &mut self,
        chip: &ChipHDL,
        provider: &Rc<dyn HdlProvider>,
    ) -> Result<(ChipHDL, OptimizationInfo), Box<(dyn Error)>> {
        // Traverse the chip to identify sequential components.
        let mut chip_sequential = false;
        for part in &chip.parts {
            chip_sequential |= self.traverse(part, provider)?;
        }

        // If any part of the chip is sequential, the chip is sequential.
        self.sequential_flag_map.insert(chip.name.clone(), chip_sequential);

        Ok((chip.clone(), OptimizationInfo::SequentialFlagMap(self.sequential_flag_map.clone())))
    }
}


impl SequentialPass {
    pub fn new() -> Self {
        Self {
            sequential_flag_map: HashMap::new(),
        }
    }

    fn traverse(
        &mut self,
        part: &Part,
        provider: &Rc<dyn HdlProvider>,
    ) -> Result<bool, Box<dyn Error>> {
        match part {
            Part::Loop(loop_part) => {
                let mut loop_sequential = false;
                for component_in_loop in &loop_part.body {
                    loop_sequential |= self.process_component(component_in_loop, provider)?;
                }
                Ok(loop_sequential)
            }
            Part::Component(component) => {
                let is_sequential = self.process_component(component, provider)?;
                Ok(is_sequential)
            }
            Part::AssignmentHDL(_) => Ok(false),
        }
    }

    fn process_component(
        &mut self,
        component: &Component,
        provider: &Rc<dyn HdlProvider>,
    ) -> Result<bool, Box<dyn Error>> {
        // Get the ChipHDL of the component
        let component_chip = get_hdl(&component.name.value, provider)?;

        // Traverse in post-order, so process dependencies first.
        let mut component_sequential = false;
        for dependency in self.get_all_dependencies(&component_chip)? {
            component_sequential |= self.traverse(&dependency, provider)?;
        }

        // If the component itself is sequential, or any of its children are, then it's sequential.
        component_sequential |= self.is_sequential(&component_chip);

        // Update the sequential_flag_map
        self.sequential_flag_map
            .insert(component.name.value.clone(), component_sequential);

        Ok(component_sequential)
    }

    /// Function to determine if a chip is sequential.
    /// Currently, it only checks if the chip is a DFF.
    fn is_sequential(&self, chip: &ChipHDL) -> bool {
        chip.name == "DFF"
    }

    fn get_all_dependencies(&self, chip: &ChipHDL) -> Result<Vec<Part>, Box<dyn Error>> {
        let mut dependencies = Vec::new();
        for part in &chip.parts {
            if let Part::Component(component) = part {
                dependencies.push(Part::Component(component.clone()));
            }
        }
        Ok(dependencies)
    }
}
