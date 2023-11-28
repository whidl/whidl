// This module is responsible for taking a parsed Chip as input and
// producing equivalent VHDL code.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write;
use std::fs;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write as OtherWrite;
use std::ops::Deref;
use std::path::PathBuf;
use std::ptr;
use std::rc::Rc;

use crate::expr::{eval_expr, GenericWidth, Op, Terminal};
use crate::opt::optimization::OptimizationInfo::{self};
use crate::opt::optimization::OptimizationPass;
use crate::opt::portmap_dedupe::PortMapDedupe;
use crate::opt::sequential::SequentialPass;
use crate::parser::*;
use crate::simulator::infer_widths;
use crate::simulator::Chip;
use crate::Scanner;

// ========= STRUCTS ========== //
pub struct VhdlEntity {
    pub name: String,               // The name of this chip.
    pub generics: Vec<String>,      // Declared generics.
    pub ports: Vec<VhdlPort>,       // Declared ports.
    pub signals: Vec<Signal>,       // Declared signals.
    pub statements: Vec<Statement>, // VHDL statements.
    pub optimization_info: Option<Rc<RefCell<OptimizationInfo>>>,
    pub chip_hdl: ChipHDL,
    pub hdl_provider: Rc<dyn HdlProvider>,
}

impl Hash for VhdlEntity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
impl PartialEq for VhdlEntity {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for VhdlEntity {}

pub struct IdStatement {
    pub id: usize,
    pub stmt: Statement,
}

#[derive(Clone)]
//#[allow(clippy::large_enum_variant)]
pub enum Statement {
    Component(VhdlComponent),
    Process(Process),
    Assignment(AssignmentVHDL),
    Assert(AssertVHDL),
    Wait(WaitVHDL),
}

#[derive(Clone)]
pub struct AssertVHDL {
    pub signal_name: String,
    pub signal_value: LiteralVHDL,
    pub report_msg: String,
}

#[derive(Clone)]
pub struct WaitVHDL {}

#[derive(Clone)]
pub enum SignalRhs {
    Slice(SliceVHDL),
    Literal(LiteralVHDL),
}

#[derive(Clone)]
/// Designates two wire names. The signal from the right wire will be assigned to the left.
pub struct AssignmentVHDL {
    pub left: SliceVHDL,
    pub right: SignalRhs,
}

#[derive(Clone)]
pub struct LiteralVHDL {
    pub values: Vec<bool>,
}

#[derive(Clone)]
pub struct Process {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct Signal {
    pub name: String,
    pub width: GenericWidth,
}

/// Abstract VHDL component.
/// unit generic map (...) port map (...)
#[derive(Clone)]
pub struct VhdlComponent {
    pub unit: String,
    pub generic_params: Vec<GenericWidth>,
    pub port_mappings: Vec<PortMappingVHDL>,
}

/// VHDL example: foo(3 downto 0) or bar(X downto 0)
#[derive(Clone)]
pub struct SliceVHDL {
    /// The name of the signal. This is foo or bar in the example above.
    pub name: String,

    /// The start of the slice (inclusive). This will be None for signals without indices.
    pub start: Option<GenericWidth>,

    /// The end of the slice (exclusive). This will be None for signals without indices.
    pub end: Option<GenericWidth>,
}

#[derive(Clone)]
pub struct PortMappingVHDL {
    pub wire_name: String,
    pub port: SliceVHDL,
    pub wire: SignalRhs,
}

pub struct QuartusProject {
    pub chip_hdl: ChipHDL,
    pub chip_vhdl: VhdlEntity,
    pub project_dir: PathBuf,
}

pub struct VhdlPort {
    pub name: String,
    pub width: GenericWidth,
    pub direction: PortDirection,
}

// ========= TRAITS ========== //

/// A Pet is an object that we track by name.
pub trait Pet<'a> {
    fn name(&self) -> &'a str;
}

/// A container for pets.
pub trait Cage<'a> {
    fn contains_name(self, name: &str) -> bool
    where
        Self: IntoIterator + std::marker::Sized,
        <Self as IntoIterator>::Item: Pet<'a>,
    {
        self.into_iter().any(|pet| pet.name() == name)
    }
}

