//! # Sequential Chip Identification pass.
//!
//! This pass identifies which components are sequential.

use std::{collections::HashMap, rc::Rc};
use std::error::Error;

use crate::parser::{Part, Component, get_hdl};
use crate::{opt::optimization::OptimizationPass, parser::{ChipHDL, HdlProvider}};

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
        Ok((chip.clone(), OptimizationInfo::SequentialFlagMap(self.sequential_flag_map.clone())))
    }
}

impl SequentialPass {
    pub fn new() -> Self {
        Self {
            sequential_flag_map: HashMap::new(),
        }
    }

    /// Recursive function to traverse the dependencies of a part.
    ///
    /// This function will check if the part is a sequential chip. If it is, it will
    /// mark the corresponding entry in the sequential_flag_map as true. Then, it will
    /// call itself on each of the part's dependencies.
    fn traverse(&mut self, part: &Part, provider: &Rc<dyn HdlProvider>) -> Result<(), Box<dyn Error>> {
        match part {
            Part::Loop(loop_part) => {
                // Loops have a body that needs to be traversed
                // Traverse each component in the loop body.
                for component_in_loop in &loop_part.body {
                    self.process_component(component_in_loop, provider)?;
                }
            }
            Part::Component(component) => {
                // Components can have dependencies that need to be traversed.
                self.process_component(component, provider)?;
            }
            Part::AssignmentHDL(_) => {
                // Assignments don't have dependencies, so we don't need to traverse them.
            }
        }
        Ok(())
    }

    /// Process a single component: retrieve its ChipHDL, process its dependencies, 
    /// and determine if it is sequential.
    fn process_component(&mut self, component: &Component, provider: &Rc<dyn HdlProvider>) -> Result<(), Box<dyn Error>> {
        // Get the ChipHDL of the component
        let component_chip = get_hdl(&component.name.value, provider)?;

        // Traverse in post-order, so process dependencies first.
        for dependency in self.get_all_dependencies(&component_chip)? {
            self.traverse(&dependency, provider)?;
        }

        // If any of the dependencies are sequential, or the part is a DFF, then the part is sequential.
        let is_sequential = self.is_sequential(&component_chip);

        // Update the sequential_flag_map
        self.sequential_flag_map.insert(component.name.value.clone(), is_sequential);

        Ok(())
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

    /// Function to determine if a chip is sequential. 
    /// Currently, it only checks if the chip is a DFF.
    fn is_sequential(&self, chip: &ChipHDL) -> bool {
        chip.name == "DFF"
    }
}
