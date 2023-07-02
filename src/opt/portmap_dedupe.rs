//! # Port Map Deduplication Optimization Pass
//!
//! `portmap_dedupe` is an optimization pass that simplifies the port mappings
//! of component instances.  It eliminates duplicate entries in the component
//! instances' port mappings by introducing intermediate signals.  The VHDL
//! synthesizer depends on this pass.

use crate::opt::optimization::OptimizationPass;
use crate::parser::{
    get_hdl, AssignmentHDL, BusHDL, ChipHDL, Component, HdlProvider, Identifier, Loop, Part,
    PortDirection, PortMappingHDL,
};
use std::error::Error;

pub struct PortMapDedupe {
    component_counter: u32,
}

use std::collections::HashMap;
use std::rc::Rc;

impl OptimizationPass for PortMapDedupe {
    fn apply(
        &mut self,
        chip: &ChipHDL,
        provider: &Rc<dyn HdlProvider>,
    ) -> Result<ChipHDL, Box<dyn Error>> {
        let mut new_chip = ChipHDL {
            parts: vec![],
            ..chip.clone()
        };
        let mut new_assignments: Vec<AssignmentHDL> = vec![];

        for part in &chip.parts {
            match part {
                Part::Component(comp) => {
                    let (new_comp, new_signals) =
                        self.process_component(comp, &mut new_assignments, provider)?;
                    new_chip.parts.push(Part::Component(new_comp));
                }
                Part::Loop(loop_part) => {
                    let mut new_loop_body = vec![];
                    for comp in &loop_part.body {
                        let (new_comp, new_signals) =
                            self.process_component(comp, &mut new_assignments, provider)?;
                        new_loop_body.push(new_comp);
                    }
                    new_chip.parts.push(Part::Loop(Loop {
                        body: new_loop_body,
                        ..loop_part.clone()
                    }));
                }
                Part::AssignmentHDL(_) => {}
            }
        }

        for assignment in new_assignments {
            new_chip.parts.push(Part::AssignmentHDL(assignment));
        }

        Ok(new_chip)
    }
}

impl PortMapDedupe {
    pub fn new() -> PortMapDedupe {
        PortMapDedupe {
            component_counter: 0,
        }
    }

    // This is the core of the optimization pass.  It takes a component
    // instance and deduplicates the port mappings by introducing an
    // intermediate signal for any output port that is used more than
    // once.
    fn process_component(
        &mut self,
        comp: &Component,
        new_assignments: &mut Vec<AssignmentHDL>,
        provider: &Rc<dyn HdlProvider>,
    ) -> Result<(Component, Vec<AssignmentHDL>), Box<dyn Error>> {
        self.component_counter += 1;

        // The new component that we are constructing.
        let mut new_comp = Component {
            mappings: Vec::new(),
            ..comp.clone()
        };
        let comp_chip = get_hdl(&comp.name.value, provider)?;

        // Count the number of times each port is used.
        let mut counts = HashMap::new();
        for mapping in &comp.mappings {
            let port_name = &mapping.port.name;
            let count = counts.entry(port_name).or_insert(0);
            *count += 1;
        }

        // Only keep duplicates. The fact that non-duplicates are not
        // included in the counts map is used later to determine which
        // ports need to be mapped to intermediate signals.
        counts.retain(|_, &mut v| v > 1);

        for mapping in &comp.mappings {
            let port_name = &mapping.port.name;
            let port = comp_chip.get_port(port_name)?;

            if port.direction == PortDirection::In {
                new_comp.mappings.push(mapping.clone());
                continue;
            }

            if let Some(count) = counts.get_mut(port_name) {
                let new_signal_name = format!("{}_{}", port_name, self.component_counter);
                if *count > 1 {
                    *count -= 1;
                    new_assignments.push(AssignmentHDL {
                        left: mapping.wire.clone(),
                        right: BusHDL {
                            name: new_signal_name.clone(),
                            start: None,
                            end: None,
                        },
                    });
                } else {
                    *count -= 1;

                    new_assignments.push(AssignmentHDL {
                        left: mapping.wire.clone(),
                        right: BusHDL {
                            name: new_signal_name.clone(),
                            start: None,
                            end: None,
                        },
                    });

                    new_comp.mappings.push(PortMappingHDL {
                        wire_ident: Identifier {
                            value: new_signal_name.clone(),
                            path: mapping.wire_ident.path.clone(),
                            line: mapping.wire_ident.line,
                        },
                        wire: BusHDL {
                            name: new_signal_name,
                            start: None,
                            end: None,
                        },
                        port: mapping.port.clone(),
                    })
                }
            } else {
                new_comp.mappings.push(mapping.clone());
            }
        }

        Ok((new_comp, new_assignments.clone()))
    }
}