impl<'a> Pet<'a> for &'a VhdlPort {
    fn name(&self) -> &'a str {
        &self.name
    }
}

impl<'a> Cage<'a> for &Vec<VhdlPort> {}

// ========= DISPLAY ========== //
//
// The fmt::Display trait is used to convert VHDL abstract syntax nodes
// to VHDL concrete syntax.

impl fmt::Display for IdStatement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.stmt {
            Statement::Component(x) => write!(f, "cn2v{}: {}", self.id, x),
            Statement::Process(x) => write!(f, "cn2v{}: {}", self.id, x),
            Statement::Assignment(x) => write!(f, "{}", x),
            Statement::Assert(x) => write!(f, "cn2v{}: {}", self.id, x),
            Statement::Wait(_) => write!(f, "wait for 10 ns;"),
        }
    }
}

impl fmt::Display for AssertVHDL {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "assert {} = {} report \"{}\";",
            keyw(&self.signal_name),
            self.signal_value,
            self.report_msg
        )
    }
}

impl fmt::Display for AssignmentVHDL {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} <= {};", self.left, self.right)
    }
}

impl fmt::Display for WaitVHDL {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "wait for 10 ns;")
    }
}

impl fmt::Display for LiteralVHDL {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"")?;

        for &x in self.values.iter().rev() {
            if x {
                write!(f, "1")?;
            } else {
                write!(f, "0")?;
            }
        }

        write!(f, "\"")
    }
}

impl fmt::Display for SignalRhs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Slice(x) => write!(f, "{}", x),
            Self::Literal(x) => write!(f, "{}", x),
        }
    }
}

impl fmt::Display for VhdlEntity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "library ieee;")?;
        writeln!(f, "use ieee.std_logic_1164.all;")?;
        writeln!(f)?;

        // Final VHDL generated for the top-level chip.
        writeln!(f, "entity {} is", keyw(&self.name))?;

        for x in &self.generics {
            writeln!(f, "{}", x)?;
        }

        let port_vec: Vec<String> = self.ports.iter().map(|x| keyw(&x.to_string())).collect();
        if !port_vec.is_empty() {
            writeln!(f, "port (")?;
            writeln!(f, "{}", port_vec.join(";\n"))?;
            writeln!(f, ");")?;
        }

        writeln!(f, "end entity {};", keyw(&self.name))?;
        writeln!(f)?;

        writeln!(f, "architecture arch of {} is", keyw(&self.name))?;

        // We need to iterate over HDL parts in order to generate declarations for them.
        let mut seen = HashSet::new();
        for part in &self.chip_hdl.parts {
            match part {
                Part::Component(component) => {
                    if seen.insert(&component.name.value) {
                        // If it's a Component, we generate its declaration
                        let decl = self.declaration(component, Rc::clone(&self.hdl_provider))?;
                        writeln!(f, "{}", decl)?;
                    }
                }
                Part::Loop(loop_hdl) => {
                    for component in &loop_hdl.body {
                        if seen.insert(&component.name.value) {
                            let decl =
                                self.declaration(component, Rc::clone(&self.hdl_provider))?;
                            writeln!(f, "{}", decl)?;
                        }
                    }
                }
                Part::AssignmentHDL(_) => {
                    // Do nothing for AssignmentHDL
                }
            }
        }

        for x in &self.signals {
            writeln!(f, "signal {}", x)?;
        }

        writeln!(f, "begin")?;
        for (i, x) in self.statements.iter().enumerate() {
            let id_stmt = IdStatement {
                id: i,
                stmt: x.clone(),
            };
            writeln!(f, "{}", id_stmt)?;
        }

        writeln!(f, "end arch;")?;
        write!(f, "")
    }
}

