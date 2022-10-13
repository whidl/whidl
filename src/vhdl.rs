// This module is responsible for taking a parsed Chip as input and
// producing equivalent VHDL code.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::fs::File;
use std::io::Write as OtherWrite;
use std::path::Path;
use std::rc::Rc;

use crate::error::N2VError;
use crate::expr::{eval_expr, GenericWidth, Op, Terminal};
use crate::parser::*;
use crate::simulator::infer_widths;

pub fn create_quartus_project(
    chip: &ChipHDL,
    chips_vhdl: HashMap<String, String>,
    project_dir: &Path,
) -> std::io::Result<()> {
    // check to see if the directory exists. panic if it exists/
    fs::create_dir(project_dir)?;
    let mut tcl = format!("project_new {} -overwrite", chip.name);

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
        keyw(&chip.name)
    )
    .unwrap();

    // write out each vhdl file
    for (chip_name, chip_vhdl) in &chips_vhdl {
        let chip_filename = chip_name.clone() + ".vhdl";
        let mut file = File::create(project_dir.join(&chip_filename))?;
        file.write_all(chip_vhdl.as_bytes())?;
        writeln!(
            tcl,
            "set_global_assignment -name VHDL_FILE {}",
            chip_filename
        )
        .unwrap();
    }

    let nand_vhdl = r#"
library ieee;
use ieee.std_logic_1164.all;
entity nand_n2v is
port (a : in std_logic;
b : in std_logic;
out_n2v : out std_logic
);
end entity nand_n2v;
architecture arch of nand_n2v is
begin
out_n2v <= a nand b;
end architecture arch;
"#;
    let mut file = File::create(project_dir.join("nand.vhdl"))?;
    file.write_all(nand_vhdl.as_bytes())?;

    tcl.push_str("set_global_assignment -name VHDL_FILE nand.vhdl\n");
    tcl.push_str("project_close");
    let mut file = File::create(project_dir.join("project.tcl"))?;
    file.write_all(tcl.as_bytes())?;

    // TODO: pins for de1-soc
    Ok(())
}

fn generics(chip: &ChipHDL) -> String {
    let mut vhdl = String::new();

    let mut generics = Vec::new();
    for g in &chip.generic_decls {
        let mut generic_vhdl = String::new();
        write!(&mut generic_vhdl, "{} : positive", keyw(&g.value)).unwrap();
        generics.push(generic_vhdl);
    }

    if !generics.is_empty() {
        writeln!(&mut vhdl, "generic ({});", generics.join(";\n")).unwrap();
    }

    vhdl
}

fn ports(chip: &ChipHDL) -> String {
    let mut vhdl = String::new();

    let mut ports = Vec::new();
    for port in &chip.ports {
        let mut port_vhdl = String::new();
        write!(&mut port_vhdl, "{} : ", keyw(&port.name.value)).unwrap();
        if port.direction == PortDirection::In {
            write!(&mut port_vhdl, "in ").unwrap();
        } else {
            write!(&mut port_vhdl, "out ").unwrap();
        }

        match &port.width {
            GenericWidth::Terminal(Terminal::Num(port_width_num)) => {
                if port_width_num > &1 {
                    write!(
                        &mut port_vhdl,
                        "std_logic_vector({} downto 0)",
                        port_width_num - 1
                    )
                    .unwrap();
                } else {
                    write!(&mut port_vhdl, "std_logic").unwrap();
                }
            }
            _ => {
                let sub1 = GenericWidth::Expr(
                    Op::Sub,
                    Box::new(port.width.clone()),
                    Box::new(GenericWidth::Terminal(Terminal::Num(1))),
                );
                write!(
                    &mut port_vhdl,
                    "std_logic_vector({} downto 0)",
                    eval_expr(&sub1, &HashMap::new())
                )
                .unwrap();
            }
        };

        ports.push(port_vhdl);
    }

    writeln!(&mut vhdl, "port ({});", ports.join(";\n")).unwrap();

    vhdl
}

