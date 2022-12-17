// This module is responsible for taking a parsed Chip as input and
// producing equivalent VHDL code.

use std::collections::{HashMap, HashSet};
use std::error::Error;
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
out_n2v : out std_logic;
CLOCK_50 : in std_logic
);
end entity nand_n2v;
architecture arch of nand_n2v is
begin
out_n2v <= a nand b;
end architecture arch;
"#;
    let mut file = File::create(project_dir.join("nand.vhdl"))?;
    file.write_all(nand_vhdl.as_bytes())?;

    let dff_vhdl = r#"
library ieee;
use ieee.std_logic_1164.all;
LIBRARY altera;
USE altera.altera_primitives_components.all;

entity DFF_n2v is
port (in_n2v : in std_logic;
CLOCK_50 : in std_logic;
out_n2v : out std_logic);
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
x0: DFF port map (d => in_n2v, clk => CLOCK_50, clrn => '1', prn => '1', q => out_n2v);
end architecture arch;
"#;
    let mut file = File::create(project_dir.join("dff.vhdl"))?;
    file.write_all(dff_vhdl.as_bytes())?;

    tcl.push_str("set_global_assignment -name VHDL_FILE nand.vhdl\n");
    tcl.push_str("set_global_assignment -name VHDL_FILE dff.vhdl\n");
    tcl.push_str("project_close");
    let mut file = File::create(project_dir.join("project.tcl"))?;
    file.write_all(tcl.as_bytes())?;

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

    writeln!(
        &mut vhdl,
        "port (CLOCK_50 : in std_logic; {});",
        ports.join(";\n")
    )
    .unwrap();

    vhdl
}

