use crate::simulator::Port;
use crate::{parser, simulator::Chip};

use quick_xml::se::to_string;
use quick_xml::DeError;
use serde::Serialize;
use serde::Serializer;

use rand::Rng;
use std::collections::HashMap;
use std::fmt;

// ========= STRUCTS ========== //

#[derive(Serialize)]
enum ToolbarItem {
    #[serde(rename = "sep")]
    Separator,
    #[serde(rename = "tool")]
    Tool(Tool),
}

#[derive(Serialize)]
#[serde(rename = "project")]
struct Project {
    #[serde(rename = "@source")]
    source: String,
    #[serde(rename = "@version")]
    version: String,
    lib: Vec<Library>,
    main: Main,
    options: Options,
    mappings: Mappings,
    toolbar: Toolbar,
    circuit: Circuit,
}

#[derive(Serialize)]
#[serde(rename = "lib")]
struct Library {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@desc")]
    desc: Option<String>,
    #[serde(rename = "tool")]
    tools: Vec<Tool>,
}

#[derive(Serialize)]
#[serde(rename = "tool")]
struct Tool {
    #[serde(rename = "@lib")]
    lib: Option<String>,
    #[serde(rename = "@map")]
    map: Option<String>,
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "a")]
    attributes: Vec<Attribute>,
}

#[derive(Serialize)]
#[serde(rename = "a")]
struct Attribute {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@val")]
    val: String,
}

#[derive(Serialize)]
#[serde(rename = "main")]
struct Main {
    #[serde(rename = "@name")]
    name: String,
}

#[derive(Serialize)]
#[serde(rename = "options")]
struct Options {
    #[serde(rename = "a")]
    attributes: Vec<Attribute>,
}

#[derive(Serialize)]
#[serde(rename = "mappings")]
struct Mappings {
    #[serde(rename = "tool")]
    tools: Vec<Tool>,
}

#[derive(Serialize)]
#[serde(rename = "toolbar")]
struct Toolbar {
    #[serde(rename = "tool")]
    items: Vec<ToolbarItem>,
}

#[derive(Serialize)]
#[serde(rename = "circuit")]
struct Circuit {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "a")]
    attributes: Vec<Attribute>,
    #[serde(rename = "comp")]
    components: Vec<Component>,
}

struct Coordinate {
    x: u16,
    y: u16,
}

impl Serialize for Coordinate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Create a string in the format "(x, y)"
        let s = format!("({}, {})", self.x, self.y);
        // Serialize the string as an attribute
        serializer.serialize_str(&s)
    }
}

impl fmt::Display for Coordinate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[derive(Serialize)]
#[serde(rename = "comp")]
struct Component {
    #[serde(rename = "@lib")]
    lib: String,
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@loc")]
    location: Coordinate,
    // Add a Vector field to store Attributes
    #[serde(rename = "a", skip_serializing_if = "Vec::is_empty")]
    attributes: Vec<Attribute>,
}

struct PinDirection {}

struct Pin {
    direction: PinDirection,
}

struct Wire {
    from: Coordinate,
    to: Coordinate,
}

fn create_library(name: &str, desc: &str) -> Library {
    Library {
        name: name.to_owned(),
        tools: Vec::new(),
        desc: Some(desc.to_owned()),
    }
}

fn create_attribute(name: &str, val: &str) -> Attribute {
    Attribute {
        name: name.to_owned(),
        val: val.to_owned(),
    }
}

fn create_tool(name: &str, lib: &str, map: Option<String>) -> Tool {
    Tool {
        name: Some(name.to_owned()),
        lib: Some(lib.to_owned()),
        map: map.to_owned(),
        attributes: Vec::new(),
    }
}

