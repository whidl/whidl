use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::ops::Range;
use std::rc::Rc;

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Graph;
use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;

use crate::busmap::BusMap;
use crate::error::{ErrorKind, N2VError};
use crate::expr::*;
use crate::parser::*;

/// The main graph connecting components of a chip together.
/// Each chip is a component such as And, Or, Not, Nand.
type Circuit = Graph<Chip, Wire>;

/// Used to facilitate lazy elaboration of components.
/// Outputs of chips are cached for given inputs.
type Cache = HashMap<InputCacheEntry, BusMap>;

trait TryMap {
    fn try_map();
}

#[derive(Serialize, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Bus {
    pub name: String,
    pub range: Option<Range<usize>>,
}

#[derive(Hash, Eq, PartialEq)]
/// Stores the results of a chip for a given set a of inputs.
/// Used to avoid recalculating the same results over and over again.
pub struct InputCacheEntry {
    name: String,
    signals: BusMap,
}

pub struct Simulator {
    pub input_cache: Cache,
    pub dirty_dffs: Vec<*mut Chip>,
    pub chip: Chip,
}

impl Simulator {
    pub fn new(chip: Chip) -> Simulator {
        Simulator {
            input_cache: HashMap::new(),
            dirty_dffs: Vec::new(),
            chip,
        }
    }

    pub fn simulate(&mut self, inputs: &BusMap) -> Result<BusMap, Box<dyn Error>> {
        let ports = self.chip.ports.clone();
        for (port_name, port) in ports {
            if port.direction == PortDirection::Out {
                continue;
            }

            let bus_idx = Bus {
                name: port_name.clone(),
                range: Some(0..port.width),
            };
            let port_input = inputs.get_bus(&bus_idx);
            self.chip.signals.insert_option(&bus_idx, port_input)
        }

        self.chip.dirty = true;
        self.chip
            .compute(&mut self.input_cache, &mut self.dirty_dffs)?;

        Ok(self.chip.get_port_values())
    }

    // Tick advances the clock without changing the inputs to the chip.
    pub fn tick(&mut self) -> Result<(), Box<dyn Error>> {
        let dffs_this_tick = self.dirty_dffs.clone();
        self.dirty_dffs.clear();
        let mut parents = Vec::new();
        for dff_ref in dffs_this_tick {
            let mut dff = unsafe { dff_ref.as_mut().unwrap() };

            dff.signals.insert_option(
                &Bus {
                    name: String::from("out"),
                    range: Some(0..1),
                },
                dff.signals.get_bus(&Bus {
                    name: String::from("in"),
                    range: Some(0..1),
                }),
            );
            dff.dirty = true;

            // chase parents up to the top level chip
            // mark everything along the way as dirty, no cache.
            let mut parent = dff.parent;
            while !parent.is_null() {
                let parent_chip;
                unsafe {
                    parent_chip = &mut *parent;
                }
                parent_chip.cache = false;
                parent_chip.dirty = true;
                parent = parent_chip.parent;
                parents.push(parent_chip);
            }
        }

        for parent_chip in parents {
            parent_chip.compute(&mut self.input_cache, &mut self.dirty_dffs)?;
        }

        Ok(())
    }
}

impl Serialize for Chip {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("Chip", 1)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("ports", &self.ports)?;
        s.end()
    }
}

// Same as HDL port, but with generic widths resolved.
#[derive(Clone, Serialize)]
pub struct Port {
    pub name: Identifier,
    pub width: usize,
    pub direction: PortDirection,
}

pub struct Assignment {
    pub left: Bus,
    pub right: Bus,
    pub width: usize,
}

// A chip constructed from parsed HDL.
pub struct Chip {
    pub name: String,
    hdl: Option<ChipHDL>, // This should probably be a reference. We don't need to have a zillion copies of the HDL.
    pub circuit: Circuit,
    pub ports: HashMap<String, Port>,
    input_port_nodes: Vec<NodeIndex>,
    output_port_nodes: Vec<NodeIndex>,
    pub signals: BusMap,
    elaborated: bool,
    parent: *mut Chip,
    pub components: Vec<Component>, // Constructed from HDL parts which may contain for-generate loops.

    dirty: bool,
    cache: bool,

    // A chip must have an HDL provider because it is responsible
    // for lazily elaborating itself. This is reference counted because
    // elaboration makes new chips with HdlProviders and we don't know
    // size at compilation time.
    hdl_provider: Rc<dyn HdlProvider>,

    // Values of variables (generics and iterators)
    variables: HashMap<String, usize>,
    assignments: Vec<Assignment>,
}

impl fmt::Debug for Chip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(&self.name).finish()
    }
}

// TODO: All Wires in the graph must have buses with ranges. But the range
// in the Bus type is optional because it is also used by the parser.
// These should be two separate types?
#[derive(Serialize, Clone, Debug)]
pub struct Wire {
    pub source: Bus,
    pub target: Bus,
}

impl Chip {
    /// Constructs a Chip from the parse tree.
    pub fn new(
        hdl: &ChipHDL,
        parent: *mut Chip,
        hdl_provider: &Rc<dyn HdlProvider>,
        elaborate: bool,
        generics: &Vec<usize>, // generic args when this chip is being created.
    ) -> Result<Chip, Box<dyn Error>> {
        let circuit = Circuit::new();

        if hdl.name.to_uppercase() == "NAND" {
            return Ok(make_nand_chip(parent, hdl_provider));
        } else if hdl.name.to_uppercase() == "DFF" {
            return Ok(make_dff_chip(parent, hdl_provider));
        }

        // Assign values to generic variables.
        if generics.len() != hdl.generic_decls.len() {
            return Err(Box::new(N2VError {
                msg: format!(
                    "Chip {} declares {} generics but instantiated with {}",
                    hdl.name,
                    hdl.generic_decls.len(),
                    generics.len()
                ),
                kind: ErrorKind::SimulationError(hdl.path.clone()),
            }));
        }
        let mut variables = HashMap::new();

        #[allow(clippy::needless_range_loop)]
        for gv in 0..hdl.generic_decls.len() {
            variables.insert(hdl.generic_decls[gv].value.clone(), generics[gv]);
        }

        // Signals for this component.
        // Circuit graph holds the actual signal data.
        let mut signals = BusMap::new();

        // Create port signals
        for port in &hdl.ports {
            let width = eval_expr_numeric(&port.width, &variables)?;

            if let Err(e) = signals.create_bus(&port.name.value, width) {
                return Err(Box::new(N2VError {
                    msg: { format!("Cannot create port {}: {}", port.name.value, e) },
                    kind: ErrorKind::ParseIdentError(hdl_provider.clone(), port.name.clone()),
                }));
            }
        }

        // Create component definitions (expand for-generate loops).
        let components = Self::generate_components(hdl, generics)?;
        let assignments = gather_assignments(&hdl.parts);

        let general_generics: Vec<GenericWidth> = generics
            .iter()
            .map(|x| GenericWidth::Terminal(Terminal::Num(*x)))
            .collect();
        let inferred_widths = infer_widths(
            hdl,
            &assignments,
            &components,
            hdl_provider,
            &general_generics,
        )?;

        let generated_assignments =
            Self::generate_assignments(&inferred_widths, assignments, &variables)?;

        // Create disconnected internal signals.
        // These are connected below.
        for (n, iw) in inferred_widths {
            // To create a full chip we need only numeric expressions.
            if let GenericWidth::Terminal(Terminal::Num(x)) = iw {
                if let Err(e) = signals.create_bus(&n, x) {
                    return Err(Box::new(N2VError {
                        msg: {
                            format!(
                                "{:?} Cannot create internal signal {} of width {}. {}",
                                hdl.path, n, iw, e
                            )
                        },
                        kind: ErrorKind::SimulationError(hdl.path.clone()),
                    }));
                }
            } else {
                return Err(Box::new(N2VError {
                    msg: {
                        format!(
                            "{:?} Cannot create internal signal {} of width {}.",
                            hdl.path, n, iw
                        )
                    },
                    kind: ErrorKind::SimulationError(hdl.path.clone()),
                }));
            }
        }

        let ports = hdl
            .ports
            .iter()
            .map(|x| {
                let pw = eval_expr_numeric(&x.width, &variables)?;
                Ok((
                    x.name.value.clone(),
                    Port {
                        direction: x.direction,
                        name: x.name.clone(),
                        width: pw,
                    },
                ))
            })
            .collect::<Result<HashMap<String, Port>, N2VError>>()?;

        let mut chip = Chip {
            name: hdl.name.clone(),
            ports,
            signals,
            hdl: Some(hdl.clone()),
            elaborated: false,
            circuit,
            dirty: false,
            input_port_nodes: Vec::new(),
            output_port_nodes: Vec::new(),
            cache: true,
            parent,
            hdl_provider: Rc::clone(hdl_provider),
            variables,
            components,
            assignments: generated_assignments,
        };

        if elaborate {
            chip.elaborate()?;
        }

        Ok(chip)
    }