// Creates the VHDL generic map for a component.
fn generic_map(component_hdl: &ChipHDL, component: &Component) -> String {
    // Parameters assigned to generic variables.
    let component_variables: Vec<String> = component_hdl
        .generic_decls
        .iter()
        .map(|x| x.value.clone())
        .zip(component.generic_params.clone())
        .map(|(var, val)| format!("{} => {}", var, val))
        .collect();

    if component_variables.is_empty() {
        String::from("")
    } else {
        format!("generic map({})\n\t", component_variables.join(","))
    }
}

fn port_mapping(mapping: &PortMapping) -> String {
    let port_range = match &mapping.port.start {
        None => String::from(""),
        Some(_) => {
            if mapping.port.start == mapping.port.end {
                format!("({})", &mapping.port.start.as_ref().unwrap())
            } else {
                format!(
                    "({} downto {})",
                    &mapping.port.end.as_ref().unwrap(),
                    &mapping.port.start.as_ref().unwrap()
                )
            }
        }
    };
    let wire_range = match &mapping.wire.start {
        None => String::from(""),
        Some(_) => {
            if mapping.wire.start == mapping.wire.end {
                format!("({})", mapping.wire.start.as_ref().unwrap())
            } else {
                format!(
                    "({} downto {})",
                    &mapping.wire.end.as_ref().unwrap(),
                    &mapping.wire.start.as_ref().unwrap()
                )
            }
        }
    };
    let port_name = keyw(&mapping.port.name);
    let wire_name: String = if let "false" = mapping.wire.name.to_lowercase().as_str() {
        // we may not know what the width of the port is
        String::from("(others => '0')")
    } else if let "true" = mapping.wire.name.to_lowercase().as_str() {
        String::from("(others => '1')")
    } else {
        keyw(&mapping.wire.name)
    };
    format!("{}{} => {}{}", port_name, port_range, wire_name, wire_range)
}

// VHDL keywords that we can't use.
fn keyw(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "in" => String::from("in_n2v"),
        "out" => String::from("out_n2v"),
        "not" => String::from("not_n2v"),
        "nand" => String::from("nand_n2v"),
        "and" => String::from("and_n2v"),
        "or" => String::from("or_n2v"),
        "xor" => String::from("xor_n2v"),
        _ => String::from(name),
    }
}