// Declaration VHDL for an entity.
impl VhdlEntity {
    fn declaration(
        &self,
        dep: &Component,
        provider: Rc<dyn HdlProvider>,
    ) -> Result<String, std::fmt::Error> {
        let mut decl = String::new();

        // Just parse it again to get the chip_hdl.
        // It's not ideal, but it's the easiest way to get the chip_hdl for now.
        let chip_hdl = get_hdl(&dep.name.value, &provider).unwrap();

        writeln!(decl, "component {} is", keyw(&dep.name.value))?;
        writeln!(decl, "port (")?;

        match &self.optimization_info {
            Some(info) => match RefCell::borrow(info).deref() {
                OptimizationInfo::SequentialFlagMap(seq_flag_map) => {
                    if seq_flag_map.get(&dep.name.value) == Some(&true) {
                        writeln!(decl, "clk : in std_logic_vector(0 downto 0);")?;
                    }
                }
                OptimizationInfo::None => unimplemented!(),
            },
            None => (),
        }

        let vhdl_ports: Vec<String> = chip_hdl
            .ports
            .iter()
            .map(|port| VhdlPort::from(port).to_string())
            .collect();
        writeln!(decl, "{}", vhdl_ports.join(";\n"))?;

        writeln!(decl, ");")?;
        writeln!(decl, "end component {};", keyw(&dep.name.value))?;

        Ok(decl)
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} : ", keyw(&self.name))?;
        match self.width {
            GenericWidth::Terminal(Terminal::Num(port_width_num)) => {
                write!(f, "std_logic_vector({} downto 0);", port_width_num - 1)
            }
            _ => {
                let sub1 = GenericWidth::Expr(
                    Op::Sub,
                    Box::new(self.width.clone()),
                    Box::new(GenericWidth::Terminal(Terminal::Num(1))),
                );
                write!(
                    f,
                    "std_logic_vector({} downto 0);",
                    eval_expr(&sub1, &HashMap::new())
                )
            }
        }
    }
}

impl fmt::Display for VhdlComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} port map(", keyw(&self.unit))?;

        // At this point the VHDL AST should be valid, so here
        // we don't need to handle the case where the same output port
        // is used for multiple signals.
        for (i, mapping) in self.port_mappings.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", mapping)?;
        }
        writeln!(f, ");")
    }
}

impl fmt::Display for Process {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "process begin")?;

        for (i, x) in self.statements.iter().enumerate() {
            let id_stmt = IdStatement {
                id: i,
                stmt: x.clone(),
            };
            writeln!(f, "{}", id_stmt)?;
        }

        writeln!(f, "end process;")
    }
}

/// Synthesizes VHDL for BusVHDL.
impl std::fmt::Display for SliceVHDL {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name == "true" {
            return write!(f, "(others => '1')");
        } else if self.name == "false" {
            return write!(f, "(others => '0')");
        }

        // Only write out downto syntax if this is an array.
        if self.start.is_some() {
            let start: &GenericWidth = self.start.as_ref().unwrap();
            let end: &GenericWidth = self.end.as_ref().unwrap();
            write!(f, "{}({} downto {})", keyw(&self.name), end, start)
        } else {
            write!(f, "{}", keyw(&self.name))
        }
    }
}

impl fmt::Display for VhdlPort {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} : ", keyw(&self.name))?;

        if self.direction == PortDirection::In {
            write!(f, "in ")?;
        } else {
            write!(f, "out ")?;
        }

        match self.width {
            GenericWidth::Terminal(Terminal::Num(port_width_num)) => {
                write!(f, "std_logic_vector({} downto 0)", port_width_num - 1)?;
            }
            _ => {
                let sub1 = GenericWidth::Expr(
                    Op::Sub,
                    Box::new(self.width.clone()),
                    Box::new(GenericWidth::Terminal(Terminal::Num(1))),
                );
                write!(
                    f,
                    "std_logic_vector({} downto 0)",
                    eval_expr(&sub1, &HashMap::new())
                )?;
            }
        };

        write!(f, "")
    }
}

impl std::fmt::Display for PortMappingVHDL {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} => {}", &self.port, &self.wire)
    }
}

// ========= CONVERSIONS ========== //

