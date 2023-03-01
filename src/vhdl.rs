// This module is responsible for taking a parsed Chip as input and
// producing equivalent VHDL code.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write as OtherWrite;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

use crate::expr::{eval_expr, GenericWidth, Op, Terminal};
use crate::parser::*;
use crate::simulator::{gather_assignments, infer_widths, Bus};

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

impl<'a> Cage<'a> for &Vec<VhdlPort> {}

#[derive(Debug)]
pub struct Signal {
    pub name: String,
    pub width: GenericWidth,
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} : ", self.name)?;
        match self.width {
            GenericWidth::Terminal(Terminal::Num(port_width_num)) => {
                if port_width_num > 1 {
                    write!(f, "std_logic_vector({} downto 0);", port_width_num - 1)
                } else {
                    write!(f, "std_logic;")
                }
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

pub struct VhdlEntity {
    name: String,
    generics: Vec<String>, // All generics have type positive.
    ports: Vec<VhdlPort>,
    signals: Vec<Signal>,
    components: Vec<VhdlComponent>,
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

impl fmt::Display for VhdlEntity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "library ieee;")?;
        writeln!(f, "use ieee.std_logic_1164.all;")?;
        writeln!(f)?;

        // Final VHDL generated for the top-level chip.
        writeln!(f, "entity {} is", keyw(&self.name));

        self.generics.iter().for_each(|x| {
            writeln!(f, "{}", x);
        });

        self.ports.iter().for_each(|x| {
            writeln!(f, "{}", x);
        });

        writeln!(f, "end entity {};", keyw(&self.name));
        writeln!(f);

        writeln!(f, "architecture arch of {} is", keyw(&self.name));
        self.signals.iter().for_each(|x| {
            writeln!(f, "signal {}", x);
        });

        self.components.iter().for_each(|x| {
            writeln!(f, "{}", x);
        });

        write!(f, "")
    }
}

/// This is where VHDL is synthesized for an HDL chip.
impl TryFrom<&ChipHDL> for VhdlEntity {
    type Error = Box<dyn Error>;

    fn try_from(chip_hdl: &ChipHDL) -> Result<Self, Box<dyn Error>> {
        // Declare components
        let vhdl_components = generate_components(chip_hdl);
        let mut generics: Vec<String> = Vec::new();
        let mut ports: Vec<VhdlPort> = Vec::new();
        let components: Vec<Component> = vhdl_components
            .iter()
            .map(|vc| Component::from(vc))
            .collect();

        for port in &chip_hdl.ports {
            ports.push(VhdlPort::from(port));
        }

        let inferred_widths = infer_widths(
            &chip_hdl,
            &Vec::new(),
            &components,
            &chip_hdl.provider,
            &Vec::new(),
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

        // for part in chip_hdl.parts {
        //     match part {
        //         Part::Component(c) => {
        //             // Generate the VHDL definitions for each type of component.
        //             let generated_definition = generate_component_definition(c)?;
        //             if generated_definition.is_none() {
        //                 continue;
        //             }
        //             self.entities
        //                 .extend(generated_definition.unwrap().dependencies);

        //             // Generate component declarations for components used by this chip.
        //             // Only output one declaration even if the component is used multiple times.
        //             let generated_declaration = self.generate_component_declaration(c)?;
        //             if !component_decls.contains(&generated_declaration) {
        //                 write!(&mut top_level_vhdl, "{}", &generated_declaration)?;
        //                 component_decls.insert(generated_declaration);
        //             }
        //         }
        //         Part::Loop(lp) => {
        //             for c in &lp.body {
        //                 let generated_definition = self.generate_component_definition(c)?;
        //                 if generated_definition.is_none() {
        //                     continue;
        //                 }
        //                 self.entities
        //                     .extend(generated_definition.unwrap().dependencies);

        //                 // Generate component declarations for components used by this chip.
        //                 // Only output one declaration even if the component is used multiple times.
        //                 let generated_declaration = self.generate_component_declaration(c)?;
        //                 if !component_decls.contains(&generated_declaration) {
        //                     write!(&mut top_level_vhdl, "{}", &generated_declaration)?;
        //                     component_decls.insert(generated_declaration);
        //                 }
        //             }
        //         }
        //         Part::AssignmentHDL(_) => {}
        //     }

        Ok(VhdlEntity {
            name: chip_hdl.name.clone(),
            generics,
            ports,
            signals,
            components: vhdl_components,
        })
    }
}


/// Abstract VHDL component.
/// label : unit generic map (...) port map (...)
pub struct VhdlComponent {
    label: String,
    unit: String,
    generic_params: Vec<GenericWidth>,
    port_mappings: Vec<PortMappingVHDL>,
}

impl From<&Component> for VhdlComponent {
    fn from(component: &Component) -> Self {
        let port_mappings: Vec<PortMappingVHDL> = component
            .mappings
            .iter()
            .map(|x| PortMappingVHDL::from(x))
            .collect();

        VhdlComponent {
            label: component.name.value.clone(),
            unit: component.name.value.clone(),
            generic_params: component.generic_params.clone(),
            port_mappings,
        }
    }
}


/// Conversion from VhdlComponents to Components is necessary
/// because infer_widths operates over Components.
impl From<&VhdlComponent> for Component {
    fn from(vc: &VhdlComponent) -> Self {
        Component {
            name: Identifier::from(&vc.unit[..]),
            generic_params: vc.generic_params.clone(),
            mappings: vc.port_mappings.iter().map(|x| PortMappingHDL::from(x)).collect()
        }
    }
}

impl fmt::Display for VhdlComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mappings_vhdl: String = self
            .port_mappings
            .iter()
            .map(|x| format!("{}", x))
            .collect::<Vec<String>>()
            .join(", ");