    // This expands for-generate loops into components for the chip. This
    // cannot be done during parsing because the values of generic variables
    // may not be known until elaboration.
    fn generate_components(
        hdl: &ChipHDL,
        generics: &Vec<usize>,
    ) -> Result<Vec<Component>, N2VError> {
        let mut res = Vec::new();

        // Assign values to generic variables.
        if generics.len() != hdl.generic_decls.len() {
            return Err(N2VError {
                msg: format!(
                    "Chip {} declares {} generics but instantiated with {}",
                    hdl.name,
                    hdl.generic_decls.len(),
                    generics.len()
                ),
                kind: ErrorKind::SimulationError(hdl.path.clone()),
            });
        }
        let mut variables = HashMap::new();

        #[allow(clippy::needless_range_loop)]
        for gv in 0..hdl.generic_decls.len() {
            variables.insert(hdl.generic_decls[gv].value.clone(), generics[gv]);
        }

        for part in &hdl.parts {
            match part {
                Part::Component(c) => {
                    res.push(c.clone());
                }
                Part::Loop(l) => {
                    let start = eval_expr_numeric(&l.start, &variables)?;
                    let end = eval_expr_numeric(&l.end, &variables)?;

                    // Replace any instances of iterator with current iterator value.
                    for i in start..(end + 1) {
                        let replace = |w: &GenericWidth| -> GenericWidth {
                            replace_expr(
                                w,
                                &l.iterator.value,
                                &GenericWidth::Terminal(Terminal::Num(i)),
                            )
                        };

                        for c in &l.body {
                            let mut new_c: Component = c.clone();
                            for m in &mut new_c.mappings {
                                m.port.start = m.port.start.as_ref().map(replace);
                                m.port.end = m.port.end.as_ref().map(replace);
                                m.wire.start = m.wire.start.as_ref().map(replace);
                                m.wire.end = m.wire.end.as_ref().map(replace);
                            }

                            new_c.generic_params =
                                new_c.generic_params.iter().map(replace).collect();

                            res.push(new_c);
                        }
                    }
                }
                Part::AssignmentHDL(_a) => {} // ignore assignments for now
            }
        }

        Ok(res)
    }