// ========= CONVERSIONS ========== //
impl From<&Chip> for Project {
    fn from(chip: &Chip) -> Project {
        let lib_0_wiring = Library {
            desc: Some(String::from("#Wiring")),
            name: String::from("0"),
            tools: vec![Tool {
                name: Some(String::from("Pin")),
                lib: None,
                map: None,
                attributes: vec![Attribute {
                    name: String::from("appearance"),
                    val: String::from("classic"),
                }],
            }],
        };

        let lib_1_gates = create_library("1", "#Gates");
        let lib_2_plexers = create_library("2", "#Plexers");
        let lib_3_arithmetic = create_library("3", "#Arithmetic");
        let lib_4_memory = create_library("4", "#Memory");
        let lib_5_io = create_library("5", "#I/O");
        let lib_6_ttl = create_library("6", "#TTL");
        let lib_7_tcl = create_library("7", "#TCL");
        let lib_8_base = create_library("8", "#Base");
        let lib_9_bfh = create_library("9", "#BFH-Praktika");
        let lib_10_ioextra = create_library("10", "#Input/Output-Extra");
        let lib_11_soc = create_library("11", "#Soc");

        let lib = vec![
            lib_0_wiring,
            lib_1_gates,
            lib_2_plexers,
            lib_3_arithmetic,
            lib_4_memory,
            lib_5_io,
            lib_6_ttl,
            lib_7_tcl,
            lib_8_base,
            lib_9_bfh,
            lib_10_ioextra,
            lib_11_soc,
        ];

        let circuit_appearance = create_attribute("appearance", "logisim_evolution");
        let circuit_facing = create_attribute("facing", "west");
        let circuit_output = create_attribute("output", "true");

        let mut circuit = Circuit {
            name: chip.name.clone(),
            components: Vec::new(),
            attributes: vec![circuit_appearance, circuit_facing, circuit_output],
        };

        for c in &chip.components {
            let logisim_component = Component::from(c);
            circuit.components.push(logisim_component);
        }

        // Ports become logisim pins, which are components in Logisim.
        chip.ports.iter().for_each(|(_, p)| {
            let logisim_component = Component::from(p);
            circuit.components.push(logisim_component);
        });

        let a_gate_undefined = create_attribute("gateUndefined", "ignore");
        let a_sim_limit = create_attribute("simlimit", "1000");
        let a_sim_rand = create_attribute("simrand", "0");

        let options = Options {
            attributes: vec![a_gate_undefined, a_sim_limit, a_sim_rand],
        };

        let poke_map = create_tool("Poke Tool", "8", Some("Button2".to_owned()));
        let menu_map = create_tool("Menu Tool", "8", Some("Button3".to_owned()));
        let menu2_map = create_tool("Menu Tool", "8", Some("Ctrl Button1".to_owned()));

        let mappings = Mappings {
            tools: vec![poke_map, menu_map, menu2_map],
        };

        let poke_toolbar = create_tool("Poke Tool", "8", Some("Button2".to_owned()));
        let edit_toolbar = create_tool("Edit Tool", "8", Some("Button1".to_owned()));
        let wiring_toolbar = create_tool("Wiring Tool", "8", Some("Button3".to_owned()));
        let text_toolbar = create_tool("Text Tool", "8", Some("Ctrl Button1".to_owned()));
        let pin1_toolbar = create_tool("Pin", "0", None);

        let pin2_toolbar = Tool {
            lib: Some("0".to_owned()),
            map: None,
            name: Some("Pin".to_owned()),
            attributes: vec![
                Attribute {
                    name: "facing".to_owned(),
                    val: "west".to_owned(),
                },
                Attribute {
                    name: "output".to_owned(),
                    val: "true".to_owned(),
                },
            ],
        };

        let not_toolbar = create_tool("NOT Gate", "1", None);
        let and_toolbar = create_tool("AND Gate", "1", None);
        let or_toolbar = create_tool("OR Gate", "1", None);
        let xor_toolbar = create_tool("XOR Gate", "1", None);
        let nand_toolbar = create_tool("NAND Gate", "1", None);
        let nor_toolbar = create_tool("NOR Gate", "1", None);
        let dff_toolbar = create_tool("D Flip-Flop", "4", None);
        let reg_toolbar = create_tool("Register", "4", None);

        let toolbar_items = vec![
            ToolbarItem::Tool(poke_toolbar),
            ToolbarItem::Tool(edit_toolbar),
            ToolbarItem::Tool(wiring_toolbar),
            ToolbarItem::Tool(text_toolbar),
            ToolbarItem::Separator,
            ToolbarItem::Tool(pin1_toolbar),
            ToolbarItem::Tool(pin2_toolbar),
            ToolbarItem::Separator,
            ToolbarItem::Tool(not_toolbar),
            ToolbarItem::Tool(and_toolbar),
            ToolbarItem::Tool(or_toolbar),
            ToolbarItem::Tool(xor_toolbar),
            ToolbarItem::Tool(nand_toolbar),
            ToolbarItem::Tool(nor_toolbar),
            ToolbarItem::Separator,
            ToolbarItem::Tool(dff_toolbar),
            ToolbarItem::Tool(reg_toolbar),
        ];

        let toolbar = Toolbar {
            items: toolbar_items,
        };

        Project {
            source: String::from("3.8.0"),
            version: String::from("1.0"),
            circuit,
            lib,
            main: Main {
                name: String::from("main"),
            },
            options,
            mappings,
            toolbar,
        }
    }
}