/// This is where VHDL is synthesized for an HDL chip.
impl TryFrom<&Chip> for VhdlEntity {
    type Error = Box<dyn Error>;

    fn try_from(chip: &Chip) -> Result<Self, Box<dyn Error>> {
        let mut vhdl_components: Vec<VhdlComponent> =
            chip.components.iter().map(VhdlComponent::from).collect();

        let mut ports: Vec<VhdlPort> = Vec::new();
        let chip_hdl = chip.hdl.as_ref().unwrap();

        for port in &chip_hdl.ports {
            ports.push(VhdlPort::from(port));
        }

        // Create a clock port if this is a sequential chip.
        let mut sequential_pass = SequentialPass::new();
        let (_, sequential_pass_info_raw) = sequential_pass.apply(chip_hdl, &chip_hdl.provider)?;
        let sequential_pass_info = Rc::new(RefCell::new(sequential_pass_info_raw));

        if let OptimizationInfo::SequentialFlagMap(sequential_flag_map) =
            &*sequential_pass_info.borrow()
        {
            if sequential_flag_map.get(&chip_hdl.name) == Some(&true) {
                let clock_port = VhdlPort {
                    name: "clk".to_string(),
                    width: GenericWidth::Terminal(Terminal::Num(1)),
                    direction: PortDirection::In,
                };

                ports.push(clock_port);
            }
        }

        let mut generic_params = Vec::new();
        let mut generic_decls = Vec::new();
        for decl in &chip.hdl.as_ref().unwrap().generic_decls {
            let param = chip.variables.get(&decl.value).unwrap();
            generic_params.push(GenericWidth::Terminal(Terminal::Num(*param)));
            generic_decls.push(decl.value.clone());
        }

        // At this point we need to know the values of the generics.
        // where this was instantiated.
        let inferred_widths = infer_widths(
            chip_hdl,
            &Vec::new(),
            &chip.components,
            &chip_hdl.provider,
            &generic_params,
        )?;

        let ports_ref = &ports;

        let signals = inferred_widths
            .iter()
            .filter(|(signal_name, _)| !ports_ref.contains_name(signal_name))
            .map(|(signal_name, signal_width)| Signal {
                name: signal_name.clone(),
                width: signal_width.clone(),
            })
            .collect();

        let mut statements = Vec::new();
        for c in &mut vhdl_components {
            // If this is a sequential chip, we need to add a clock port mapping.
            if let OptimizationInfo::SequentialFlagMap(sequential_flag_map) =
                &*sequential_pass_info.borrow()
            {
                if sequential_flag_map.get(&c.unit) == Some(&true) {
                    let clock_port_mapping = PortMappingVHDL {
                        wire_name: "clk".to_string(),
                        port: SliceVHDL {
                            name: "clk".to_string(),
                            start: None,
                            end: None,
                        },
                        wire: SignalRhs::Slice(SliceVHDL {
                            name: "clk".to_string(),
                            start: None,
                            end: None,
                        }),
                    };

                    c.port_mappings.push(clock_port_mapping);
                }
            }

            statements.push(Statement::Component(c.clone()));
        }

        // Synthesize assignments. Assignments were added after components
        // and are handled a little differently because they don't instantiate
        // a chip.
        for assignment in &chip_hdl.parts {
            if let Part::AssignmentHDL(assignment) = assignment {
                let assignment = AssignmentVHDL::from(assignment);
                statements.push(Statement::Assignment(assignment));
            }
        }

        Ok(VhdlEntity {
            name: chip_hdl.name.clone(),
            generics: generic_decls,
            ports,
            signals,
            statements,
            optimization_info: Some(Rc::clone(&sequential_pass_info)),
            chip_hdl: chip_hdl.clone(),
            hdl_provider: chip_hdl.provider.clone(),
        })
    }
}

