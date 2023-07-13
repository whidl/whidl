use crate::{simulator::Chip, parser};

use quick_xml::DeError;
use serde::Serialize;
use serde::Serializer;
use quick_xml::se::to_string;

use std::fmt;
use rand::Rng;


// ========= STRUCTS ========== //

#[derive(Serialize)]
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

struct PinDirection {

}

struct Pin {
    direction: PinDirection,
}

struct Wire {
    from: Coordinate,
    to: Coordinate,
}

// ========= CONVERSIONS ========== //
impl From<&Chip> for Circuit {
    fn from(chip: &Chip) -> Circuit {
        let mut circuit = Circuit {
            name: String::from("test"),
            components: Vec::new(),
        };

        for c in &chip.components {
            let logisim_component = Component::from(c);
            circuit.components.push(logisim_component);
        }

        circuit
    }
}

impl From<&parser::Component> for Component {
    fn from(chip: &parser::Component) -> Component {
        let mut rng = rand::thread_rng();

        let x : u16 = rng.gen_range(0..1000);
        let y : u16 = rng.gen_range(0..1000);

        Component {
            name: chip.name.value.clone(),
            location: Coordinate { x, y }
        }
    }
}


pub fn export(chip: &Chip) -> Result<String, DeError> {
    let circuit = Circuit::from(chip);
    let serialized = to_string(&circuit)?;
    Ok(serialized)
}