    fn elaborate(&mut self) -> Result<(), Box<dyn Error>> {
        let self_ptr = self as *mut Chip;
        self.elaborated = true;
        if self.hdl.is_none() {
            return Ok(());
        }

        // Where each bit of the signal source comes from.
        let mut signal_sources: HashMap<String, Vec<Option<(NodeIndex, Bus)>>> = HashMap::new();

        // create input components
        for (port_name, port) in &self.ports {
            if port.direction == PortDirection::Out {
                continue;
            }
            let port_chip = make_port_chip(port_name, port.width, self_ptr, &self.hdl_provider);
            let port_node = self.circuit.add_node(port_chip);
            self.input_port_nodes.push(port_node);

            let mut source = Vec::new();
            for i in 0..port.width {
                let source_bus = Bus {
                    name: String::from("in"),
                    range: Some(i..i + 1),
                };
                source.push(Some((port_node, source_bus)));
            }

            signal_sources.insert(port.name.value.clone(), source);
        }

        let mut need_true_literal = false;
        let mut need_false_literal = false;

        // Insert signal sources for every assignment.
        for a in &self.assignments {
            if &a.right.name == "true" {
                need_true_literal = true;
            }
            if &a.right.name == "false" {
                need_false_literal = true;
            }
            let port_chip = make_port_chip(
                a.left.name.clone().as_str(),
                a.width,
                self_ptr,
                &self.hdl_provider,
            );
            let assignment_port_node = self.circuit.add_node(port_chip);

            let mut source = Vec::new();
            for i in 0..a.width {
                let source_bus = Bus {
                    name: String::from("in"),
                    range: Some(i..i + 1),
                };
                source.push(Some((assignment_port_node, source_bus)));
            }

            signal_sources.insert(a.left.name.clone(), source);
        }

        // Create components and handle out ports from components into signals
        // indices of created_components needs to match order of parts
        // Also checks if true/false literals are used.
        let mut created_components: Vec<NodeIndex> = Vec::new();
        for (_, part) in self.components.iter().enumerate() {
            let part_hdl = get_hdl(&part.name.value, &self.hdl_provider)?;

            // Convert generics with vars to concrete generics for component.
            // e.g. Mux<W> needs to become Mux<4> if W=4. At this point
            // we need actual bus widths.
            let mut resolved_generics: Vec<usize> = Vec::new();
            for g in &part.generic_params {
                if let GenericWidth::Terminal(Terminal::Num(w)) = g {
                    resolved_generics.push(*w);
                } else if let GenericWidth::Terminal(Terminal::Var(w)) = g {
                    let rw = self.variables.get(&w.value).unwrap();
                    resolved_generics.push(*rw);
                }
            }

            let part_chip = Chip::new(
                &part_hdl,
                self_ptr,
                &Rc::clone(&self.hdl_provider),
                false, // Only elaborate one level deep.
                &resolved_generics,
            )?;
            let part_variables = part_chip.variables.clone();

            let mut used_port_buses: BusMap = BusMap::new();
            for (port_name, port) in &part_chip.ports {
                used_port_buses.create_bus(port_name, port.width)?;
            }

            let part_node = self.circuit.add_node(part_chip);
            created_components.push(part_node);

            for m in &part.mappings {
                let signal_name = &m.wire.name;
                let port = match part_hdl.get_port(&m.port.name) {
                    Ok(x) => x,
                    Err(_) => {
                        let err_more_info = N2VError {
                            kind: ErrorKind::ParseIdentError(
                                self.hdl_provider.clone(),
                                m.wire_ident.clone(),
                            ),
                            msg: format!("Attempt to get non-existent port {}.", &m.port.name),
                        };

                        return Err(Box::new(err_more_info));
                    }
                };
                if signal_name == "true" {
                    need_true_literal = true;
                }
                if signal_name == "false" {
                    need_false_literal = true;
                }

                let port_width = eval_expr_numeric(&port.width, &part_variables)?;
                let port_start = match &m.port.start {
                    None => 0,
                    Some(x) => eval_expr_numeric(x, &self.variables)?,
                };
                let port_end = match &m.port.end {
                    None => port_width - 1,
                    Some(x) => eval_expr_numeric(x, &self.variables)?,
                };
                // Convert inclusive range in HDL to exclusive Range in Rust
                let port_range: Range<usize> = Range {
                    start: port_start,
                    end: port_end + 1,
                };

                // Insert port range for the pupose of verifying that we have
                // inputs for all of the input pins. Skip the rest of the loop.
                if port.direction == PortDirection::In {
                    let used_bus = Bus {
                        name: m.port.name.clone(),
                        range: Some(port_range.clone()),
                    };
                    used_port_buses.insert_option(&used_bus, vec![Some(true); port_range.len()]);

                    continue;
                }

                let wire_start = match &m.wire.start {
                    None => 0,
                    Some(x) => eval_expr_numeric(x, &self.variables)?,
                };
                let wire_end = match &m.wire.end {
                    None => port_width - 1,
                    Some(x) => eval_expr_numeric(x, &self.variables)?,
                };
                // Convert inclusive range in HDL to exclusive Range in Rust
                let wire_range: Range<usize> = Range {
                    start: wire_start,
                    end: wire_end + 1,
                };

                if signal_sources.get(signal_name).is_none() {
                    signal_sources.insert(
                        signal_name.clone(),
                        vec![None; self.signals.get_width(signal_name).unwrap()],
                    );
                }

                let mut i = port_range.start;
                let mut j = wire_range.start;
                while i < port_range.end {
                    // Check here to see if a bit already has a source.
                    // If it does we have an error in the HDL.
                    if signal_sources.get(signal_name).unwrap()[j].is_some() {
                        return Err(Box::new(N2VError {
                            kind: ErrorKind::ParseIdentError(
                                self.hdl_provider.clone(),
                                m.wire_ident.clone(),
                            ),
                            msg: format!("Duplicate source for signal name {}.", signal_name),
                        }));
                    }

                    signal_sources.get_mut(signal_name).unwrap()[j] = Some((
                        part_node,
                        Bus {
                            name: port.name.value.clone(),
                            range: Some(i..i + 1),
                        },
                    ));
                    i += 1;
                    j += 1;
                }
            }

            // Make sure we have inputs for every port bit.
            // for each port in the used busmap
            // if any bit is None, then warn.
            for port in part_hdl.ports {
                if port.direction == PortDirection::Out {
                    continue;
                }

                let used_bits = used_port_buses.get_name(&port.name.value);
                for b in used_bits {
                    if b.is_none() {
                        return Err(Box::new(N2VError {
                            kind: ErrorKind::ParseIdentError(
                                self.hdl_provider.clone(),
                                part.name.clone(),
                            ),
                            msg: format!(
                                "Component does not provide inputs for all bits of {}.",
                                &port.name.value
                            ),
                        }));
                    }
                }
            }
        }

        // Create true/false literals only if a port mapping requires it.
        let false_chip;
        let false_node;
        if need_false_literal {
            false_chip = make_literal_chip(Some(false), self_ptr, &self.hdl_provider);
            false_node = self.circuit.add_node(false_chip);
            let false_vector: Vec<_> = (0..16)
                .map(|i| {
                    Some((
                        false_node,
                        Bus {
                            name: String::from("out"),
                            range: Some(i..i + 1),
                        },
                    ))
                })
                .collect();
            signal_sources.insert(String::from("false"), false_vector);
        }
        let true_chip;
        let true_node;
        if need_true_literal {
            true_chip = make_literal_chip(Some(true), self_ptr, &self.hdl_provider);
            true_node = self.circuit.add_node(true_chip);
            let true_vector: Vec<_> = (0..16)
                .map(|i| {
                    Some((
                        true_node,
                        Bus {
                            name: String::from("out"),
                            range: Some(i..i + 1),
                        },
                    ))
                })
                .collect();
            signal_sources.insert(String::from("true"), true_vector);
        }

        // Closure for retrieving signal source and returning an error if the source is not found.
        let get_signal_source = |signal_name: &str, idx: usize, relevant_ident: &Identifier| {
            if signal_sources.contains_key(signal_name) {
                let signal_source = signal_sources.get(signal_name).unwrap();

                if idx >= signal_source.len() {
                    return Err(N2VError {
                        msg: format!(
                            "Bit {} for signal name {} is out of range.",
                            idx, signal_name
                        ),
                        kind: ErrorKind::ParseIdentError(
                            self.hdl_provider.clone(),
                            relevant_ident.clone(),
                        ),
                    });
                }

                match &signal_source[idx] {
                    None => Err(N2VError {
                        msg: format!("Bit {} for signal name {} is undefined.", idx, signal_name),
                        kind: ErrorKind::ParseIdentError(
                            self.hdl_provider.clone(),
                            relevant_ident.clone(),
                        ),
                    }),
                    Some(x) => Ok(Some(x)),
                }
            } else {
                Err(N2VError {
                    msg: format!("No source for signal name {}.", signal_name),
                    kind: ErrorKind::ParseIdentError(
                        self.hdl_provider.clone(),
                        relevant_ident.clone(),
                    ),
                })
            }
        };

        // Add edges for assignments
        for a in &self.assignments {
            for j in 0..a.width {
                let (source_node, source_bus) = match get_signal_source(
                    a.right.name.as_str(),
                    j,
                    &Identifier::from(a.right.name.as_str()),
                )? {
                    Some(x) => x,
                    None => {
                        continue;
                    }
                };
                let (target_node, _target_bus) = match get_signal_source(
                    a.left.name.as_str(),
                    j,
                    &Identifier::from(a.left.name.as_str()),
                )? {
                    Some(x) => x,
                    None => {
                        continue;
                    }
                };
                let wire = Wire {
                    source: source_bus.clone(),
                    target: Bus {
                        name: String::from("in"),
                        range: Some(j..j + 1),
                    },
                };
                self.circuit.add_edge(*source_node, *target_node, wire);
            }
        }

        for (part_idx, part) in self.components.iter().enumerate() {
            let part_hdl = get_hdl(&part.name.value, &self.hdl_provider)?;

            // Handle in ports from signals to components
            for m in &part.mappings {
                let signal_name = &m.wire.name;
                let port_idx = part_hdl
                    .ports
                    .iter()
                    .position(|x| x.name.value == m.port.name)
                    .unwrap();
                let port = &part_hdl.ports[port_idx];
                if port.direction == PortDirection::Out {
                    continue;
                }

                let port_width = self.eval_port_width(port, &part_hdl, part)?;
                let port_start = match &m.port.start {
                    None => 0,
                    Some(x) => eval_expr_numeric(x, &self.variables)?,
                };
                let port_end = match &m.port.end {
                    None => port_width - 1,
                    Some(x) => eval_expr_numeric(x, &self.variables)?,
                };
                // Convert inclusive range in HDL to exclusive Range in Rust
                let port_range: Range<usize> = Range {
                    start: port_start,
                    end: port_end + 1,
                };

                let wire_start = match &m.wire.start {
                    None => 0,
                    Some(x) => eval_expr_numeric(x, &self.variables)?,
                };
                let wire_end = match &m.wire.end {
                    None => port_width - 1,
                    Some(x) => eval_expr_numeric(x, &self.variables)?,
                };
                // Convert inclusive range in HDL to exclusive Range in Rust
                let wire_range: Range<usize> = Range {
                    start: wire_start,
                    end: wire_end + 1,
                };

                let mut i = wire_range.start;
                let mut j = port_range.start;

                // Wire and port ranges must be equal.
                // Loop over the wire. For each wire bit look up the bit
                while j < port_range.end {
                    // source_node is the graph node for the chip feeding into signal
                    // source_bus is the port/range creating this particular bit.
                    // signal_idx is the index of bit in the signal.
                    let (source_node, source_bus) =
                        match get_signal_source(signal_name, i, &m.wire_ident)? {
                            Some(x) => x,
                            None => {
                                i += 1;
                                j += 1;
                                continue;
                            }
                        };

                    let wire = Wire {
                        source: source_bus.clone(),
                        target: Bus {
                            name: port.name.value.clone(),
                            range: Some(j..j + 1),
                        },
                    };

                    self.circuit
                        .add_edge(*source_node, created_components[part_idx], wire);

                    i += 1;
                    j += 1;
                }
            }
        }

        for (port_name, port) in &self.ports {
            if port.direction == PortDirection::In {
                continue;
            }
            let port_chip = make_port_chip(port_name, port.width, self_ptr, &self.hdl_provider);
            let port_node = self.circuit.add_node(port_chip);
            self.output_port_nodes.push(port_node);

            for j in 0..port.width {
                let (source_node, source_bus) = match get_signal_source(port_name, j, &port.name)? {
                    Some(x) => x,
                    None => {
                        continue;
                    }
                };
                let wire = Wire {
                    source: source_bus.clone(),
                    target: Bus {
                        name: String::from("in"),
                        range: Some(j..j + 1),
                    },
                };
                self.circuit.add_edge(*source_node, port_node, wire);
            }
        }

        optimize_circuit(&mut self.circuit);

        Ok(())
    }

    pub fn get_port_values_for_direction(&self, direction: PortDirection) -> BusMap {
        // Return output signals as a BusMap
        let mut values = BusMap::new();
        let ports = self.ports.clone();
        for (port_name, port) in ports {
            if port.direction != direction {
                continue;
            }

            let idx = Bus {
                name: port_name.clone(),
                range: Some(0..port.width),
            };
            values.create_bus(&idx.name, port.width).unwrap();
            values.insert_option(&idx, self.signals.get_bus(&idx));
        }
        values
    }

    fn get_port_values(&self) -> BusMap {
        // Return output signals as a BusMap
        let mut values = BusMap::new();
        let ports = self.ports.clone();
        for (port_name, port) in ports {
            let idx = Bus {
                name: port_name.clone(),
                range: Some(0..port.width),
            };
            values.create_bus(&port_name, port.width).unwrap();
            values.insert_option(&idx, self.signals.get_bus(&idx));
        }
        values
    }

    fn insert_cache_entry(&mut self, input_cache: &mut Cache) {
        let inputs = self.get_port_values_for_direction(PortDirection::In);
        let cache_entry = InputCacheEntry {
            name: self.name.clone(),
            signals: inputs,
        };
        input_cache.insert(
            cache_entry,
            self.get_port_values_for_direction(PortDirection::Out),
        );
    }

