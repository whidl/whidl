use crate::{parser, simulator::Chip};

use quick_xml::se::to_string;
use quick_xml::DeError;
use serde::Serialize;
use serde::Serializer;

use rand::Rng;
use std::fmt;

// ========= STRUCTS ========== //

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
#[serde(rename = "circuit")]
struct Circuit {
    #[serde(rename = "@name")]
    name: String,
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
    #[serde(rename = "@name")]
    name: String,

    #[serde(rename = "@location")]
    location: Coordinate,
}

struct PinDirection {}

struct Pin {
    direction: PinDirection,
}

struct Wire {
    from: Coordinate,
    to: Coordinate,
}

// ========= CONVERSIONS ========== //
impl From<&Chip> for Project {
    fn from(chip: &Chip) -> Project {
        // <lib desc="#Wiring" name="0">
        //     <tool name="Pin">
        //     <a name="appearance" val="classic"/>
        //     </tool>
        // </lib>
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
        let lib_1_gates = Library {
            name: String::from("1"),
            tools: Vec::new(),
            desc: None,
        };
        let lib_2_plexers = Library {
            name: String::from("2"),
            tools: Vec::new(),
            desc: None,
        };
        let lib_3_arithmetic = Library {
            name: String::from("3"),
            tools: Vec::new(),
            desc: None,
        };
        let lib_4_memory = Library {
            name: String::from("4"),
            tools: Vec::new(),
            desc: None,
        };
        let lib_5_io = Library {
            name: String::from("5"),
            tools: Vec::new(),
            desc: None,
        };
        let lib_6_ttl = Library {
            name: String::from("6"),
            tools: Vec::new(),
            desc: None,
        };
        let lib_7_tcl = Library {
            name: String::from("7"),
            tools: Vec::new(),
            desc: None,
        };
        let lib_8_base = Library {
            name: String::from("8"),
            tools: Vec::new(),
            desc: None,
        };
        let lib_9_bfh = Library {
            name: String::from("9"),
            tools: Vec::new(),
            desc: None,
        };
        let lib_10_ioextra = Library {
            name: String::from("10"),
            tools: Vec::new(),
            desc: None,
        };
        let lib_11_soc = Library {
            name: String::from("11"),
            tools: Vec::new(),
            desc: None,
        };
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

        let mut circuit = Circuit {
            name: chip.name.clone(),
            components: Vec::new(),
        };

        for c in &chip.components {
            let logisim_component = Component::from(c);
            circuit.components.push(logisim_component);
        }

        let a_gate_undefined = Attribute {
            name: String::from("gateUndefined"),
            val: String::from("ignore"),
        };
        let a_sim_limit = Attribute {
            name: String::from("simlimit"),
            val: String::from("1000"),
        };
        let a_sim_rand= Attribute {
            name: String::from("simrand"),
            val: String::from("0"),
        };

        let options = Options {
            attributes: vec![a_gate_undefined, a_sim_limit, a_sim_rand],
        };

        let poke_map = Tool {
            lib: Some("8".to_owned()),
            map: Some("Button2".to_owned()),
            name: Some("Poke Tool".to_owned()),
            attributes: Vec::new(),
        };
        let menu_map = Tool {
            lib: Some("8".to_owned()),
            map: Some("Button3".to_owned()),
            name: Some("Menu Tool".to_owned()),
            attributes: Vec::new(),
        };
        let menu2_map = Tool {
            lib: Some("8".to_owned()),
            map: Some("Ctrl Button1".to_owned()),
            name: Some("Menu Tool".to_owned()),
            attributes: Vec::new(),
        };

        let mappings = Mappings {
            tools: vec![poke_map, menu_map, menu2_map]
        };

        Project {
            source: String::from("3.8.0"),
            version: String::from("1.0"),
            circuit,
            lib,
            main: Main { name: String::from("main") },
            options,
            mappings,
        }
    }
}

impl From<&parser::Component> for Component {
    fn from(chip: &parser::Component) -> Component {
        let mut rng = rand::thread_rng();

        let x: u16 = rng.gen_range(0..1000);
        let y: u16 = rng.gen_range(0..1000);

        Component {
            name: chip.name.value.clone(),
            location: Coordinate { x, y },
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