        writeln!(f, "{} port map({});", self.unit, mappings_vhdl)
    }
}

/// BusVHDL represents the abstract VHDL syntax for an array.
/// VHDL example: foo(3 downto 0) or bar(X downto 0)
#[derive(Clone)]
pub struct BusVHDL {
    /// The name of the signal. This is foo or bar in the example above.
    pub name: String,

    /// The start of the slice (inclusive). This will be None for signals without indices.
    pub start: Option<GenericWidth>,

    /// The end of the slice (exclusive). This will be None for signals without indices.
    pub end: Option<GenericWidth>,
}

impl From<&BusHDL> for BusVHDL {
    fn from(hdl: &BusHDL) -> Self {
        BusVHDL {
            name: hdl.name.clone(),
            start: hdl.start.clone(),
            end: hdl.end.clone(),
        }
    }
}

/// Synthesizes VHDL for BusVHDL.
impl std::fmt::Display for BusVHDL {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Only write out downto syntax if this is an array.
        if self.start.is_some() {
            let start : &GenericWidth = self.start.as_ref().unwrap();
            let end : &GenericWidth = self.end.as_ref().unwrap();
            write!(f, "{}({} downto {})", keyw(&self.name), start, end)
        } else {
            write!(f, "{}", &self.name)
        }
    }
}

struct VhdlPort {
    pub name: String,
    pub width: GenericWidth,
    pub direction: PortDirection,
}

impl<'a> Pet<'a> for &'a VhdlPort {
    fn name(&self) -> &'a str {
        &self.name
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
                if port_width_num > 1 {
                    write!(f, "std_logic_vector({} downto 0)", port_width_num - 1)?;
                } else {
                    write!(f, "std_logic")?;
                }
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

#[derive(Clone)]
pub struct PortMappingVHDL {
    pub wire_name: String,
    pub port: BusVHDL,
    pub wire: BusVHDL,
}

impl std::fmt::Display for PortMappingVHDL {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} <= {}", &self.port, &self.wire)
    }
}

impl From<&PortMappingHDL> for PortMappingVHDL {
    fn from(pm: &PortMappingHDL) -> Self {
        PortMappingVHDL {
            wire_name: pm.wire.name.clone(),
            port: BusVHDL::from(pm.port),
            wire: BusVHDL::from(pm.wire),
        }
    }
}