    fn mark_neighbors(&mut self, component_idx: NodeIndex, dirty_dffs: &mut Vec<*mut Chip>) {
        // Mark neighbors dirty if we have changed any of their inputs.
        let mut neighbors = self.circuit.neighbors(component_idx).detach();
        while let Some(wire_idx) = neighbors.next_edge(&self.circuit) {
            let wire = self.circuit.edge_weight(wire_idx).unwrap().clone();

            let endpoints = self.circuit.edge_endpoints(wire_idx).unwrap();
            let neighbor_idx = endpoints.1;

            let neighbor_current_vals = self
                .circuit
                .node_weight_mut(neighbor_idx)
                .unwrap()
                .signals
                .get_bus(&wire.target);

            let neighbor_new_vals = self
                .circuit
                .node_weight_mut(component_idx)
                .unwrap()
                .signals
                .get_bus(&wire.source);

            if neighbor_new_vals != neighbor_current_vals {
                let neighbor_component = self.circuit.node_weight_mut(endpoints.1).unwrap();
                neighbor_component.dirty = true;
                self.dirty = true;
                neighbor_component
                    .signals
                    .insert_option(&wire.target, neighbor_new_vals);
                if neighbor_component.name == "DFF" {
                    dirty_dffs.push(neighbor_component as *mut Chip);
                }
            }
        }
    }

    /// Turns a list of AssignmentHDL into Assignments for easier computation
    fn generate_assignments(
        inferred_widths: &HashMap<String, GenericWidth>,
        assignments: Vec<AssignmentHDL>,
        generic_state: &HashMap<String, usize>,
    ) -> Result<Vec<Assignment>, Box<dyn Error>> {
        let mut converted_assignments = Vec::<Assignment>::new();
        for a in assignments {
            // This could possibly return None--add dummy width to infer_widths
            let w = inferred_widths.get(&a.left.name).unwrap();
            let usize_w = eval_expr_numeric(w, &generic_state)?;
            let left_bus = Bus {
                name: a.left.name,
                range: Some(0..usize_w),
            };
            let right_bus = Bus {
                name: a.right.name,
                range: Some(0..usize_w),
            };

            // Create a Bus for left and right
            let new_assignment = Assignment {
                left: left_bus,
                right: right_bus,
                width: usize_w,
            };
            converted_assignments.push(new_assignment);
        }
        Ok(converted_assignments)
    }

    fn compute(
        &mut self,
        input_cache: &mut Cache,
        dirty_dffs: &mut Vec<*mut Chip>,
    ) -> Result<(), Box<dyn Error>> {
        while self.dirty {
            self.dirty = false;

            if self.name.to_uppercase() == "NAND" {
                // Why not use get_name here?
                let a = self.signals.get_bus(&Bus::from("a"))[0];
                let b = self.signals.get_bus(&Bus::from("b"))[0];
                let new_value = vec![nand(a, b)];
                self.signals.insert_option(&Bus::from("out"), new_value);
                return Ok(());
            } else if self.name.to_uppercase() == "DFF" {
                let current_value = self.signals.get_bus(&Bus::from("out"))[0];
                let new_value = self.signals.get_bus(&Bus::from("in"))[0];

                if new_value.is_none() || current_value == new_value {
                    return Ok(());
                }

                // chase parents up to the top level chip
                // mark everything along the way as no cache because
                // those chips now depend on a DFF with a pending write.
                let mut parent = self.parent;
                while !parent.is_null() {
                    let parent_chip;
                    unsafe {
                        parent_chip = &mut *parent;
                    }
                    parent_chip.cache = false;
                    parent = parent_chip.parent;
                }
            } else if self.name.to_uppercase() == "BUFFER" {
                let r = self.signals.get_name("in");
                self.signals.insert_option(&Bus::from("out"), r);
                return Ok(());
            }

            let cache_entry = InputCacheEntry {
                name: self.name.clone(),
                signals: self.get_port_values_for_direction(PortDirection::In),
            };

            if !self.elaborated && self.cache && input_cache.contains_key(&cache_entry) {
                let cached_outputs = input_cache.get(&cache_entry).unwrap();

                // set output signals directly
                for o in cached_outputs.signals() {
                    let width = cached_outputs.get_width(&o).unwrap();
                    let bus = Bus {
                        name: o.clone(),
                        range: Some(0..width),
                    };
                    let value = cached_outputs.get_bus(&bus);
                    self.signals.insert_option(&bus, value);
                }
                return Ok(());
            }

            if !self.elaborated {
                self.elaborate()?;
            }

            // copy chip inputs into dummy subcomponents as graph entry points
            for &port_idx in &self.input_port_nodes {
                let port_component = self.circuit.node_weight_mut(port_idx).unwrap();
                let new_val = self.signals.get_name(&port_component.name);
                port_component.signals.insert_option(
                    &Bus {
                        name: String::from("in"),
                        range: Some(0..new_val.len()),
                    },
                    new_val,
                );
            }

            // Compute our value by computing subcomponents.
            // Pick subcomponents in SCC topo order
            let mut sccs = kosaraju_scc(&self.circuit);
            sccs.reverse();

            for scc in &sccs {
                for &component_idx in scc {
                    // Compute component bus values.
                    {
                        let component = self.circuit.node_weight_mut(component_idx).unwrap();
                        component.compute(input_cache, dirty_dffs)?;
                    }

                    self.mark_neighbors(component_idx, dirty_dffs);
                }
            }
        }

        // populate output buses
        for &port_idx in &self.output_port_nodes {
            let port_component = self.circuit.node_weight_mut(port_idx).unwrap();
            let new_val = port_component.signals.get_name("in");
            self.signals.insert_option(
                &Bus {
                    name: port_component.name.clone(),
                    range: Some(0..new_val.len()),
                },
                new_val,
            );
        }

        if self.cache {
            self.insert_cache_entry(input_cache)
        }

        Ok(())
    }

    pub fn eval_port_width(
        &self,
        port: &GenericPort,
        component_hdl: &ChipHDL,
        component: &Component,
    ) -> Result<usize, N2VError> {
        let mut resolved_generics: Vec<usize> = Vec::new();
        for g in &component.generic_params {
            if let GenericWidth::Terminal(Terminal::Num(w)) = g {
                resolved_generics.push(*w);
            } else if let GenericWidth::Terminal(Terminal::Var(w)) = g {
                let rw = self.variables.get(&w.value).unwrap();
                resolved_generics.push(*rw);
            }
        }

        let component_variables: HashMap<String, usize> = component_hdl
            .generic_decls
            .iter()
            .map(|x| x.value.clone())
            .zip(resolved_generics)
            .collect();

        eval_expr_numeric(&port.width, &component_variables)
    }
}

// Combines adjacent edges
fn optimize_circuit(circuit: &mut Circuit) {
    // node indices are stable during edge removal.
    // Collect all node indices here to avoid graph borrow.
    let all_nodes: Vec<NodeIndex> = circuit.node_indices().collect();

    // each node index and get the neighbor edges for that node via
    // a detached iterator to avoid borrowing from the graph.
    for node_idx in all_nodes {
        let neighbors: HashSet<NodeIndex> = circuit.neighbors(node_idx).into_iter().collect();
        for neighbor in neighbors {
            // Edges connecting the two neighboring nodes
            let mut connecting_edges: Vec<EdgeIndex> = circuit
                .edges_connecting(node_idx, neighbor)
                .into_iter()
                .map(|x| x.id())
                .collect();

            // Sort the edge indices first by port, then sub-sort by target bus bit.
            // source bit could be constant so can't use that to sort.
            connecting_edges.sort_by(|u, v| {
                let u_bus = &circuit.edge_weight(*u).unwrap().target;
                let v_bus = &circuit.edge_weight(*v).unwrap().target;

                let u_range = u_bus.range.as_ref().unwrap_or(&Range { start: 0, end: 1 });
                let v_range = v_bus.range.as_ref().unwrap_or(&Range { start: 0, end: 1 });

                if u_bus.name == v_bus.name {
                    u_range.start.cmp(&v_range.start)
                } else {
                    u_bus.name.cmp(&v_bus.name)
                }
            });

            let mut initial_vec = Vec::new();
            let new_edge_weights: &mut Vec<Wire> =
                connecting_edges
                    .iter_mut()
                    .fold(&mut initial_vec, |acc, e| {
                        let edge_weight = circuit.edge_weight(*e).unwrap();
                        if acc.is_empty() {
                            acc.push(edge_weight.clone());
                            return acc;
                        }

                        let prev_edge = acc[acc.len() - 1].clone();
                        let cur_edge = circuit.edge_weight(*e).unwrap();
                        let prev_source_bus = &prev_edge.source;
                        let prev_target_bus = &prev_edge.target;
                        let cur_source_bus = &cur_edge.source;
                        let cur_target_bus = &cur_edge.target;
                        let prev_source_range = prev_source_bus
                            .range
                            .as_ref()
                            .unwrap_or(&Range { start: 0, end: 1 });
                        let prev_target_range = prev_target_bus
                            .range
                            .as_ref()
                            .unwrap_or(&Range { start: 0, end: 1 });
                        let cur_source_range = cur_source_bus
                            .range
                            .as_ref()
                            .unwrap_or(&Range { start: 0, end: 1 });
                        let cur_target_range = cur_target_bus
                            .range
                            .as_ref()
                            .unwrap_or(&Range { start: 0, end: 1 });

                        // Merge
                        if cur_source_range.start == prev_source_range.end
                            && (cur_target_range.start == prev_target_range.end)
                            && (cur_source_bus.name == prev_source_bus.name)
                            && (cur_target_bus.name == prev_target_bus.name)
                        {
                            acc.remove(acc.len() - 1);
                            acc.push(Wire {
                                source: Bus {
                                    name: prev_source_bus.name.clone(),
                                    range: Some(Range {
                                        start: prev_source_range.start,
                                        end: cur_source_range.end,
                                    }),
                                },
                                target: Bus {
                                    name: prev_target_bus.name.clone(),
                                    range: Some(Range {
                                        start: prev_target_range.start,
                                        end: cur_target_range.end,
                                    }),
                                },
                            })
                        } else {
                            acc.push(edge_weight.clone());
                        }

                        acc
                    });

            // remove all existing edges between neighbors
            // Sort in descending order because removing an edge invalidates last edge index in graph.
            connecting_edges.sort_by(|a, b| b.cmp(a));
            for e in connecting_edges {
                circuit.remove_edge(e);
            }

            // add new edges
            for w in new_edge_weights {
                circuit.add_edge(node_idx, neighbor, w.clone());
            }
        }
    }
}