fn port_mapping(
    hdl: &ChipHDL,
    mapping: &PortMapping,
    inferred_widths: &HashMap<String, GenericWidth>,
) -> Result<(String, String, String, String), Box<dyn Error>> {
    let port_width = &hdl.get_port(&mapping.port.name)?.width;
    let vhdl_port_name = keyw(&mapping.port.name);

    let port_range = match &mapping.port.start {
        None => {
            if &GenericWidth::Terminal(Terminal::Num(1)) != port_width {
                if &mapping.wire.name != "false" && &mapping.wire.name != "true" {
                    let wire_width = inferred_widths.get(&mapping.wire.name).unwrap();
                    if &GenericWidth::Terminal(Terminal::Num(1)) == wire_width {
                        // This happens when port width is 1 due to generic var.
                        // and signal is width 1 and therefore std_logic.
                        // The widths match up, but one is std_logic_vector and one is std_logic.
                        String::from("(0)")
                    } else {
                        String::from("")
                    }
                } else {
                    String::from("")
                }
            } else {
                String::from("")
            }
        }
        Some(_) => {
            if let GenericWidth::Terminal(Terminal::Num(1)) = port_width {
                format!("({})", mapping.wire.start.as_ref().unwrap())
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
            let wire_width = inferred_widths.get(&mapping.wire.name).unwrap();
            if let GenericWidth::Terminal(Terminal::Num(1)) = wire_width {
                String::from("")
            } else if let GenericWidth::Terminal(Terminal::Num(1)) = port_width {
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
    let wire_name: String = if let "false" = mapping.wire.name.to_lowercase().as_str() {
        if let GenericWidth::Terminal(Terminal::Num(1)) = port_width {
            String::from("'0'")
        } else {
            // we may not know what the width of the port is
            String::from("(others => '0')")
        }
    } else if let "true" = mapping.wire.name.to_lowercase().as_str() {
        if let GenericWidth::Terminal(Terminal::Num(1)) = port_width {
            String::from("'1'")
        } else {
            // we may not know what the width of the port is
            String::from("(others => '1')")
        }
    } else {
        keyw(&mapping.wire.name)
    };

    Ok((vhdl_port_name, port_range, wire_name, wire_range))
}

// VHDL keywords that we can't use.
fn keyw(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "abs" => String::from("abs_n2v"),
        "access" => String::from("access_n2v"),
        "after" => String::from("after_n2v"),
        "alias" => String::from("alias_n2v"),
        "all" => String::from("all_n2v"),
        "and" => String::from("and_n2v"),
        "architecture" => String::from("architecture_n2v"),
        "array" => String::from("array_n2v"),
        "assert" => String::from("assert_n2v"),
        "attribute" => String::from("attribute_n2v"),
        "begin" => String::from("begin_n2v"),
        "block" => String::from("block_n2v"),
        "body" => String::from("body_n2v"),
        "buffer" => String::from("buffer_n2v"),
        "bus" => String::from("bus_n2v"),
        "case" => String::from("case_n2v"),
        "component" => String::from("component_n2v"),
        "configuration" => String::from("configuration_n2v"),
        "constant" => String::from("constant_n2v"),
        "disconnect" => String::from("disconnect_n2v"),
        "downto" => String::from("downto_n2v"),
        "else" => String::from("else_n2v"),
        "elsif" => String::from("elsif_n2v"),
        "end" => String::from("end_n2v"),
        "entity" => String::from("entity_n2v"),
        "exit" => String::from("exit_n2v"),
        "file" => String::from("file_n2v"),
        "for" => String::from("for_n2v"),
        "function" => String::from("function_n2v"),
        "generate" => String::from("generate_n2v"),
        "generic" => String::from("generic_n2v"),
        "group" => String::from("group_n2v"),
        "guarded" => String::from("guarded_n2v"),
        "if" => String::from("if_n2v"),
        "impure" => String::from("impure_n2v"),
        "in" => String::from("in_n2v"),
        "intertial" => String::from("inertial_n2v"),
        "inout" => String::from("inout_n2v"),
        "is" => String::from("is_n2v"),
        "label" => String::from("label_n2v"),
        "library" => String::from("library_n2v"),
        "linkage" => String::from("linkage_n2v"),
        "literal" => String::from("literal_n2v"),
        "loop" => String::from("loop_n2v"),
        "map" => String::from("map_n2v"),
        "mod" => String::from("mod_n2v"),
        "nand" => String::from("nand_n2v"),
        "new" => String::from("new_n2v"),
        "next" => String::from("next_n2v"),
        "nor" => String::from("nor_n2v"),
        "not" => String::from("not_n2v"),
        "null" => String::from("null_n2v"),
        "of" => String::from("of_n2v"),
        "on" => String::from("on_n2v"),
        "open" => String::from("open_n2v"),
        "or" => String::from("or_n2v"),
        "others" => String::from("others_n2v"),
        "out" => String::from("out_n2v"),
        "package" => String::from("package_n2v"),
        "port" => String::from("port_n2v"),
        "postponed" => String::from("postponed_n2v"),
        "procedure" => String::from("procedure_n2v"),
        "process" => String::from("process_n2v"),
        "pure" => String::from("pure_n2v"),
        "range" => String::from("range_n2v"),
        "record" => String::from("record_n2v"),
        "register" => String::from("register_n2v"),
        "reject" => String::from("reject_n2v"),
        "rem" => String::from("rem_n2v"),
        "report" => String::from("report_n2v"),
        "return" => String::from("return_n2v"),
        "rol" => String::from("rol_n2v"),
        "ror" => String::from("ror_n2v"),
        "select " => String::from("select_n2v"),
        "severity" => String::from("severity_n2v"),
        "signal" => String::from("signal_n2v"),
        "shared" => String::from("shared_n2v"),
        "sla" => String::from("sla_n2v"),
        "sll" => String::from("sll_n2v"),
        "sra" => String::from("sra_n2v"),
        "srl" => String::from("srl_n2v"),
        "subtype" => String::from("subtype_n2v"),
        "then" => String::from("then_n2v"),
        "to" => String::from("to_n2v"),
        "transport" => String::from("transport_n2v"),
        "type" => String::from("type_n2v"),
        "unaffected" => String::from("unaffected_n2v"),
        "units" => String::from("units_n2v"),
        "until" => String::from("until_n2v"),
        "use" => String::from("use_n2v"),
        "variable" => String::from("variable_n2v"),
        "waitwhen" => String::from("waitwhen_n2v"),
        "while" => String::from("while_n2v"),
        "with" => String::from("with_n2v"),
        "xnor" => String::from("xnor_n2v"),
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
) -> Result<HashMap<String, String>, Box<dyn Error>> {
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
    )?;

    writeln!(&mut top_level_vhdl)?;

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
                        write!(&mut top_level_vhdl, "{}", &generated_declaration)?;
                        component_decls.insert(generated_declaration);
                    }
                }
            }
        }
    }

    let mut signal_vhdl: String = String::new();
    let mut arch_vhdl: String = String::new();

    let components = generate_components(hdl)?;
    let inferred_widths = infer_widths(hdl, &components, provider, &Vec::new())?;
    let port_names: HashSet<String> = hdl.ports.iter().map(|x| keyw(&x.name.value)).collect();

    let print_signal = |wire_name: &String, wire_width: &GenericWidth| -> String {
        let mut new_signal: String = String::new();
        if !port_names.contains(&keyw(wire_name)) {
            write!(&mut new_signal, "signal {} ", keyw(wire_name)).unwrap();
            if let GenericWidth::Terminal(Terminal::Num(1)) = wire_width {
                write!(&mut new_signal, ": std_logic;").unwrap();
            } else {
                write!(
                    &mut new_signal,
                    ": std_logic_vector({} downto 0);",
                    wire_width - &GenericWidth::Terminal(Terminal::Num(1))
                )
                .unwrap();
            }
        }

        new_signal
    };

    let mut signals: HashSet<String> = HashSet::new();

    for (component_counter, part) in hdl.parts.iter().enumerate() {
        match part {
            Part::Component(c) => {
                let component_hdl = get_hdl(&c.name.value, provider)?;
                let component_id = format!("nand2v_c{}", component_counter);

                // Parameters assigned to generic variables.
                let component_variables: HashMap<String, GenericWidth> = component_hdl
                    .generic_decls
                    .iter()
                    .map(|x| x.value.clone())
                    .zip(c.generic_params.clone())
                    .collect();
                let vhdl_generic_params: Vec<String> = component_variables
                    .iter()
                    .map(|(var, val)| format!("{} => {}", var, val))
                    .collect();
                let mut generic_map = String::new();
                if !component_variables.is_empty() {
                    write!(
                        &mut generic_map,
                        "generic map({})\n\t",
                        vhdl_generic_params.join(",")
                    )?;
                }

                let mut port_map: Vec<String> = Vec::new();

                let mut redirected_ports: HashSet<String> = HashSet::new();
                for mapping in c.mappings.iter() {
                    // Print the declaration for the signal required for this mapping.
                    if &mapping.wire.name != "true" && &mapping.wire.name != "false" {
                        let wire_width = inferred_widths.get(&mapping.wire.name).unwrap();
                        let sig = print_signal(
                            &mapping.wire.name,
                            &eval_expr(wire_width, &component_variables),
                        );
                        signals.insert(sig);
                    }

                    let port_direction = &component_hdl.get_port(&mapping.port.name)?.direction;
                    let (vhdl_port_name, port_range, wire_name, wire_range) =
                        port_mapping(&component_hdl, mapping, &inferred_widths)?;

                    if port_direction == &PortDirection::In {
                        port_map.push(format!(
                            "{}{} => {}{}",
                            vhdl_port_name, port_range, wire_name, wire_range
                        ));
                    } else {
                        let redirect_signal = format!("{}_{}", component_id, vhdl_port_name);
                        if redirected_ports.get(&vhdl_port_name).is_none() {
                            redirected_ports.insert(vhdl_port_name.clone());
                            port_map.push(format!(
                                "{}{} => {}{}",
                                vhdl_port_name, port_range, redirect_signal, wire_range
                            ));
                        }
                        writeln!(
                            &mut arch_vhdl,
                            "{}{} <= {}{};",
                            wire_name, wire_range, redirect_signal, wire_range
                        )?;

                        let wire_width = inferred_widths.get(&mapping.wire.name).unwrap();
                        let sig = print_signal(
                            &redirect_signal,
                            &eval_expr(wire_width, &component_variables),
                        );
                        signals.insert(sig);
                    }
                }

                writeln!(
                    &mut arch_vhdl,
                    "{} : {}\n\t{}port map ({}, CLOCK_50 => CLOCK_50);\n",
                    component_id,
                    keyw(&c.name.value),
                    generic_map,
                    port_map.join(", ")
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
                        let component_id = format!("n2vc{}_lp{}", component_counter, i);

                        // Parameters assigned to generic variables.
                        let component_variables: HashMap<String, GenericWidth> = component_hdl
                            .generic_decls
                            .iter()
                            .map(|x| x.value.clone())
                            .zip(c.generic_params.clone())
                            .collect();
                        let vhdl_generic_params: Vec<String> = component_variables
                            .iter()
                            .map(|(var, val)| format!("{} => {}", var, val))
                            .collect();
                        let mut generic_map = String::new();
                        if !component_variables.is_empty() {
                            write!(
                                &mut generic_map,
                                "generic map({})\n\t",
                                vhdl_generic_params.join(",")
                            )
                            .unwrap();
                        }

                        let mut port_map: Vec<String> = Vec::new();

                        let mut redirected_ports: HashSet<String> = HashSet::new();
                        for mapping in c.mappings.iter() {
                            // Print the declaration for the signal required for this mapping.
                            if &mapping.wire.name != "true" && &mapping.wire.name != "false" {
                                let wire_width = inferred_widths.get(&mapping.wire.name).unwrap();
                                let sig = print_signal(
                                    &mapping.wire.name,
                                    &eval_expr(wire_width, &component_variables),
                                );
                                signals.insert(sig);
                            }

                            let port_direction =
                                &component_hdl.get_port(&mapping.port.name)?.direction;
                            let (vhdl_port_name, port_range, wire_name, wire_range) =
                                port_mapping(&component_hdl, mapping, &inferred_widths)?;

                            if port_direction == &PortDirection::In {
                                port_map.push(format!(
                                    "{}{} => {}{}",
                                    vhdl_port_name, port_range, wire_name, wire_range
                                ));
                            } else if &mapping.wire.name != "true" && &mapping.wire.name != "false"
                            {
                                let redirect_signal =
                                    format!("{}_{}", component_id, vhdl_port_name);
                                if redirected_ports.get(&vhdl_port_name).is_none() {
                                    redirected_ports.insert(vhdl_port_name.clone());
                                    port_map.push(format!(
                                        "{}{} => {}{}",
                                        vhdl_port_name, port_range, redirect_signal, wire_range
                                    ));
                                }
                                writeln!(
                                    &mut body_vhdl,
                                    "{}{} <= {}{};",
                                    wire_name, wire_range, redirect_signal, wire_range
                                )
                                .unwrap();

                                let wire_width = inferred_widths.get(&mapping.wire.name).unwrap();
                                let sig = print_signal(
                                    &redirect_signal,
                                    &eval_expr(wire_width, &component_variables),
                                );
                                signals.insert(sig);
                            } else {
                                port_map.push(format!(
                                    "{}{} => {}",
                                    vhdl_port_name, port_range, &mapping.wire.name
                                ));
                            }
                        }

                        writeln!(
                            &mut body_vhdl,
                            "{} : {}\n\t{}port map ({}, CLOCK_50 => CLOCK_50);\n",
                            component_id,
                            keyw(&c.name.value),
                            generic_map,
                            port_map.join(", ")
                        )
                        .unwrap();

                        Ok(body_vhdl)
                    })
                    .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
                writeln!(
                    &mut arch_vhdl,
                    "n2vlp{} : for {} in {} to {} generate\n{} end generate n2vlp{};",
                    component_counter,
                    lp.iterator.value,
                    lp.start,
                    lp.end,
                    body.join("\n"),
                    component_counter
                )?;
            }
        }
    }

    for s in &signals {
        writeln!(&mut signal_vhdl, "{}", s).unwrap();
    }

    // Actual chip definition
    top_level_vhdl = top_level_vhdl + &signal_vhdl;
    writeln!(&mut top_level_vhdl, "begin").unwrap();
    top_level_vhdl = top_level_vhdl + &arch_vhdl;
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
) -> Result<HashMap<String, String>, Box<dyn Error>> {
    // We skip NAND because that is hard-coded and will be copied separately.
    if &component.name.value.to_lowercase() == "nand" {
        return Ok(HashMap::new());
    }
    if &component.name.value.to_lowercase() == "dff" {
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

fn generate_components(hdl: &ChipHDL) -> Result<Vec<Component>, N2VError> {
    let mut res = Vec::new();

    let mut variables = HashMap::new();

    for part in &hdl.parts {
        match part {
            Part::Component(c) => {
                res.push(c.clone());
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

                        res.push(new_c);
                    }
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
