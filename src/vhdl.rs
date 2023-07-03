// This module is responsible for taking a parsed Chip as input and
// producing equivalent VHDL code.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write;
use std::fs;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write as OtherWrite;
use std::path::PathBuf;
use std::ptr;
use std::rc::Rc;

use crate::expr::{eval_expr, GenericWidth, Op, Terminal};
use crate::opt::optimization::OptimizationInfo::SequentialFlagMap;
use crate::opt::optimization::OptimizationPass;
use crate::opt::portmap_dedupe::PortMapDedupe;
use crate::opt::sequential::SequentialPass;
use crate::parser::*;
use crate::simulator::infer_widths;
use crate::simulator::Chip;
use crate::Scanner;

// ========= STRUCTS ========== //
pub struct VhdlEntity {
    pub name: String,                      // The name of this chip.
    pub generics: Vec<String>,             // Declared generics.
    pub ports: Vec<VhdlPort>,              // Declared ports.
    pub signals: Vec<Signal>,              // Declared signals.
    pub statements: Vec<Statement>,        // VHDL statements.
    pub dependencies: HashSet<VhdlEntity>, // Entities for components.
    pub chip: Chip,
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
            Statement::Wait(x) => write!(f, "cn2v{}: {}", self.id, x),
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
        for x in &self.dependencies {
            writeln!(f, "{}", x.declaration()?)?;
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
    fn declaration(&self) -> Result<String, std::fmt::Error> {
        let mut decl = String::new();

        writeln!(decl, "component {} is", keyw(&self.name))?;
        writeln!(decl, "port (")?;
        let port_vec: Vec<String> = self.ports.iter().map(|x| keyw(&x.to_string())).collect();
        writeln!(decl, "{}", port_vec.join(";\n"))?;
        writeln!(decl, ");")?;
        writeln!(decl, "end component {};", keyw(&self.name))?;

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
impl TryFrom<&ChipHDL> for VhdlEntity {
    type Error = Box<dyn Error>;

    fn try_from(raw_hdl: &ChipHDL) -> Result<Self, Box<dyn Error>> {
        let mut dedupe_pass = PortMapDedupe::new();
        let (chip_hdl, _) = &dedupe_pass.apply(raw_hdl, &raw_hdl.provider)?;

        let chip = Chip::new(
            chip_hdl,
            ptr::null_mut(),
            &chip_hdl.provider,
            true,
            &Vec::new(),
        )?;

        // Declare components
        let vhdl_components: Vec<VhdlComponent> =
            chip.components.iter().map(VhdlComponent::from).collect();
        let generics: Vec<String> = Vec::new();
        let mut ports: Vec<VhdlPort> = Vec::new();

        for port in &chip_hdl.ports {
            ports.push(VhdlPort::from(port));
        }

        // Create a clock port if this is a sequential chip.
        let mut sequential_pass = SequentialPass::new();
        if let (_, SequentialFlagMap(sequential_flag_map)) =
            sequential_pass.apply(chip_hdl, &chip_hdl.provider)?
        {
            if sequential_flag_map.get(&chip_hdl.name) == Some(&true) {
                let clock_port = VhdlPort {
                    name: "clk".to_string(),
                    width: GenericWidth::Terminal(Terminal::Num(1)),
                    direction: PortDirection::In,
                };

                ports.push(clock_port);
            }
        } else {
            panic!();
        }

        let inferred_widths = infer_widths(
            chip_hdl,
            &Vec::new(),
            &chip.components,
            &chip_hdl.provider,
            &Vec::new(),
        )?;

        let dependencies = get_all_dependencies(chip_hdl, &chip_hdl.provider)?;

        let ports_ref = &ports;

        let signals = inferred_widths
            .iter()
            .filter(|(signal_name, _)| !ports_ref.contains_name(signal_name))
            .map(|(signal_name, signal_width)| Signal {
                name: signal_name.clone(),
                width: signal_width.clone(),
            })
            .collect();
        let mut statements: Vec<Statement> = vhdl_components
            .into_iter()
            .map(Statement::Component)
            .collect();

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
            generics,
            ports,
            signals,
            statements,
            dependencies,
            chip,
        })
    }
}

pub fn get_all_dependencies(
    chip: &ChipHDL,
    provider: &Rc<dyn HdlProvider>,
) -> Result<HashSet<VhdlEntity>, Box<dyn Error>> {
    let mut dependencies = HashSet::new();

    for part in &chip.parts {
        match part {
            Part::Component(component) => {
                let component_hdl = get_hdl(&component.name.value, provider)?;
                let current_entity = VhdlEntity::try_from(&component_hdl)?;
                dependencies.insert(current_entity);

                let sub_dependencies = get_all_dependencies(&component_hdl, provider)?;
                dependencies.extend(sub_dependencies);
            }
            Part::Loop(loop_hdl) => {
                for subpart in &loop_hdl.body {
                    let component_hdl = get_hdl(&subpart.name.value, provider)?;
                    let sub_dependencies = get_all_dependencies(&component_hdl, provider)?;
                    dependencies.extend(sub_dependencies);
                }
            }
            _ => {}
        }
    }

    Ok(dependencies)
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
    let mut tcl = format!("project_new {} -overwrite", &qp.chip_vhdl.name);

    tcl.push_str(&String::from(
        r#"
        # Assign family, device, and top-level file
        set_global_assignment -name FAMILY "Cyclone V"
        set_global_assignment -name DEVICE 5CSEMA5F31C6
        #============================================================
        # LEDR
        #============================================================
        set_location_assignment PIN_V16 -to LEDR[0]
        set_location_assignment PIN_W16 -to LEDR[1]
        set_location_assignment PIN_V17 -to LEDR[2]
        set_location_assignment PIN_V18 -to LEDR[3]
        set_location_assignment PIN_W17 -to LEDR[4]
        set_location_assignment PIN_W19 -to LEDR[5]
        set_location_assignment PIN_Y19 -to LEDR[6]
        set_location_assignment PIN_W20 -to LEDR[7]
        set_location_assignment PIN_W21 -to LEDR[8]
        set_location_assignment PIN_Y21 -to LEDR[9]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[0]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[1]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[2]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[3]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[4]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[5]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[6]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[7]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[8]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[9]
        #============================================================
        # SW
        #============================================================
        set_location_assignment PIN_AB12 -to SW[0]
        set_location_assignment PIN_AC12 -to SW[1]
        set_location_assignment PIN_AF9 -to SW[2]
        set_location_assignment PIN_AF10 -to SW[3]
        set_location_assignment PIN_AD11 -to SW[4]
        set_location_assignment PIN_AD12 -to SW[5]
        set_location_assignment PIN_AE11 -to SW[6]
        set_location_assignment PIN_AC9 -to SW[7]
        set_location_assignment PIN_AD10 -to SW[8]
        set_location_assignment PIN_AE12 -to SW[9]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[0]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[1]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[2]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[3]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[4]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[5]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[6]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[7]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[8]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[9]
        #============================================================
        # HEX0
        #============================================================
        set_location_assignment PIN_AE26 -to HEX0[0]
        set_location_assignment PIN_AE27 -to HEX0[1]
        set_location_assignment PIN_AE28 -to HEX0[2]
        set_location_assignment PIN_AG27 -to HEX0[3]
        set_location_assignment PIN_AF28 -to HEX0[4]
        set_location_assignment PIN_AG28 -to HEX0[5]
        set_location_assignment PIN_AH28 -to HEX0[6]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[0]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[1]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[2]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[3]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[4]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[5]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[6]
        #============================================================
        # HEX1
        #============================================================
        set_location_assignment PIN_AJ29 -to HEX1[0]
        set_location_assignment PIN_AH29 -to HEX1[1]
        set_location_assignment PIN_AH30 -to HEX1[2]
        set_location_assignment PIN_AG30 -to HEX1[3]
        set_location_assignment PIN_AF29 -to HEX1[4]
        set_location_assignment PIN_AF30 -to HEX1[5]
        set_location_assignment PIN_AD27 -to HEX1[6]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[0]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[1]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[2]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[3]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[4]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[5]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[6]
        #============================================================
        # HEX2
        #============================================================
        set_location_assignment PIN_AB23 -to HEX2[0]
        set_location_assignment PIN_AE29 -to HEX2[1]
        set_location_assignment PIN_AD29 -to HEX2[2]
        set_location_assignment PIN_AC28 -to HEX2[3]
        set_location_assignment PIN_AD30 -to HEX2[4]
        set_location_assignment PIN_AC29 -to HEX2[5]
        set_location_assignment PIN_AC30 -to HEX2[6]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[0]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[1]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[2]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[3]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[4]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[5]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[6]
        #============================================================
        # HEX3
        #============================================================
        set_location_assignment PIN_AD26 -to HEX3[0]
        set_location_assignment PIN_AC27 -to HEX3[1]
        set_location_assignment PIN_AD25 -to HEX3[2]
        set_location_assignment PIN_AC25 -to HEX3[3]
        set_location_assignment PIN_AB28 -to HEX3[4]
        set_location_assignment PIN_AB25 -to HEX3[5]
        set_location_assignment PIN_AB22 -to HEX3[6]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[0]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[1]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[2]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[3]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[4]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[5]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[6]
        #============================================================
        # HEX4
        #============================================================
        set_location_assignment PIN_AA24 -to HEX4[0]
        set_location_assignment PIN_Y23 -to HEX4[1]
        set_location_assignment PIN_Y24 -to HEX4[2]
        set_location_assignment PIN_W22 -to HEX4[3]
        set_location_assignment PIN_W24 -to HEX4[4]
        set_location_assignment PIN_V23 -to HEX4[5]
        set_location_assignment PIN_W25 -to HEX4[6]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[0]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[1]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[2]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[3]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[4]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[5]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[6]
        #============================================================
        # HEX5
        #============================================================
        set_location_assignment PIN_V25 -to HEX5[0]
        set_location_assignment PIN_AA28 -to HEX5[1]
        set_location_assignment PIN_Y27 -to HEX5[2]
        set_location_assignment PIN_AB27 -to HEX5[3]
        set_location_assignment PIN_AB26 -to HEX5[4]
        set_location_assignment PIN_AA26 -to HEX5[5]
        set_location_assignment PIN_AA25 -to HEX5[6]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[0]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[1]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[2]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[3]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[4]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[5]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[6]
        #============================================================
        # KEY
        #============================================================
        set_location_assignment PIN_AA14 -to KEY[0]
        set_location_assignment PIN_AA15 -to KEY[1]
        set_location_assignment PIN_W15 -to KEY[2]
        set_location_assignment PIN_Y16 -to KEY[3]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to KEY[0]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to KEY[1]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to KEY[2]
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to KEY[3]
        #============================================================
        # CLOCK
        #============================================================
        set_location_assignment PIN_AF14 -to CLOCK_50
        set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to CLOCK_50

        # Device and Pin options
        set_global_assignment -name RESERVE_ALL_UNUSED_PINS_WEAK_PULLUP "AS INPUT TRI-STATED"
    "#,
    ));

    writeln!(
        tcl,
        "set_global_assignment -name TOP_LEVEL_ENTITY {}",
        keyw(&qp.chip_vhdl.name)
    )?;

    let chip_filename = qp.chip_vhdl.name.clone() + ".vhdl";
    writeln!(
        tcl,
        "set_global_assignment -name VHDL_FILE {}",
        chip_filename
    )?;
    for dep in &qp.chip_vhdl.dependencies {
        writeln!(
            tcl,
            "set_global_assignment -name VHDL_FILE {}",
            dep.name.clone() + ".vhdl"
        )?;
    }

    let nand_vhdl = r#"
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
"#;
    let mut file = File::create(qp.project_dir.join("NAND.vhdl"))?;
    file.write_all(nand_vhdl.as_bytes())?;

    let dff_vhdl = r#"
library ieee;
use ieee.std_logic_1164.all;
LIBRARY altera;
USE altera.altera_primitives_components.all;

entity DFF_n2v is
port (in_n2v : in std_logic_vector(0 downto 0);
out_n2v : out std_logic_vector(0 downto 0);
clk : in std_logic_vector(0 downto 0)
);
end entity DFF_n2v;

architecture arch of DFF_n2v is

COMPONENT DFF
   PORT (d   : IN STD_LOGIC;
        clk  : IN STD_LOGIC;
        clrn : IN STD_LOGIC;
        prn  : IN STD_LOGIC;
        q    : OUT STD_LOGIC );

END COMPONENT;

begin
x0: DFF port map (d => in_n2v(0), clrn => '1', prn => '1', q => out_n2v(0), clk => clk(0));
end architecture arch;
"#;

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
    let mut worklist: Vec<String> = Vec::new();

    // Pushes parts onto the worklist.
    fn push_parts(parts: &Vec<Part>, worklist: &mut Vec<String>, done: &mut HashSet<String>) {
        for part in parts {
            match part {
                Part::Component(c) => {
                    if !done.contains(&c.name.value) {
                        done.insert(c.name.value.clone());
                        worklist.push(c.name.value.clone());
                    }
                }
                Part::Loop(l) => {
                    for c in &l.body {
                        if !done.contains(&c.name.value) {
                            done.insert(c.name.value.clone());
                            worklist.push(c.name.value.clone());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Seed the worklist with all components of the top-level entity.
    push_parts(&qp.chip_hdl.parts, &mut worklist, &mut done);

    let base_path = qp.chip_hdl.path.as_ref().unwrap().parent().unwrap();

    while !worklist.is_empty() {
        let next_chip_name = worklist.pop().unwrap();
        let next_hdl_path = base_path.join(next_chip_name.clone() + ".hdl");

        let next_source_code = fs::read_to_string(&next_hdl_path)?;
        let mut next_scanner = Scanner::new(&next_source_code, next_hdl_path.clone());
        let mut next_parser = Parser::new(&mut next_scanner, qp.chip_hdl.provider.clone());
        let next_hdl = next_parser.parse()?;

        // Convert HDL to VHDL (VHDl synthesis).
        let next_vhdl: VhdlEntity = VhdlEntity::try_from(&next_hdl)?;

        let next_filename = next_chip_name + ".vhdl";
        let mut next_file = File::create(qp.project_dir.join(&next_filename))?;
        next_file.write_all(format!("{}", next_vhdl).as_bytes())?;

        push_parts(&next_hdl.parts, &mut worklist, &mut done);
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