/// Creates chips for 16-bit true/false literals.
fn make_literal_chip(
    value: Option<bool>,
    parent: *mut Chip,
    hdl_provider: &Rc<dyn HdlProvider>,
) -> Chip {
    let circuit = Circuit::new();

    let mut signals = BusMap::new();
    signals.create_bus("out", 16).unwrap();
    signals.insert_option(&Bus::from("out"), vec![value; 16]);

    let name = match value {
        None => String::from("none"),
        Some(x) => x.to_string(),
    };

    Chip {
        name,
        ports: HashMap::new(),
        signals,
        hdl: None,
        elaborated: false,
        circuit,
        dirty: false,
        input_port_nodes: Vec::new(),
        output_port_nodes: Vec::new(),
        cache: false,
        parent,
        hdl_provider: Rc::clone(hdl_provider),
        variables: HashMap::new(),
        components: Vec::new(),
        assignments: Vec::new(),
    }
}

// cache lookup will always return correct output for nands.
fn make_nand_chip(parent: *mut Chip, hdl_provider: &Rc<dyn HdlProvider>) -> Chip {
    let circuit = Circuit::new();
    let mut signals = BusMap::new();
    signals.create_bus("a", 1).unwrap();
    signals.create_bus("b", 1).unwrap();
    signals.create_bus("out", 1).unwrap();
    let ports = HashMap::from([
        (
            String::from("a"),
            Port {
                name: Identifier::from("a"),
                width: 1,
                direction: PortDirection::In,
            },
        ),
        (
            String::from("b"),
            Port {
                name: Identifier::from("b"),
                width: 1,
                direction: PortDirection::In,
            },
        ),
        (
            String::from("out"),
            Port {
                name: Identifier::from("out"),
                width: 1,
                direction: PortDirection::Out,
            },
        ),
    ]);

    Chip {
        name: String::from("nand"),
        ports,
        signals,
        hdl: None,
        elaborated: false,
        circuit,
        dirty: false,
        input_port_nodes: Vec::new(),
        output_port_nodes: Vec::new(),
        cache: false,
        parent,
        hdl_provider: Rc::clone(hdl_provider),
        variables: HashMap::new(),
        components: Vec::new(),
        assignments: Vec::new(),
    }
}

fn make_port_chip(
    name: &str,
    width: usize,
    parent: *mut Chip,
    hdl_provider: &Rc<dyn HdlProvider>,
) -> Chip {
    let circuit = Circuit::new();
    let ports = HashMap::new();
    let mut signals = BusMap::new();
    signals.create_bus("in", width).unwrap();
    signals.create_bus("out", width).unwrap();

    Chip {
        name: String::from(name),
        ports,
        signals,
        hdl: None,
        elaborated: true,
        circuit,
        dirty: false,
        input_port_nodes: Vec::new(),
        output_port_nodes: Vec::new(),
        cache: false,
        parent,
        hdl_provider: Rc::clone(hdl_provider),
        variables: HashMap::new(),
        components: Vec::new(),
        assignments: Vec::new(),
    }
}

fn make_dff_chip(parent: *mut Chip, hdl_provider: &Rc<dyn HdlProvider>) -> Chip {
    let circuit = Circuit::new();
    let mut signals = BusMap::new();
    signals.create_bus("in", 1).unwrap();
    signals.create_bus("out", 1).unwrap();
    signals.insert_option(
        &Bus {
            name: String::from("in"),
            range: Some(0..1),
        },
        vec![Some(false)],
    );
    signals.insert_option(
        &Bus {
            name: String::from("out"),
            range: Some(0..1),
        },
        vec![Some(false)],
    );

    Chip {
        name: String::from("DFF"),
        ports: HashMap::from([
            (
                String::from("in"),
                Port {
                    direction: PortDirection::In,
                    name: Identifier::from("in"),
                    width: 1,
                },
            ),
            (
                String::from("out"),
                Port {
                    direction: PortDirection::Out,
                    name: Identifier::from("out"),
                    width: 1,
                },
            ),
        ]),
        signals,
        hdl: None,
        elaborated: false,
        circuit,
        dirty: false,
        input_port_nodes: Vec::new(),
        output_port_nodes: Vec::new(),
        cache: false,
        parent,
        hdl_provider: Rc::clone(hdl_provider),
        variables: HashMap::new(),
        components: Vec::new(),
        assignments: Vec::new(),
    }
}

// Return the width of port name in hdl instantiated as component under parent variables.