impl From<&Port> for Component {
    fn from(port: &Port) -> Self {
        // Create random coordinates for the port
        let mut rng = rand::thread_rng();
        let x: u16 = rng.gen_range(0..1000);
        let y: u16 = rng.gen_range(0..1000);

        let facing = match port.direction {
            parser::PortDirection::In => "west",
            parser::PortDirection::Out => "east",
        };

        let output = match port.direction {
            parser::PortDirection::In => "false",
            parser::PortDirection::Out => "true",
        };

        Component {
            lib: "0".to_owned(),
            name: "Pin".to_owned(),
            location: Coordinate { x, y },
            attributes: vec![
                Attribute {
                    name: "facing".to_owned(),
                    val: facing.to_owned(),
                },
                Attribute {
                    name: "output".to_owned(),
                    val: output.to_owned(),
                },
                Attribute {
                    name: "appearance".to_owned(),
                    val: "NewPins".to_owned(),
                },
            ],
        }
    }
}

impl From<&parser::Component> for Component {
    fn from(component: &parser::Component) -> Self {
        let mut rng = rand::thread_rng();

        let x: u16 = rng.gen_range(0..1000);
        let y: u16 = rng.gen_range(0..1000);

        // All components are in library 1.
        let lib = "1";

        let mut rename_map: HashMap<String, String> = HashMap::new();
        rename_map.insert("nand".to_string(), "NAND Gate".to_string());

        let renamed = rename_map
            .get(&component.name.value.to_lowercase())
            .cloned()
            .unwrap_or(component.name.value.clone());

        Component {
            lib: lib.to_owned(),
            name: renamed,
            location: Coordinate { x, y },
            attributes: vec![], // Initialize empty attribute vector
        }
    }
}

pub fn export(chip: &Chip) -> Result<String, DeError> {
    let project = Project::from(chip);
    let serialized = to_string(&project)?;
    let serialized_with_decl = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n{}",
        serialized
    );
    Ok(serialized_with_decl)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::scanner::Scanner;

    #[test]
    fn test_component_serialization() {
        let mut rng = rand::thread_rng();
        let x: u16 = rng.gen_range(0..1000);
        let y: u16 = rng.gen_range(0..1000);
        let component = Component {
            lib: String::from("1"), // test with library 1
            name: String::from("test_component"),
            location: Coordinate { x, y },
            attributes: vec![], // Initialize with no attributes
        };

        let serialized_component = to_string(&component).unwrap();

        // Verify serialization result, you may need to change this based on the expected XML result
        let expected_serialization = format!(
            "<comp lib=\"{}\" name=\"{}\" loc=\"({}, {})\"/>",
            "1", "test_component", x, y
        );
        assert_eq!(serialized_component, expected_serialization);
    }

    // Import required elements here from your main code, adjust as needed
    use quick_xml::se::to_string;
    use std::{fs, path::PathBuf, ptr, rc::Rc};

    use crate::parser::{FileReader, HdlProvider, Parser};

    #[test]
    fn test_chip_conversion_contains_comp() {
        // Read a chip file, parse it and create a Chip structure
        let source_code = fs::read_to_string("resources/tests/nand2tetris/solutions/Not.hdl")
            .expect("unable to read chip file");
        let mut scanner = Scanner::new(&source_code, PathBuf::from("path/to/your/chip/file"));
        let base_path = scanner.path.parent().unwrap();
        let provider: Rc<dyn HdlProvider> = Rc::new(FileReader::new(base_path));
        let mut parser = Parser::new(&mut scanner, provider.clone());
        let hdl = parser.parse().expect("Failed to parse HDL");
        let chip = Chip::new(&hdl, ptr::null_mut(), &provider, true, &Vec::new())
            .expect("Failed to create CHIP");

        // Convert this chip to Project; this is the function you're testing
        let project = Project::from(&chip);

        // Now, serialize this project
        let serialized_project = to_string(&project).unwrap();

        // Then we assert that '<comp' is found in the serialization
        assert!(
            serialized_project.contains("<comp "),
            "The serialized xml does not contain '<comp'"
        );
    }
}