impl From<&PortMappingVHDL> for PortMappingHDL {
    fn from(pm: &PortMappingVHDL) -> Self {
        PortMappingHDL {
            wire_ident: Identifier::from(pm.wire.name.clone()),
            wire_name: pm.wire.name.clone(),
            port: BusHDL::from(pm.port),
            wire: BusHDL::from(pm.wire),
        }
    }
}

pub struct QuartusProject {
    chip_hdl: ChipHDL,
    chip_vhdl: VhdlEntity,
    project_dir: PathBuf,
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

// pub struct VhdlSynthesizer {
//     hdl: ChipHDL,
//     provider: Rc<dyn HdlProvider>,
//     component_counter: usize,
//     entities: HashSet<VhdlEntity>,
// }

// impl VhdlSynthesizer {
//     pub fn new(hdl: ChipHDL, provider: Rc<dyn HdlProvider>) -> Self {
//         VhdlSynthesizer {
//             hdl,
//             provider,
//             component_counter: 1,
//             entities: HashSet::new(),
//         }
//     }

//     /// Synthesizes VHDL for a top-level chip and all of its components.
//     ///
//     /// `hdl` - HDL for the chip to convert to VHDL.
//     /// `provider` - Responsible for fetching HDL files
//     /// `generic_params` - Instantiate the top-level chip with this parameter list.
//     pub fn synth_vhdl(&mut self) -> Result<VhdlEntity, Box<dyn Error>> {
//         // We don't want to make a chip for simulation, because we might have
//         // top-level generics. We aren't simulating the chip, we are translating
//         // the HDL to VHDL.

//         let mut signals: HashSet<String> = HashSet::new();

//         for (component_counter, part) in self.hdl.parts.iter().enumerate() {
//             match part {
//                 Part::Component(c) => {}

//                 Part::Loop(lp) => {}
//                 Part::AssignmentHDL(_) => {}
//             }
//         }

//         for s in &signals {
//             writeln!(&mut signal_vhdl, "{}", s).unwrap();
//         }

//         // Actual chip definition
//         top_level_vhdl = top_level_vhdl + &signal_vhdl;
//         writeln!(&mut top_level_vhdl, "begin").unwrap();
//         top_level_vhdl = top_level_vhdl + &arch_vhdl;
//         writeln!(&mut top_level_vhdl, "end architecture arch;").unwrap();

//         let mut header_vhdl = String::new();
//         writeln!(&mut header_vhdl).unwrap();
//         top_level_vhdl = header_vhdl + &top_level_vhdl;

//         Ok(VhdlEntity {
//             name: self.hdl.name.clone(),
//             dependencies: HashSet::new(),
//         })
//     }

//     /// Generates VHDL corresponding to a component (and subcomponents). This will be the same
//     /// for every instantiation of the component. It is generating the VHDL
//     /// for that type of chip.
//     fn generate_component_definition(
//         &self,
//         component: &Component,
//     ) -> Result<Option<VhdlEntity>, Box<dyn Error>> {
//         // We skip NAND because that is hard-coded and will be copied separately.
//         if &component.name.value.to_lowercase() == "nand" {
//             return Ok(None);
//         }
//         if &component.name.value.to_lowercase() == "dff" {
//             return Ok(None);
//         }

//         let component_hdl = get_hdl(&component.name.value, &self.provider).unwrap();

//         let mut component_synthesizer =
//             VhdlSynthesizer::new(component_hdl.clone(), self.provider.clone());

//         return match component_synthesizer.synth_vhdl() {
//             Err(e) => Err(e),
//             Ok(x) => Ok(Some(x)),
//         };
//     }

//     /// Generates the declaration for a component that can be included in the VHDL.
//     /// of another chip that uses this component.
//     fn generate_component_declaration(
//         &self,
//         component: &Component,
//     ) -> Result<String, Box<dyn Error>> {
//         let component_hdl = get_hdl(&component.name.value, &self.provider)?;
//         let mut component_decl = String::new();
//         writeln!(
//             &mut component_decl,
//             "component {}",
//             keyw(&component_hdl.name)
//         )
//         .unwrap();
//         write!(&mut component_decl, "{}", generics(&component_hdl)?)?;
//         write!(&mut component_decl, "{}", ports(&component_hdl))?;
//         writeln!(&mut component_decl, "end component;")?;
//         writeln!(&mut component_decl)?;

//         Ok(component_decl)
//     }

//     fn synth_component(
//         self,
//         hdl: &ChipHDL,
//         c: &Component,
//         inferred_widths: &HashMap<String, GenericWidth>,
//         signals: &mut HashSet<String>,
//     ) -> Result<String, Box<dyn Error>> {
//         let component_hdl = get_hdl(&c.name.value, &self.provider)?;
//         let component_id = format!("nand2v_c{}", self.component_counter);

//         // Parameters assigned to generic variables.
//         let component_variables: HashMap<String, GenericWidth> = component_hdl
//             .generic_decls
//             .iter()
//             .map(|x| x.value.clone())
//             .zip(c.generic_params.clone())
//             .collect();
//         let vhdl_generic_params: Vec<String> = component_variables
//             .iter()
//             .map(|(var, val)| format!("{} => {}", var, val))
//             .collect();
//         let mut generic_map = String::new();
//         if !component_variables.is_empty() {
//             write!(
//                 &mut generic_map,
//                 "generic map({})\n\t",
//                 vhdl_generic_params.join(",")
//             )?;
//         }

//         let mut port_map: Vec<String> = Vec::new();

//         let mut redirected_ports: HashSet<String> = HashSet::new();
//         for mapping in c.mappings.iter() {
//             // Print the declaration for the signal required for this mapping.
//             let signal_name = mapping.wire.name.clone();
//             if !is_implicit_signal(&hdl, &signal_name) {
//                 let signal_width = inferred_widths.get(&signal_name).unwrap();
//                 let signal = Signal {
//                     name: signal_name,
//                     width: eval_expr(signal_width, &component_variables),
//                 };
//                 let signal_decl_vhdl = signal_declaration(&signal)?;
//                 signals.insert(signal_decl_vhdl);
//             }

//             let port_direction = &component_hdl.get_port(&mapping.port.name)?.direction;
//             let (vhdl_port_name, port_range, wire_name, wire_range) =
//                 port_mapping(&component_hdl, mapping, &inferred_widths)?;

//             if port_direction == &PortDirection::In {
//                 port_map.push(format!(
//                     "{}{} => {}{}",
//                     vhdl_port_name, port_range, wire_name, wire_range
//                 ));
//             } else {
//                 let redirect_signal_name = format!("{}_{}", component_id, vhdl_port_name);
//                 if redirected_ports.get(&vhdl_port_name).is_none() {
//                     redirected_ports.insert(vhdl_port_name.clone());
//                     port_map.push(format!(
//                         "{}{} => {}{}",
//                         vhdl_port_name, port_range, redirect_signal_name, wire_range
//                     ));
//                 }
//                 // writeln!(
//                 //     &mut arch_vhdl,
//                 //     "{}{} <= {}{};",
//                 //     wire_name, wire_range, redirect_signal_name, wire_range
//                 // )?;

//                 let redirect_signal_width = inferred_widths.get(&mapping.wire.name).unwrap();
//                 let sig = signal_declaration(&Signal {
//                     name: redirect_signal_name,
//                     width: eval_expr(redirect_signal_width, &component_variables),
//                 })?;
//                 signals.insert(sig);
//             }
//         }

//         let mut component_vhdl: String = String::new();
//         writeln!(
//             &mut component_vhdl,
//             "{} : {}\n\t{}port map ({}, CLOCK_50 => CLOCK_50);\n",
//             component_id,
//             keyw(&c.name.value),
//             generic_map,
//             port_map.join(", ")
//         )
//         .unwrap();

//         Ok(component_vhdl)
//     }
// }

// /// Creates a quartus prime project inside project_dir
// ///
// /// - `chip`: The parsed HDL of the top-level chip to convert to VHDL.
// /// ` `chips_vhdl`: The VHDL files (strings) of all supporting chips.
// /// - `project_dir` - The directory to place the quartus prime project. This
// ///                 directory should already exist.
// pub fn write_quartus_project(qp: &QuartusProject) -> Result<(), Box<dyn Error>> {
//     let mut tcl = format!("project_new {} -overwrite", &qp.chip_vhdl.name);

//     tcl.push_str(&String::from(
//         r#"
//         # Assign family, device, and top-level file
//         set_global_assignment -name FAMILY "Cyclone V"
//         set_global_assignment -name DEVICE 5CSEMA5F31C6
//         #============================================================
//         # LEDR
//         #============================================================
//         set_location_assignment PIN_V16 -to LEDR[0]
//         set_location_assignment PIN_W16 -to LEDR[1]
//         set_location_assignment PIN_V17 -to LEDR[2]
//         set_location_assignment PIN_V18 -to LEDR[3]
//         set_location_assignment PIN_W17 -to LEDR[4]
//         set_location_assignment PIN_W19 -to LEDR[5]
//         set_location_assignment PIN_Y19 -to LEDR[6]
//         set_location_assignment PIN_W20 -to LEDR[7]
//         set_location_assignment PIN_W21 -to LEDR[8]
//         set_location_assignment PIN_Y21 -to LEDR[9]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[0]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[1]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[2]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[3]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[4]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[5]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[6]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[7]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[8]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to LEDR[9]
//         #============================================================
//         # SW
//         #============================================================
//         set_location_assignment PIN_AB12 -to SW[0]
//         set_location_assignment PIN_AC12 -to SW[1]
//         set_location_assignment PIN_AF9 -to SW[2]
//         set_location_assignment PIN_AF10 -to SW[3]
//         set_location_assignment PIN_AD11 -to SW[4]
//         set_location_assignment PIN_AD12 -to SW[5]
//         set_location_assignment PIN_AE11 -to SW[6]
//         set_location_assignment PIN_AC9 -to SW[7]
//         set_location_assignment PIN_AD10 -to SW[8]
//         set_location_assignment PIN_AE12 -to SW[9]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[0]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[1]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[2]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[3]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[4]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[5]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[6]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[7]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[8]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to SW[9]
//         #============================================================
//         # HEX0
//         #============================================================
//         set_location_assignment PIN_AE26 -to HEX0[0]
//         set_location_assignment PIN_AE27 -to HEX0[1]
//         set_location_assignment PIN_AE28 -to HEX0[2]
//         set_location_assignment PIN_AG27 -to HEX0[3]
//         set_location_assignment PIN_AF28 -to HEX0[4]
//         set_location_assignment PIN_AG28 -to HEX0[5]
//         set_location_assignment PIN_AH28 -to HEX0[6]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[0]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[1]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[2]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[3]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[4]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[5]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX0[6]
//         #============================================================
//         # HEX1
//         #============================================================
//         set_location_assignment PIN_AJ29 -to HEX1[0]
//         set_location_assignment PIN_AH29 -to HEX1[1]
//         set_location_assignment PIN_AH30 -to HEX1[2]
//         set_location_assignment PIN_AG30 -to HEX1[3]
//         set_location_assignment PIN_AF29 -to HEX1[4]
//         set_location_assignment PIN_AF30 -to HEX1[5]
//         set_location_assignment PIN_AD27 -to HEX1[6]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[0]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[1]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[2]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[3]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[4]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[5]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX1[6]
//         #============================================================
//         # HEX2
//         #============================================================
//         set_location_assignment PIN_AB23 -to HEX2[0]
//         set_location_assignment PIN_AE29 -to HEX2[1]
//         set_location_assignment PIN_AD29 -to HEX2[2]
//         set_location_assignment PIN_AC28 -to HEX2[3]
//         set_location_assignment PIN_AD30 -to HEX2[4]
//         set_location_assignment PIN_AC29 -to HEX2[5]
//         set_location_assignment PIN_AC30 -to HEX2[6]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[0]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[1]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[2]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[3]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[4]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[5]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX2[6]
//         #============================================================
//         # HEX3
//         #============================================================
//         set_location_assignment PIN_AD26 -to HEX3[0]
//         set_location_assignment PIN_AC27 -to HEX3[1]
//         set_location_assignment PIN_AD25 -to HEX3[2]
//         set_location_assignment PIN_AC25 -to HEX3[3]
//         set_location_assignment PIN_AB28 -to HEX3[4]
//         set_location_assignment PIN_AB25 -to HEX3[5]
//         set_location_assignment PIN_AB22 -to HEX3[6]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[0]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[1]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[2]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[3]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[4]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[5]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX3[6]
//         #============================================================
//         # HEX4
//         #============================================================
//         set_location_assignment PIN_AA24 -to HEX4[0]
//         set_location_assignment PIN_Y23 -to HEX4[1]
//         set_location_assignment PIN_Y24 -to HEX4[2]
//         set_location_assignment PIN_W22 -to HEX4[3]
//         set_location_assignment PIN_W24 -to HEX4[4]
//         set_location_assignment PIN_V23 -to HEX4[5]
//         set_location_assignment PIN_W25 -to HEX4[6]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[0]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[1]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[2]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[3]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[4]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[5]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX4[6]
//         #============================================================
//         # HEX5
//         #============================================================
//         set_location_assignment PIN_V25 -to HEX5[0]
//         set_location_assignment PIN_AA28 -to HEX5[1]
//         set_location_assignment PIN_Y27 -to HEX5[2]
//         set_location_assignment PIN_AB27 -to HEX5[3]
//         set_location_assignment PIN_AB26 -to HEX5[4]
//         set_location_assignment PIN_AA26 -to HEX5[5]
//         set_location_assignment PIN_AA25 -to HEX5[6]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[0]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[1]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[2]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[3]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[4]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[5]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to HEX5[6]
//         #============================================================
//         # KEY
//         #============================================================
//         set_location_assignment PIN_AA14 -to KEY[0]
//         set_location_assignment PIN_AA15 -to KEY[1]
//         set_location_assignment PIN_W15 -to KEY[2]
//         set_location_assignment PIN_Y16 -to KEY[3]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to KEY[0]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to KEY[1]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to KEY[2]
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to KEY[3]
//         #============================================================
//         # CLOCK
//         #============================================================
//         set_location_assignment PIN_AF14 -to CLOCK_50
//         set_instance_assignment -name IO_STANDARD "3.3-V LVTTL" -to CLOCK_50

//         # Device and Pin options
//         set_global_assignment -name RESERVE_ALL_UNUSED_PINS_WEAK_PULLUP "AS INPUT TRI-STATED"
//     "#,
//     ));

//     writeln!(
//         tcl,
//         "set_global_assignment -name TOP_LEVEL_ENTITY {}",
//         keyw(&qp.chip_vhdl.name)
//     )?;

//     // write out each vhdl file
//     for chip_vhdl in &qp.chip_vhdl.dependencies {
//         let chip_filename = chip_vhdl.name.clone() + ".vhdl";
//         let mut file = File::create(qp.project_dir.join(&chip_filename))?;
//         file.write_all(format!("{}", chip_vhdl).as_bytes())?;
//         writeln!(
//             tcl,
//             "set_global_assignment -name VHDL_FILE {}",
//             chip_filename
//         )?;
//     }

//     let nand_vhdl = r#"
// library ieee;
// use ieee.std_logic_1164.all;
// entity nand_n2v is
// port (a : in std_logic;
// b : in std_logic;
// out_n2v : out std_logic;
// CLOCK_50 : in std_logic
// );
// end entity nand_n2v;
// architecture arch of nand_n2v is
// begin
// out_n2v <= a nand b;
// end architecture arch;
// "#;
//     let mut file = File::create(project_dir.join("nand.vhdl"))?;
//     file.write_all(nand_vhdl.as_bytes())?;

//     let dff_vhdl = r#"
// library ieee;
// use ieee.std_logic_1164.all;
// LIBRARY altera;
// USE altera.altera_primitives_components.all;

// entity DFF_n2v is
// port (in_n2v : in std_logic;
// CLOCK_50 : in std_logic;
// out_n2v : out std_logic);
// end entity DFF_n2v;

// architecture arch of DFF_n2v is

// COMPONENT DFF
//    PORT (d   : IN STD_LOGIC;
//         clk  : IN STD_LOGIC;
//         clrn : IN STD_LOGIC;
//         prn  : IN STD_LOGIC;
//         q    : OUT STD_LOGIC );

// END COMPONENT;

// begin
// x0: DFF port map (d => in_n2v, clk => CLOCK_50, clrn => '1', prn => '1', q => out_n2v);
// end architecture arch;
// "#;
//     let mut file = File::create(project_dir.join("dff.vhdl"))?;
//     file.write_all(dff_vhdl.as_bytes())?;

//     tcl.push_str("set_global_assignment -name VHDL_FILE nand.vhdl\n");
//     tcl.push_str("set_global_assignment -name VHDL_FILE dff.vhdl\n");
//     tcl.push_str("project_close");
//     let mut file = File::create(project_dir.join("project.tcl"))?;
//     file.write_all(tcl.as_bytes())?;

//     Ok(())
// }

// fn generics(chip: &ChipHDL) -> Result<String, Box<dyn Error>> {
//     let mut vhdl = String::new();

//     let mut generics = Vec::new();
//     for g in &chip.generic_decls {
//         let mut generic_vhdl = String::new();
//         write!(&mut generic_vhdl, "{} : positive", keyw(&g.value))?;
//         generics.push(generic_vhdl);
//     }

//     if !generics.is_empty() {
//         writeln!(&mut vhdl, "generic ({});", generics.join(";\n"))?;
//     }

//     Ok(vhdl)
// }

// fn port_mapping(
//     hdl: &ChipHDL,
//     mapping: &PortMapping,
//     inferred_widths: &HashMap<String, GenericWidth>,
// ) -> Result<(String, String, String, String), Box<dyn Error>> {
//     let port_width = &hdl.get_port(&mapping.port.name)?.width;
//     let vhdl_port_name = keyw(&mapping.port.name);

//     let port_range = match &mapping.port.start {
//         None => {
//             if &GenericWidth::Terminal(Terminal::Num(1)) != port_width {
//                 if &mapping.wire.name != "false" && &mapping.wire.name != "true" {
//                     let wire_width = inferred_widths.get(&mapping.wire.name).unwrap();
//                     if &GenericWidth::Terminal(Terminal::Num(1)) == wire_width {
//                         // This happens when port width is 1 due to generic var.
//                         // and signal is width 1 and therefore std_logic.
//                         // The widths match up, but one is std_logic_vector and one is std_logic.
//                         String::from("(0)")
//                     } else {
//                         String::from("")
//                     }
//                 } else {
//                     String::from("")
//                 }
//             } else {
//                 String::from("")
//             }
//         }
//         Some(_) => {
//             if let GenericWidth::Terminal(Terminal::Num(1)) = port_width {
//                 format!("({})", mapping.wire.start.as_ref().unwrap())
//             } else {
//                 format!(
//                     "({} downto {})",
//                     &mapping.port.end.as_ref().unwrap(),
//                     &mapping.port.start.as_ref().unwrap()
//                 )
//             }
//         }
//     };

//     let wire_range = match &mapping.wire.start {
//         None => String::from(""),
//         Some(_) => {
//             let wire_width = inferred_widths.get(&mapping.wire.name).unwrap();
//             if let GenericWidth::Terminal(Terminal::Num(1)) = wire_width {
//                 String::from("")
//             } else if let GenericWidth::Terminal(Terminal::Num(1)) = port_width {
//                 format!("({})", mapping.wire.start.as_ref().unwrap())
//             } else {
//                 format!(
//                     "({} downto {})",
//                     &mapping.wire.end.as_ref().unwrap(),
//                     &mapping.wire.start.as_ref().unwrap()
//                 )
//             }
//         }
//     };
//     let wire_name: String = if let "false" = mapping.wire.name.to_lowercase().as_str() {
//         if let GenericWidth::Terminal(Terminal::Num(1)) = port_width {
//             String::from("'0'")
//         } else {
//             // we may not know what the width of the port is
//             String::from("(others => '0')")
//         }
//     } else if let "true" = mapping.wire.name.to_lowercase().as_str() {
//         if let GenericWidth::Terminal(Terminal::Num(1)) = port_width {
//             String::from("'1'")
//         } else {
//             // we may not know what the width of the port is
//             String::from("(others => '1')")
//         }
//     } else {
//         keyw(&mapping.wire.name)
//     };

//     Ok((vhdl_port_name, port_range, wire_name, wire_range))
// }

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

// /// Converts a wire name and width into a signal declaration.
// ///
// /// `signal_name`: The name of the signal to declare.
// /// `signal_width`: The width of the signal to declare.
// pub fn signal_declaration(signal: &Signal) -> Result<String, Box<dyn Error>> {
//     let mut vhdl: String = String::new();

//     write!(&mut vhdl, "signal {} ", keyw(&signal.name))?;
//     if let GenericWidth::Terminal(Terminal::Num(1)) = signal.width {
//         write!(&mut vhdl, ": std_logic;")?;
//     } else {
//         write!(
//             &mut vhdl,
//             ": std_logic_vector({} downto 0);",
//             &signal.width - &GenericWidth::Terminal(Terminal::Num(1))
//         )?;
//     }

//     Ok(vhdl)
// }

// // Signals are implicitly created for true/false literals and port names
// fn is_implicit_signal(hdl: &ChipHDL, signal_name: &str) -> bool {
//     let port_names: HashSet<String> = hdl.ports.iter().map(|x| keyw(&x.name.value)).collect();

//     if signal_name == "true" || signal_name == "false" {
//         return true;
//     }

//     if port_names.contains(signal_name) {
//         return true;
//     }

//     return false;
// }

// fn write_top_level_entity(
//     hdl: &ChipHDL,
//     top_level_vhdl: &mut String,
// ) -> Result<(), Box<dyn Error>> {
//     Ok(())
// }

fn generate_components(hdl: &ChipHDL) -> Vec<VhdlComponent> {
    let mut res: Vec<VhdlComponent> = Vec::new();

    let mut variables = HashMap::new();

    for part in &hdl.parts {
        match part {
            Part::Component(c) => {
                res.push(VhdlComponent::from(c));
            }
            Part::Loop(l) => {
                for e in [&l.start, &l.end] {
                    let end = eval_expr(e, &HashMap::new());
                    variables.insert(l.iterator.value.clone(), end);

                    for c in &l.body {
                        let mut new_c: Component = c.clone();
                        for m in &mut new_c.mappings {
                            m.port.start = m.port.start.as_ref().map(|x| eval_expr(x, &variables));
                            m.port.end = m.port.end.as_ref().map(|x| eval_expr(x, &variables));
                            m.wire.start = m.wire.start.as_ref().map(|x| eval_expr(x, &variables));
                            m.wire.end = m.wire.end.as_ref().map(|x| eval_expr(x, &variables));
                        }

                        new_c.generic_params = new_c
                            .generic_params
                            .iter()
                            .map(|x| eval_expr(x, &variables))
                            .collect();

                        res.push(VhdlComponent::from(&new_c));
                    }
                }
            }
            Part::AssignmentHDL(_a) => {}
        }
    }

    res
}

// #[cfg(test)]
// mod test {
//     use super::*;
//     use crate::scanner::Scanner;
//     use std::fs;
//     use std::path::PathBuf;
//     use tempfile::tempdir;

//     #[test]
//     // Just tests that we get some VHDL out.
//     fn test_lightson_nocrash() {
//         let top_level_file = "resources/tests/de1-hdl/LightsOn.hdl";
//         let source_code = fs::read_to_string(&top_level_file).expect("Unable tor ead file.");
//         let mut scanner = Scanner::new(&source_code, PathBuf::from(&top_level_file));
//         let mut parser = Parser {
//             scanner: &mut scanner,
//         };
//         let hdl = parser.parse().expect("Parse error");
//         let base_path = hdl.path.as_ref().unwrap().parent().unwrap();
//         let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(base_path));
//         let mut vhdl_synthesizer = crate::vhdl::VhdlSynthesizer::new(hdl.clone(), provider);
//         let chip_vhdl = vhdl_synthesizer
//             .synth_vhdl()
//             .expect("Failure synthesizing VHDL.");
//         let temp_dir = tempdir().expect("Unable to create temp directory for test.");
//         let _ = crate::vhdl::QuartusProject::new(hdl, chip_vhdl, temp_dir.into_path());
//     }
// }