/// Infer signal widths.
/// If port mapping wire range is None then use port width, otherwise use range width.
/// signal width is width from last step.
/// if no signal range is given and signal already has a width, verify that width matches.
///
/// * `hdl` - HDL for the chip that the signals belong to
/// * 'assignments' - Vector of assignments pulled from the parts of hdl
/// * `components` - Components to use when inferring widths. May or may not be
///                  the same as HDL components due to loop expansion.
/// * `provider` - Responsible for retrieving HDL
/// * `generics` - Generic values for instantiating chip corresponding to HDL (not a subcomponent).
pub fn infer_widths(
    hdl: &ChipHDL,
    assignments: &Vec<AssignmentHDL>,
    components: &Vec<Component>,
    provider: &Rc<dyn HdlProvider>,
    generics: &Vec<GenericWidth>,
) -> Result<HashMap<String, GenericWidth>, Box<dyn Error>> {
    // Assign values to generic variables.
    if generics.len() > hdl.generic_decls.len() {
        return Err(Box::new(N2VError {
            msg: format!(
                "Chip {} declares {} generics but instantiated with {}",
                hdl.name,
                hdl.generic_decls.len(),
                generics.len()
            ),
            kind: ErrorKind::SimulationError(hdl.path.clone()),
        }));
    }
    let mut variables = HashMap::new();

    #[allow(clippy::needless_range_loop)]
    for gv in 0..generics.len() {
        variables.insert(hdl.generic_decls[gv].value.clone(), generics[gv].clone());
    }

    let mut inferred_widths: HashMap<String, GenericWidth> = HashMap::new();
    for port in &hdl.ports {
        inferred_widths.insert(port.name.value.clone(), eval_expr(&port.width, &variables));
    }
    let mut last_inferred_widths: HashMap<String, GenericWidth> = HashMap::new();
    loop {
        last_inferred_widths = inferred_widths.clone();
        for part in components {
            let component_hdl = get_hdl(&part.name.value, provider)?;
            // Convert generics with vars to concrete generics for component.
            // e.g. Mux<W> needs to become Mux<4> if W=4. At this point
            // we need actual bus widths.
            let generic_params: Vec<GenericWidth> = part
                .generic_params
                .iter()
                .map(|g| eval_expr(g, &variables))
                .collect();

            // Do not create a component chip here because that will
            // trigger elaboration of the entire component tree.
            // We only need the ports, and ports cannot be created with
            // for  loops, so this is sufficient enough to get
            // the variables map for looking up port widths.
            let component_variables: HashMap<String, GenericWidth> = component_hdl
                .generic_decls
                .iter()
                .map(|x| x.value.clone())
                .zip(generic_params)
                .collect();

            for m in &part.mappings {
                // skip false and true pseudo-signals
                // TODO: Check to make sure that no chip is writing to false/true.
                if &m.wire.name.to_lowercase() == "false"
                    || &m.wire.name.to_ascii_lowercase() == "true"
                    || &m.wire.name.to_ascii_lowercase() == "none"
                {
                    continue;
                }

                // Ports are stored in a vector in HDL. Find the index
                // of the port referred to in this port mapping and
                // retrieve the port struct from the component.
                let port_idx = component_hdl
                    .ports
                    .iter()
                    .position(|x| x.name.value == m.port.name)
                    .ok_or(N2VError {
                        msg: format!("Non-existent port {}", &m.port.name),
                        kind: ErrorKind::ParseIdentError(provider.clone(), part.name.clone()),
                    })?;
                let port = &component_hdl.ports[port_idx];

                // I need to make the port_width from the component match the wire
                // Get the width of the port referred to in the mapping.
                // This uses the component chip variables because the width of the port is defined inside the component
                let hdl_port_width = eval_expr(&port.width, &component_variables);

                let wire_start = m.wire.start.as_ref().map(|x| eval_expr(x, &variables));
                let wire_end = m.wire.end.as_ref().map(|x| eval_expr(x, &variables));

                // Convert inclusive range in HDL to exclusive Range in Rust
                let mp_wire_range: Option<Range<GenericWidth>> = wire_start.map(|ws| Range {
                    start: ws,
                    end: wire_end.unwrap() + GenericWidth::Terminal(Terminal::Num(1)),
                });
                let port_start = m.port.start.as_ref().map(|x| eval_expr(x, &variables));
                let port_end = m.port.end.as_ref().map(|x| eval_expr(x, &variables));
                // Convert inclusive range in HDL to exclusive Range in Rust
                let mp_port_range: Option<Range<GenericWidth>> = port_start.map(|ps| Range {
                    start: ps,
                    end: port_end.unwrap() + GenericWidth::Terminal(Terminal::Num(1)),
                });

                // To line up widths, use the extracted, port range, wire range from the mapping, and any width previously found
                match (
                    &mp_wire_range,
                    &mp_port_range,
                    inferred_widths.get(&m.wire.name),
                ) {
                    // wire range none, mapping port range none, previous inferred width none => use port width from component ports
                    (None, None, None) => {
                        inferred_widths.insert(m.wire.name.clone(), hdl_port_width);
                    }

                    // wire range none, port range none, width some => verify width = port width
                    (None, None, Some(w)) => {
                        if w.is_numeric() && w != &hdl_port_width {
                            return Err(Box::new(N2VError { msg: format!(
                                "Chip {} component {} inferred width of signal {} is {}, not equal to width of port {} which is {}.",
                                &hdl.name, &component_hdl.name, &m.wire.name, w, &m.port.name, &hdl_port_width
                            ),
                                                           kind: ErrorKind::ParseIdentError(provider.clone(), m.wire_ident.clone()),
                            }));
                        }
                    }

                    // wire range none, port range some, width none => use len of port range
                    (None, Some(pr), None) => {
                        inferred_widths.insert(m.wire.name.clone(), &pr.end - &pr.start);
                    }

                    // wire range none, port range some, width some => verify width same as port range
                    (None, Some(pr), Some(w)) => {
                        if w.is_numeric() && w != &(&pr.end - &pr.start) {
                            return Err(Box::new(N2VError { msg: format!("Chip {} component {} inferred width of signal {} is {}, not equal to width of port {} range which is {}.",
                                &hdl.name, &component_hdl.name, &m.wire.name, w, &m.port.name, &pr.end - &pr.start
                            ),
                            kind: ErrorKind::ParseIdentError(provider.clone(), m.wire_ident.clone()),
                        }));
                        }
                    }

                    // wire range some, port range none, width none => verify wire range = port width. Use wire max index as wire width.
                    (Some(wr), None, None) => {
                        if wr.end.is_numeric()
                            && wr.start.is_numeric()
                            && (&wr.end - &wr.start) != hdl_port_width
                        {
                            return Err(Box::new(N2VError { msg: format!("Chip {} component {} inferred width of signal {} is {}, not equal to width of port {} width which is {}.",
                                &hdl.name, &component_hdl.name, &m.wire.name, (&wr.end - &wr.start), &m.port.name, hdl_port_width
                            ),
                            kind: ErrorKind::ParseIdentError(provider.clone(), m.wire_ident.clone()),
                        }));
                        }
                        inferred_widths.insert(m.wire.name.clone(), wr.end.clone());
                    }

                    // wire range some, port range none, width some => verify wire range = port width. Use max(wire max index, existing width).
                    (Some(wr), None, Some(w)) => {
                        if wr.end.is_numeric()
                            && wr.start.is_numeric()
                            && (&wr.end - &wr.start) != hdl_port_width
                        {
                            return Err(Box::new(N2VError { msg: format!("Chip `{}` component `{}` wire range of signal `{}` is {}, not equal port `{}` width, which is {}.",
                                &hdl.name, &component_hdl.name, &m.wire.name, (&wr.end - &wr.start), &m.port.name, &hdl_port_width
                            ),
                            kind: ErrorKind::ParseIdentError(provider.clone(), m.wire_ident.clone()),
                        }));
                        }
                        let max_width = eval_expr(
                            &GenericWidth::Expr(
                                Op::Max,
                                Box::new(wr.end.clone()),
                                Box::new(w.clone()),
                            ),
                            &variables,
                        );
                        inferred_widths.insert(m.wire.name.clone(), max_width);
                    }

                    // wire range some, port range some, width none => verify wire range = port range. Use wire max index as wire width.
                    (Some(wr), Some(pr), None) => {
                        if wr.end.is_numeric()
                            && wr.start.is_numeric()
                            && pr.end.is_numeric()
                            && pr.start.is_numeric()
                            && (&wr.end - &wr.start) != (&pr.end - &pr.start)
                        {
                            return Err(Box::new(N2VError { msg: format!("Chip {} component {} inferred width of signal {} is {}, not equal to width of port {} range which is {}.",
                                &hdl.name, &component_hdl.name, &m.wire.name, (&wr.end - &wr.start), &m.port.name, (&pr.end - &pr.start)
                            ),
                            kind: ErrorKind::ParseIdentError(provider.clone(), m.wire_ident.clone()),
                        }));
                        }
                        inferred_widths.insert(m.wire.name.clone(), wr.end.clone());
                    }

                    // wire range some, port range some, width some => verify wire range = port range. Use max(wire max index, existing width).
                    (Some(wr), Some(pr), Some(w)) => {
                        if wr.end.is_numeric()
                            && wr.start.is_numeric()
                            && pr.end.is_numeric()
                            && pr.start.is_numeric()
                            && (&wr.end - &wr.start) != (&pr.end - &pr.start)
                        {
                            return Err(Box::new(N2VError { msg: format!("Chip {} component {} inferred width of signal {} is {}, not equal to width of port {} range which is {}.",
                                &hdl.name, &component_hdl.name, &m.wire.name, (&wr.end - &wr.start), &m.port.name, (&pr.end - &pr.start)
                            ),
                            //line: m.wire_ident.line,
                            kind: ErrorKind::ParseIdentError(provider.clone(), m.wire_ident.clone()),
                        }));
                        }

                        let max_width = eval_expr(
                            &GenericWidth::Expr(
                                Op::Max,
                                Box::new(wr.end.clone()),
                                Box::new(w.clone()),
                            ),
                            &variables,
                        );
                        inferred_widths.insert(m.wire.name.clone(), max_width);
                    }
                }
            }
        }
        if inferred_widths == last_inferred_widths {
            loop {
                // This runs until fixpoint as well to deal with multiple layers of redirection
                last_inferred_widths = inferred_widths.clone();
                for a in assignments {
                    let wl = inferred_widths.get(&a.left.name.clone());
                    let wr = inferred_widths.get(&a.right.name.clone());

                    match (wl, wr) {
                        (Some(w), None) => {
                            inferred_widths.insert(a.right.name.clone(), w.clone());
                        }
                        (None, Some(w)) => {
                            inferred_widths.insert(a.left.name.clone(), w.clone());
                        }
                        (Some(w1), Some(w2)) => {
                            if w1 != w2 {
                                let wname = a.right.name.clone();
                                return Err(Box::new(N2VError {
                                    msg: format!(
                                        "Signal widths of {} and {} are not equal.",
                                        &a.left.name.clone(),
                                        &a.right.name.clone(),
                                    ),
                                    kind: ErrorKind::ParseIdentError(
                                        provider.clone(),
                                        Identifier::from(wname.as_str()),
                                    ),
                                }));
                            }
                        }
                        (None, None) => {}
                    }
                }
                if inferred_widths == last_inferred_widths {
                    break;
                }
            }
            for a in assignments {
                match (
                    inferred_widths.get(&a.left.name.clone()),
                    inferred_widths.get(&a.right.name.clone()),
                ) {
                    // If neither widths have a source, throw an error. This allows us to make assumptions about widths later on.
                    (None, None) => {
                        return Err(Box::new(N2VError {
                            msg: format!(
                                "Signals {} and {} have no source or destination.",
                                &a.left.name.clone(),
                                &a.right.name.clone(),
                            ),
                            kind: ErrorKind::ParseIdentError(
                                provider.clone(),
                                Identifier::from(a.right.name.clone().as_str()),
                            ),
                        }));
                    }
                    _ => {}
                }
            }
            break;
        }
    }

    Ok(inferred_widths)
}