/// Synthesizes VHDL for a top-level chip and all of its components.
///
/// `hdl` - HDL for the chip to convert to VHDL.
/// `provider` - Responsible for fetching HDL files
/// `generic_params` - Instantiate the top-level chip with this parameter list.
pub fn synth_vhdl(
    hdl: &ChipHDL,
    provider: &Rc<dyn HdlProvider>,
) -> Result<HashMap<String, String>, N2VError> {
    // We don't want to make a chip for simulation, because we might have
    // top-level generics. We aren't simulating the chip, we are translating
    // the HDL to VHDL.

    // Component name -> component definition
    let mut entities = HashMap::new();

    // Final VHDL generated for the top-level chip.
    let mut top_level_vhdl = String::new();

    write_top_level_entity(hdl, &mut top_level_vhdl);

    writeln!(
        &mut top_level_vhdl,
        "architecture arch of {} is",
        keyw(&hdl.name)
    )
    .unwrap();

    writeln!(&mut top_level_vhdl).unwrap();

    // Declare components
    let mut component_decls: HashSet<String> = HashSet::new();

    for part in &hdl.parts {
        match part {
            Part::Component(c) => {
                // Generate the VHDL definitions for each type of component.
                let generated_definitions = generate_component_definition(c, provider)?;
                entities.extend(generated_definitions);

                // Generate component declarations for components used by this chip.
                // Only output one declaration even if the component is used multiple times.
                let generated_declaration = generate_component_declaration(c, provider);
                if !component_decls.contains(&generated_declaration) {
                    write!(&mut top_level_vhdl, "{}", &generated_declaration).unwrap();
                    component_decls.insert(generated_declaration);
                }
            }
            Part::Loop(lp) => {
                for c in &lp.body {
                    let generated_definitions = generate_component_definition(c, provider)?;
                    entities.extend(generated_definitions);

                    // Generate component declarations for components used by this chip.
                    // Only output one declaration even if the component is used multiple times.
                    let generated_declaration = generate_component_declaration(c, provider);
                    if !component_decls.contains(&generated_declaration) {
                        write!(&mut top_level_vhdl, "{}", &generated_declaration).unwrap();
                        component_decls.insert(generated_declaration);
                    }
                }
            }
        }
    }

    // Generate the signals required to port/generic map this component.
    let signals = generate_signals(hdl, provider)?;

    for signal in &signals {
        top_level_vhdl = top_level_vhdl + signal;
    }

    // Actual chip definition
    writeln!(&mut top_level_vhdl, "begin").unwrap();

    for (component_counter, part) in hdl.parts.iter().enumerate() {
        match part {
            Part::Component(c) => {
                let component_hdl = get_hdl(&c.name.value, provider).unwrap();
                let generic_map = generic_map(&component_hdl, c);

                let port_map = c
                    .mappings
                    .iter()
                    .map(port_mapping)
                    .collect::<Vec<String>>()
                    .join(", ");

                writeln!(
                    &mut top_level_vhdl,
                    "nand2v_c{} : {}\n\t{}port map ({});\n",
                    component_counter,
                    keyw(&c.name.value),
                    generic_map,
                    port_map
                )
                .unwrap();
            }
            Part::Loop(lp) => {
                let body: Vec<String> = lp
                    .body
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let mut body_vhdl = String::new();
                        let component_hdl = get_hdl(&c.name.value, provider).unwrap();
                        let generic_map = generic_map(&component_hdl, c);
                        let port_map = c
                            .mappings
                            .iter()
                            .map(port_mapping)
                            .collect::<Vec<String>>()
                            .join(", ");
                        writeln!(
                            &mut body_vhdl,
                            "n2vlpc{} : {}\n\t{}port map ({});",
                            i,
                            keyw(&c.name.value),
                            generic_map,
                            port_map
                        )
                        .unwrap();
                        body_vhdl
                    })
                    .collect();
                writeln!(
                    &mut top_level_vhdl,
                    "n2vlp{} : for {} in {} to {} generate {} end generate n2vlp{};",
                    component_counter,
                    lp.iterator.value,
                    lp.start,
                    lp.end,
                    body.join("\n"),
                    component_counter
                )
                .unwrap();
            }
        }
    }

    writeln!(&mut top_level_vhdl, "end architecture arch;").unwrap();

    let mut header_vhdl = String::new();
    writeln!(&mut header_vhdl, "library ieee;").unwrap();
    writeln!(&mut header_vhdl, "use ieee.std_logic_1164.all;").unwrap();
    writeln!(&mut header_vhdl).unwrap();
    top_level_vhdl = header_vhdl + &top_level_vhdl;

    entities.insert(hdl.name.clone(), top_level_vhdl);

    Ok(entities)
}

fn write_top_level_entity(hdl: &ChipHDL, top_level_vhdl: &mut String) {
    writeln!(top_level_vhdl, "entity {} is", keyw(&hdl.name)).unwrap();
    if !hdl.generic_decls.is_empty() {
        writeln!(top_level_vhdl, "{}", generics(hdl)).unwrap();
    }
    writeln!(top_level_vhdl, "{}", ports(hdl)).unwrap();
    writeln!(top_level_vhdl, "end entity {};", keyw(&hdl.name)).unwrap();
    writeln!(top_level_vhdl).unwrap();
}

/// Generates VHDL corresponding to a component (and subcomponents). This will be the same
/// for every instantiation of the component. It is generating the VHDL
/// for that type of chip.
fn generate_component_definition(
    component: &Component,
    provider: &Rc<dyn HdlProvider>,
) -> Result<HashMap<String, String>, N2VError> {
    // We skip NAND because that is hard-coded and will be copied separately.
    if &component.name.value.to_lowercase() == "nand" {
        return Ok(HashMap::new());
    }

    let component_hdl = get_hdl(&component.name.value, provider).unwrap();
    synth_vhdl(&component_hdl, provider)
}