fn group_port_mappings(component: &Component) -> HashMap<String, Vec<&PortMappingHDL>> {
    // port_mappings is a HashMap where each key is the port name, and each
    // value is a vector of all the PortMappingHDL instances where that port
    // is mapped.
    let mut grouped_port_mappings: HashMap<String, Vec<&PortMappingHDL>> = HashMap::new();
    for port_mapping in &component.mappings {
        grouped_port_mappings
            .entry(port_mapping.port.name.clone())
            .or_default()
            .push(port_mapping);
    }
    grouped_port_mappings
}

/// Transforms a `&Component` into a `VhdlComponent.
///
/// In HDL, multiple signals can be mapped to a single output port, which is not
/// supported in VHDL. This function creates an intermediate signals when
/// necessary.
///
/// # Parameters
///
/// - `component`: The `Component` instance to transform.
///
/// # Returns
///
/// A `VhdlComponent` instance that represents the transformed component.
impl From<&Component> for VhdlComponent {
    fn from(component: &Component) -> Self {
        let port_mappings = group_port_mappings(component);
        let mut vhdl_port_mappings = Vec::new();
        for (port_name, mappings) in port_mappings {
            if mappings.len() == 1 {
                // If there's only one mapping, no intermediate signal is necessary
                vhdl_port_mappings.push(PortMappingVHDL::from(mappings[0]));
            } else {
                // If there's more than one mapping, create an intermediate signal for each
                for (i, mapping) in mappings.iter().enumerate() {
                    let intermediate_signal_name = format!("{}_{}", port_name, i);
                    let wire = SliceVHDL {
                        name: intermediate_signal_name.clone(),
                        start: mapping.wire.start.clone(),
                        end: mapping.wire.end.clone(),
                    };

                    vhdl_port_mappings.push(PortMappingVHDL {
                        wire_name: intermediate_signal_name,
                        port: SliceVHDL::from(&mapping.port),
                        wire: SignalRhs::Slice(wire),
                    });
                }
            }
        }

        VhdlComponent {
            unit: component.name.value.clone(),
            generic_params: component.generic_params.clone(),
            port_mappings: vhdl_port_mappings,
        }
    }
}

impl From<&BusHDL> for SliceVHDL {
    fn from(hdl: &BusHDL) -> Self {
        SliceVHDL {
            name: hdl.name.clone(),
            start: hdl.start.clone(),
            end: hdl.end.clone(),
        }
    }
}

impl From<&SignalRhs> for BusHDL {
    fn from(vhdl: &SignalRhs) -> Self {
        match vhdl {
            SignalRhs::Slice(slice) => BusHDL {
                name: slice.name.clone(),
                start: slice.start.clone(),
                end: slice.end.clone(),
            },
            SignalRhs::Literal(_l) => {
                panic!("Not yet implemented.");
            }
        }
    }
}

impl From<&GenericPort> for VhdlPort {
    fn from(port: &GenericPort) -> Self {
        VhdlPort {
            name: port.name.value.clone(),
            width: port.width.clone(),
            direction: port.direction,
        }
    }
}

impl From<&PortMappingHDL> for PortMappingVHDL {
    fn from(pm: &PortMappingHDL) -> Self {
        let wire = SignalRhs::Slice(SliceVHDL::from(&pm.wire));

        PortMappingVHDL {
            wire_name: pm.wire.name.clone(),
            port: SliceVHDL::from(&pm.port),
            wire,
        }
    }
}

impl From<&AssignmentHDL> for AssignmentVHDL {
    fn from(assignment: &AssignmentHDL) -> Self {
        AssignmentVHDL {
            left: SliceVHDL::from(&assignment.left),
            right: SignalRhs::Slice(SliceVHDL::from(&assignment.right)),
        }
    }
}

impl QuartusProject {
    pub fn new(chip_hdl: ChipHDL, chip_vhdl: VhdlEntity, project_dir: PathBuf) -> Self {
        QuartusProject {
            chip_hdl,
            chip_vhdl,
            project_dir,
        }
    }
}