/// Consolidates all assignments within the vector of parts passed as argument.
pub fn gather_assignments(parts: &Vec<Part>) -> Vec<AssignmentHDL> {
    let mut assignment_vec = Vec::new();
    for part in parts {
        match part {
            Part::AssignmentHDL(pa) => {
                assignment_vec.push(pa.clone());
            }
            _ => {}
        }
    }

    assignment_vec
}

fn nand(a: Option<bool>, b: Option<bool>) -> Option<bool> {
    if a.is_none() || b.is_none() {
        return None;
    }

    Some(!(a.unwrap() && b.unwrap()))
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::scanner::Scanner;
    use std::env;
    use std::path::Path;
    use std::ptr;

    fn make_simulator(file_name: &str) -> Simulator {
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
        let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(&base_path));
        let contents = provider.get_hdl(file_name).unwrap();
        let mut scanner = Scanner::new(contents.as_str(), provider.get_path(file_name));
        let mut parser = Parser {
            scanner: &mut scanner,
        };
        let hdl = parser.parse().expect("Parse error");
        let chip = Chip::new(&hdl, ptr::null_mut(), &provider, false, &Vec::new())
            .expect("Chip creation error");
        Simulator::new(chip)
    }

    #[test]
    fn test_nand2tetris_solution_not() {
        let mut simulator = make_simulator("Not.hdl");
        let inputs = BusMap::try_from([("in", false)]).expect("Error creating inputs");
        let outputs = simulator.simulate(&inputs).expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(true)]);
    }

    #[test]
    fn test_simulator_buffer() {
        let mut simulator = make_simulator("../../buffer/Buffer.hdl");
        let inputs = BusMap::try_from([("testin", false)]).expect("Error creating inputs");
        let outputs = simulator.simulate(&inputs).expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("testout")), vec![Some(false)]);
    }

    #[test]
    fn test_simulator_buffer2() {
        let mut simulator = make_simulator("../../buffer/Buffer2.hdl");
        let inputs = BusMap::try_from([("testin", false)]).expect("Error creating inputs");
        let outputs = simulator.simulate(&inputs).expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("testout")), vec![Some(false)]);
    }

    #[test]
    fn test_simulator_buffer3() {
        let mut simulator = make_simulator("../../buffer/BufferTest3.hdl");
        let inputs = BusMap::try_from([("testin", false)]).expect("Error creating inputs");
        let outputs = simulator.simulate(&inputs).expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("testout")), vec![Some(false)]);
    }

    #[test]
    fn test_simulator_buffer4() {
        let mut simulator = make_simulator("../../buffer/Buffer4.hdl");
        let inputs = BusMap::try_from([("in", false)]).expect("Error creating inputs");
        let outputs = simulator.simulate(&inputs).expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(false)]);
    }

    #[test]
    fn test_simulator_buffer5() {
        let mut simulator = make_simulator("../../buffer/Buffer5.hdl");
        let inputs = BusMap::try_from([("in", vec![true, false])]).expect("Error creating inputs");
        let outputs = simulator.simulate(&inputs).expect("simulation failure");
        assert_eq!(
            outputs.get_bus(&Bus::from("out")),
            vec![Some(true), Some(false)]
        );
    }

    #[test]
    fn test_simulator_buffer_literal() {
        let mut simulator = make_simulator("../../buffer/BufferLiterals.hdl");
        let inputs = BusMap::try_from([("in", true)]).expect("Error creating inputs");
        let outputs = simulator.simulate(&inputs).expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(true)]);
    }

    #[test]
    fn test_nand2tetris_solution_and() {
        let mut simulator = make_simulator("And.hdl");
        let outputs = simulator
            .simulate(
                &BusMap::try_from([("a", true), ("b", true)]).expect("Error creating inputs."),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(true)]);

        let outputs = simulator
            .simulate(
                &BusMap::try_from([("a", false), ("b", true)]).expect("Error creating inputs."),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(false)]);

        let outputs = simulator
            .simulate(
                &BusMap::try_from([("a", true), ("b", false)]).expect("Error creating inputs."),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(false)]);

        let outputs = simulator
            .simulate(
                &BusMap::try_from([("a", false), ("b", false)]).expect("Error creating inputs."),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(false)]);
    }

    #[test]
    fn test_nand2tetris_solution_mux() {
        let mut simulator = make_simulator("Mux.hdl");
        let outputs = simulator
            .simulate(
                &BusMap::try_from([("a", false), ("b", false), ("sel", false)])
                    .expect("Error creating inputs"),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(false)]);

        let outputs = simulator
            .simulate(
                &BusMap::try_from([("a", false), ("b", true), ("sel", true)])
                    .expect("Error creating inputs."),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(true)]);
    }

    #[test]
    fn test_nand2tetris_solution_dmux() {
        let mut simulator = make_simulator("DMux.hdl");
        let outputs = simulator
            .simulate(
                &BusMap::try_from([("in", false), ("sel", false)]).expect("Error creating inputs."),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("a")), vec![Some(false)]);
        assert_eq!(outputs.get_bus(&Bus::from("b")), vec![Some(false)]);

        let outputs = simulator
            .simulate(
                &BusMap::try_from([("in", false), ("sel", true)]).expect("Error creating inputs."),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("a")), vec![Some(false)]);
        assert_eq!(outputs.get_bus(&Bus::from("b")), vec![Some(false)]);

        let outputs = simulator
            .simulate(
                &BusMap::try_from([("in", true), ("sel", false)]).expect("Error creating inputs."),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("a")), vec![Some(true)]);
        assert_eq!(outputs.get_bus(&Bus::from("b")), vec![Some(false)]);

        let outputs = simulator
            .simulate(
                &BusMap::try_from([("in", true), ("sel", true)]).expect("Error creating inputs."),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("a")), vec![Some(false)]);
        assert_eq!(outputs.get_bus(&Bus::from("b")), vec![Some(true)]);
    }

    #[test]
    fn test_nand2tetris_solution_dmux4way() {
        let mut simulator = make_simulator("DMux4Way.hdl");
        let outputs = simulator
            .simulate(
                &BusMap::try_from([("in", vec![false]), ("sel", vec![false, false])])
                    .expect("Error creating inputs."),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&Bus::from("a")), vec![Some(false)]);
        assert_eq!(outputs.get_bus(&Bus::from("b")), vec![Some(false)]);
        assert_eq!(outputs.get_bus(&Bus::from("c")), vec![Some(false)]);
        assert_eq!(outputs.get_bus(&Bus::from("d")), vec![Some(false)]);
    }

    #[test]
    fn test_nand2tetris_solution_not16() {
        let mut simulator = make_simulator("Not16.hdl");
        let outputs = simulator
            .simulate(&BusMap::try_from([("in", vec![false; 16])]).unwrap())
            .expect("simulation failure");
        let b = Bus {
            name: String::from("out"),
            range: Some(0..16),
        };
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
    }

    #[test]
    fn test_nand2tetris_solution_and16() {
        let mut simulator = make_simulator("And16.hdl");
        let outputs = simulator
            .simulate(&BusMap::try_from([("a", vec![true; 16]), ("b", vec![true; 16])]).unwrap())
            .expect("simulation failure");
        let b = Bus {
            name: String::from("out"),
            range: Some(0..16),
        };
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
    }

    #[test]
    fn test_nand2tetris_solution_alu() {
        let mut simulator = make_simulator("ALU.hdl");
        let outputs = simulator
            .simulate(
                &BusMap::try_from([
                    ("x", vec![false; 16]),
                    ("y", vec![true; 16]),
                    ("zx", vec![false]),
                    ("nx", vec![false]),
                    ("zy", vec![false]),
                    ("ny", vec![false]),
                    ("f", vec![true]),
                    ("no", vec![false]),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        let b = Bus {
            name: String::from("out"),
            range: Some(0..16),
        };
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
    }

    #[test]
    fn test_nand2tetris_solution_bit() {
        let mut simulator = make_simulator("Bit.hdl");
        let outputs = simulator
            .simulate(&BusMap::try_from([("in", vec![true]), ("load", vec![true])]).unwrap())
            .expect("simulation failure");
        assert_eq!(
            outputs.get_bus(&Bus::try_from("out").unwrap()),
            vec![Some(false)]
        );
        simulator.tick().expect("Tick failure");
        simulator
            .simulate(&BusMap::try_from([("in", vec![false]), ("load", vec![false])]).unwrap())
            .expect("simulation failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(true)]);

        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&Bus::from("out")), vec![Some(true)]);
    }

    #[test]
    fn test_nand2tetris_solution_register() {
        let mut simulator = make_simulator("Register.hdl");
        let b = Bus {
            name: String::from("out"),
            range: Some(0..16),
        };

        let outputs = simulator
            .simulate(&BusMap::try_from([("in", vec![true; 16]), ("load", vec![true])]).unwrap())
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&b), vec![Some(false); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator.tick().expect("Tick failure");
        simulator
            .simulate(&BusMap::try_from([("in", vec![false; 16]), ("load", vec![false])]).unwrap())
            .expect("simulation failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
    }

    #[test]
    fn test_nand2tetris_solution_ram8() {
        let mut simulator = make_simulator("RAM8.hdl");
        let b = Bus {
            name: String::from("out"),
            range: Some(0..16),
        };

        let outputs = simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![true]),
                    ("address", vec![false, true, false]),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&b), vec![Some(false); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![true]),
                    ("address", vec![false, true, false]),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![false]),
                    ("address", vec![false, false, false]),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        //simulator.tick();
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(false); 16]);
    }

    #[test]
    fn test_nand2tetris_solution_ram512() {
        let mut simulator = make_simulator("RAM512.hdl");
        let b = Bus {
            name: String::from("out"),
            range: Some(0..16),
        };

        let address1 = vec![false, false, false, false, false, false, false, true, false];
        let address2 = vec![false, false, false, false, false, false, true, false, false];

        let outputs = simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![true]),
                    ("address", address1.clone()),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&b), vec![Some(false); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![true]),
                    ("address", address1),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![false]),
                    ("address", address2),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(false); 16]);
    }

    #[test]
    fn test_nand2tetris_solution_ram4k() {
        let mut simulator = make_simulator("RAM4K.hdl");
        let b = Bus {
            name: String::from("out"),
            range: Some(0..16),
        };

        let address1 = vec![
            false, false, false, false, false, false, false, true, false, false, false, false,
        ];
        let address2 = vec![
            false, false, false, false, false, false, true, false, false, false, false, false,
        ];

        let outputs = simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![true]),
                    ("address", address1.clone()),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&b), vec![Some(false); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![true]),
                    ("address", address1),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![false]),
                    ("address", address2),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(false); 16]);
    }

    #[test]
    fn test_nand2tetris_solution_ram16k() {
        let mut simulator = make_simulator("RAM16K.hdl");
        let b = Bus {
            name: String::from("out"),
            range: Some(0..16),
        };

        let address1 = vec![
            false, false, false, false, false, false, false, true, false, false, false, false,
            true, false,
        ];
        let address2 = vec![
            false, false, false, false, false, false, true, false, false, false, false, false,
            true, true,
        ];
        let address3 = vec![
            true, true, true, true, true, true, true, true, false, false, true, true, true, true,
        ];

        let outputs = simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![true]),
                    ("address", address1.clone()),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&b), vec![Some(false); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![true]),
                    ("address", address1),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);

        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);
        simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![true; 16]),
                    ("load", vec![true]),
                    ("address", address2),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(false); 16]);
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(true); 16]);

        simulator
            .simulate(
                &BusMap::try_from([
                    ("in", vec![false; 16]),
                    ("load", vec![true]),
                    ("address", address3),
                ])
                .unwrap(),
            )
            .expect("simulation failure");
        simulator.tick().expect("Tick failure");
        let outputs = simulator
            .chip
            .get_port_values_for_direction(PortDirection::Out);
        assert_eq!(outputs.get_bus(&b), vec![Some(false); 16]);
    }

    #[test]
    fn test_optimize_circuit_empty() {
        let mut c = Circuit::new();
        optimize_circuit(&mut c);
        assert_eq!(c.node_count(), 0);
    }

    #[test]
    fn test_optimize_circuit_single() {
        let mut c = Circuit::new();
        optimize_circuit(&mut c);
        assert_eq!(c.node_count(), 0);
    }

    #[test]
    fn test_optimize_circuit_and16() {
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
        let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(&base_path));
        let contents = provider.get_hdl("And16.hdl").unwrap();
        let mut scanner = Scanner::new(contents.as_str(), provider.get_path("And16.hdl"));
        let mut parser = Parser {
            scanner: &mut scanner,
        };
        let hdl = parser.parse().expect("Parse error");
        let chip = Chip::new(&hdl, ptr::null_mut(), &provider, true, &Vec::new())
            .expect("Chip creation error");
        assert_eq!(chip.circuit.edge_count(), 48);
    }

    #[test]
    fn test_optimize_circuit_useand16() {
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
        let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(&base_path));
        let contents = "CHIP And {
      IN a[16], b[16];
      OUT out[16];

      PARTS:
      And16(a=a, b=b, out=out);
      }";

        let mut scanner = Scanner::new(contents, provider.get_path("Blah.hdl"));
        let mut parser = Parser {
            scanner: &mut scanner,
        };
        let hdl = parser.parse().expect("Parse error");
        let chip = Chip::new(&hdl, ptr::null_mut(), &provider, true, &Vec::new())
            .expect("Chip creation error");
        assert_eq!(chip.circuit.edge_count(), 3);
    }

    #[test]
    fn test_optimize_circuit_inc16() {
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
        let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(&base_path));
        let contents = provider.get_hdl("Inc16.hdl").unwrap();
        let mut scanner = Scanner::new(contents.as_str(), provider.get_path("Inc16.hdl"));
        let mut parser = Parser {
            scanner: &mut scanner,
        };
        let hdl = parser.parse().expect("Parse error");
        let chip = Chip::new(&hdl, ptr::null_mut(), &provider, true, &Vec::new())
            .expect("Chip creation error");
        assert_eq!(chip.circuit.edge_count(), 4);
    }

    // Tests that multiple assignments to the same bit of a signal produce
    // an error. See https://github.com/whidl/whidl/issues/9
    #[test]
    fn test_assign_multiple_error() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let base_path = String::from(
            manifest_dir
                .join("resources")
                .join("tests")
                .join("bad")
                .to_str()
                .unwrap(),
        );
        let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(&base_path));
        let contents = provider.get_hdl("TwoAssign.hdl").unwrap();
        let mut scanner = Scanner::new(contents.as_str(), provider.get_path("TwoAssign.hdl"));
        let mut parser = Parser {
            scanner: &mut scanner,
        };
        let hdl = parser.parse().expect("Parse error");
        let chip = Chip::new(&hdl, ptr::null_mut(), &provider, true, &Vec::new());
        assert!(chip.is_err());
    }

    // Tests that multiple assignments to the same bit of a signal produce
    // an error. See https://github.com/whidl/whidl/issues/9
    #[test]
    fn test_assign_multiple_ok() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let base_path = String::from(
            manifest_dir
                .join("resources")
                .join("tests")
                .join("bad")
                .to_str()
                .unwrap(),
        );
        let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(&base_path));
        let contents = provider.get_hdl("TwoAssignOK.hdl").unwrap();
        let mut scanner = Scanner::new(contents.as_str(), provider.get_path("TwoAssignOK.hdl"));
        let mut parser = Parser {
            scanner: &mut scanner,
        };
        let hdl = parser.parse().expect("Parse error");
        let chip = Chip::new(&hdl, ptr::null_mut(), &provider, true, &Vec::new());
        assert!(chip.is_ok());
    }

    // Tests that component instantiations provide inputs for all bits of component input ports.
    #[test]
    fn test_disconnected_component_inputs() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let base_path = String::from(
            manifest_dir
                .join("resources")
                .join("tests")
                .join("bad")
                .to_str()
                .unwrap(),
        );
        let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(&base_path));
        let contents = provider.get_hdl("Disconnected.hdl").unwrap();
        let mut scanner = Scanner::new(contents.as_str(), provider.get_path("TwoAssign.hdl"));
        let mut parser = Parser {
            scanner: &mut scanner,
        };
        let hdl = parser.parse().expect("Parse error");
        let chip = Chip::new(&hdl, ptr::null_mut(), &provider, true, &Vec::new());
        assert!(chip.is_err());
    }
}