/// Generates the declaration for a component that can be included in the VHDL.
/// of another chip that uses this component.
fn generate_component_declaration(component: &Component, provider: &Rc<dyn HdlProvider>) -> String {
    let component_hdl = get_hdl(&component.name.value, provider).unwrap();
    let mut component_decl = String::new();
    writeln!(
        &mut component_decl,
        "component {}",
        keyw(&component_hdl.name)
    )
    .unwrap();
    write!(&mut component_decl, "{}", generics(&component_hdl)).unwrap();
    write!(&mut component_decl, "{}", ports(&component_hdl)).unwrap();
    writeln!(&mut component_decl, "end component;").unwrap();
    writeln!(&mut component_decl).unwrap();
    component_decl
}

/// Generates signal definitions for the signals in a chip, iterates over all components.
/// * `hdl` - HDL of the chip
/// * `provider` - Responsible for fetching HDL files.
/// * `generic_params` - Parameter list for the *chip defined by `hdl`*
/// * `variables` - Variable state of the chip.
fn generate_signals(
    hdl: &ChipHDL,
    provider: &Rc<dyn HdlProvider>,
) -> Result<HashSet<String>, N2VError> {
    let mut signals = HashSet::new();

    // Extract list of components. For components in a generator loop substitute
    // max range value for iterator variable to ensure the widest possible
    // signal is generated.
    let components = generate_components(hdl)?;

    let port_names: HashSet<String> = hdl.ports.iter().map(|x| keyw(&x.name.value)).collect();
    let inferred_widths = infer_widths(hdl, &components, provider, &Vec::new())?;

    for component in components {
        for mapping in &component.mappings {
            if &mapping.wire.name == "true" || &mapping.wire.name == "false" {
                continue;
            }

            let mut signal_vhdl = String::new();

            let wire_width = inferred_widths.get(&mapping.wire.name).unwrap();
            if !port_names.contains(&keyw(&mapping.wire.name)) {
                write!(&mut signal_vhdl, "signal {} ", keyw(&mapping.wire.name)).unwrap();
                if let GenericWidth::Terminal(Terminal::Num(1)) = wire_width {
                    writeln!(&mut signal_vhdl, ": std_logic;").unwrap();
                } else {
                    writeln!(
                        &mut signal_vhdl,
                        ": std_logic_vector({} downto 0);",
                        wire_width - &GenericWidth::Terminal(Terminal::Num(1))
                    )
                    .unwrap();
                }
            }
            signals.insert(signal_vhdl);
        }
    }

    Ok(signals)
}

fn generate_components(hdl: &ChipHDL) -> Result<Vec<Component>, N2VError> {
    let mut res = Vec::new();

    let mut variables = HashMap::new();

    for part in &hdl.parts {
        match part {
            Part::Component(c) => {
                res.push(c.clone());
            }
            Part::Loop(l) => {
                let end = eval_expr(&l.end, &HashMap::new());
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

                    res.push(new_c);
                }
            }
        }
    }

    Ok(res)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::scanner::Scanner;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    // Just tests that we get some VHDL out.
    fn test_lightson_nocrash() {
        let mut scanner: Scanner;
        let source_code;
        let top_level_file = "resources/tests/de1-hdl/LightsOn.hdl";
        let contents = fs::read_to_string(&top_level_file);
        match contents {
            Ok(sc) => {
                source_code = sc;
                scanner = Scanner::new(&source_code, PathBuf::from(&top_level_file));
            }
            Err(_) => panic!("Unable to read file."),
        }
        let mut parser = Parser {
            scanner: &mut scanner,
        };
        let hdl = parser.parse().expect("Parse error");
        let base_path = String::from(
            hdl.path
                .as_ref()
                .unwrap()
                .parent()
                .unwrap()
                .to_str()
                .unwrap(),
        );
        let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(&base_path));
        let entities = crate::vhdl::synth_vhdl(&hdl, &provider).unwrap();
        let temp_dir = tempdir().unwrap();
        let quartus_dir = temp_dir.path().join("dummy");
        crate::vhdl::create_quartus_project(&hdl, entities, &quartus_dir)
            .expect("Unable to create project");
    }
}