pub fn write_quartus_project(qp: &QuartusProject) -> Result<(), Box<dyn Error>> {
    println!("Writing Quartus Prime project to {}", qp.project_dir.display());
    let mut tcl = format!("project_new {} -overwrite", &qp.chip_vhdl.name);

    writeln!(
        tcl,
        "set_global_assignment -name TOP_LEVEL_ENTITY {}",
        keyw(&qp.chip_vhdl.name)
    )?;

    writeln!(tcl, "set_global_assignment -name VHDL_FILE NAND.vhdl")?;
    let chip_filename = qp.chip_vhdl.name.clone() + ".vhdl";
    writeln!(
        tcl,
        "set_global_assignment -name VHDL_FILE {}",
        chip_filename
    )?;

    // Run the sequential pass on chip HDL
    let mut sequential_pass = SequentialPass::new();
    let (_, sequential_pass_info_raw) =
        sequential_pass.apply(&qp.chip_hdl, &qp.chip_hdl.provider)?;
    let sequential_pass_info = Rc::new(RefCell::new(sequential_pass_info_raw));

    if let OptimizationInfo::SequentialFlagMap(sequential_flag_map) =
        &*sequential_pass_info.borrow()
    {
        for name in sequential_flag_map.keys() {
            writeln!(tcl, "set_global_assignment -name VHDL_FILE {}.vhdl", name)?;
        }
    }

    // Read in templates/nand_template.vhdl to the string nand_vhdl.
    // This is the definition of the NAND gate.
    let nand_vhdl = "
    library ieee;
    use ieee.std_logic_1164.all;
    entity nand_n2v is
    port (a : in std_logic_vector(0 downto 0);
    b : in std_logic_vector(0 downto 0);
    out_n2v : out std_logic_vector(0 downto 0)
    );
    end entity nand_n2v;
    architecture arch of nand_n2v is
    begin
    out_n2v <= a nand b;
    end architecture arch;
    ";

    let mut file = File::create(qp.project_dir.join("Nand.vhdl"))?;
    file.write_all(nand_vhdl.as_bytes())?;

    writeln!(
        tcl,
        "set_global_assignment -name TOP_LEVEL_ENTITY {}",
        keyw(&qp.chip_vhdl.name)
    )?;

    writeln!(tcl, "set_global_assignment -name VHDL_FILE NAND.vhdl")?;
    let chip_filename = qp.chip_vhdl.name.clone() + ".vhdl";
    writeln!(
        tcl,
        "set_global_assignment -name VHDL_FILE {}",
        chip_filename
    )?;

    // Run the sequential pass on chip HDL
    let mut sequential_pass = SequentialPass::new();
    let (_, sequential_pass_info_raw) =
        sequential_pass.apply(&qp.chip_hdl, &qp.chip_hdl.provider)?;
    let sequential_pass_info = Rc::new(RefCell::new(sequential_pass_info_raw));

    if let OptimizationInfo::SequentialFlagMap(sequential_flag_map) =
        &*sequential_pass_info.borrow()
    {
        for name in sequential_flag_map.keys() {
            writeln!(tcl, "set_global_assignment -name VHDL_FILE {}.vhdl", name)?;
        }
    }

    writeln!(
        tcl,
        "set_global_assignment -name TOP_LEVEL_ENTITY {}",
        keyw(&qp.chip_vhdl.name)
    )?;

    writeln!(tcl, "set_global_assignment -name VHDL_FILE NAND.vhdl")?;
    let chip_filename = qp.chip_vhdl.name.clone() + ".vhdl";
    writeln!(
        tcl,
        "set_global_assignment -name VHDL_FILE {}",
        chip_filename
    )?;

    // Run the sequential pass on chip HDL
    let mut sequential_pass = SequentialPass::new();
    let (_, sequential_pass_info_raw) =
        sequential_pass.apply(&qp.chip_hdl, &qp.chip_hdl.provider)?;
    let sequential_pass_info = Rc::new(RefCell::new(sequential_pass_info_raw));

    if let OptimizationInfo::SequentialFlagMap(sequential_flag_map) =
        &*sequential_pass_info.borrow()
    {
        for name in sequential_flag_map.keys() {
            writeln!(tcl, "set_global_assignment -name VHDL_FILE {}.vhdl", name)?;
        }
    }

    let mut file = File::create(qp.project_dir.join("Nand.vhdl"))?;
    file.write_all(nand_vhdl.as_bytes())?;

    let dff_vhdl = "
    library ieee;
    use ieee.std_logic_1164.all;
    entity nand_n2v is
    port (a : in std_logic_vector(0 downto 0);
    b : in std_logic_vector(0 downto 0);
    out_n2v : out std_logic_vector(0 downto 0)
    );
    end entity nand_n2v;
    architecture arch of nand_n2v is
    begin
    out_n2v <= a nand b;
    end architecture arch;
    ";

    let mut file = File::create(qp.project_dir.join("DFF.vhdl"))?;
    file.write_all(dff_vhdl.as_bytes())?;

    tcl.push_str("project_close");
    let mut file = File::create(qp.project_dir.join("project.tcl"))?;
    file.write_all(tcl.as_bytes())?;

    // Write the already-parsed main chip.
    let chip_filename = qp.chip_vhdl.name.clone() + ".vhdl";
    let mut file = File::create(qp.project_dir.join(&chip_filename))?;
    file.write_all(format!("{}", qp.chip_vhdl).as_bytes())?;

    // The chip names we have already processed. We only need to
    // convert each chip type once.
    let mut done: HashSet<String> = HashSet::new();
    done.insert(String::from("Nand"));
    done.insert(String::from("DFF"));

    // Recursively parse and write all dependency components.
    // Worklist is set of chip names that we need to convert from HDL to VHDL.
    let mut worklist: Vec<&Chip> = Vec::new();

    // TODO: Instead of using the worklist we should just make one chip
    // and then traverse the chip. We are reinventing the wheel here
    // that is Simulator::elaborate
    let base_path = qp.chip_hdl.path.as_ref().unwrap().parent().unwrap();

    let mut dedupe_pass = PortMapDedupe::new();
    let (chip_hdl, _) = &dedupe_pass.apply(&qp.chip_hdl, &qp.chip_hdl.provider)?;

    let top_level_chip = Chip::new(
        chip_hdl,
        ptr::null_mut(),
        &chip_hdl.provider,
        true,
        &Vec::new(),
    )?;

    worklist.push(&top_level_chip);

    while (!worklist.is_empty()) {
        let chip = worklist.pop().unwrap();

        // If we have already processed this chip, skip it.
        if done.contains(&chip.name) {
            continue;
        }

        // Otherwise, add it to the set of chips we have processed.
        done.insert(chip.name.clone());

        // Add all the components in this chip to the worklist.
        for chip_idx in chip.circuit.node_indices() {
            let component_chip = &chip.circuit[chip_idx];
            if &component_chip.name == "Nand" || &component_chip.name == "DFF" {
                continue;
            }
            if &component_chip.name == "true" || &component_chip.name == "false" {
                continue;
            }
            if chip_hdl.get_port(&component_chip.name).is_err(){
                println!("Adding {} to worklist", component_chip.name);
                worklist.push(component_chip);
            }
        }

        // Create a VHDL entity for this chip.
        let chip_vhdl: VhdlEntity = VhdlEntity::try_from(chip)?;

        // Write the VHDL entity to a file.
        let chip_filename = chip_vhdl.name.clone() + ".vhdl";
        let mut file = File::create(qp.project_dir.join(&chip_filename))?;
        file.write_all(format!("{}", chip_vhdl).as_bytes())?;
    }

    Ok(())
}

// VHDL keywords that we can't use.
pub fn keyw(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "in" => String::from("in_n2v"),
        "out" => String::from("out_n2v"),
        "not" => String::from("not_n2v"),
        "nand" => String::from("nand_n2v"),
        "and" => String::from("and_n2v"),
        "or" => String::from("or_n2v"),
        "xor" => String::from("xor_n2v"),
        "nor" => String::from("nor_n2v"),
        "dff" => String::from("DFF_n2v"),
        "register" => String::from("register_n2v"),
        _ => String::from(name),
    }
}
