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
    circuit: Circuit,
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
        let mut circuit = Circuit {
            name: chip.name.clone(),
            components: Vec::new(),
        };

        for c in &chip.components {
            let logisim_component = Component::from(c);
            circuit.components.push(logisim_component);
        }

        Project {
            source: String::from("3.8.0"),
            version: String::from("1.0"),
            circuit,
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
