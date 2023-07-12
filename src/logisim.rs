use crate::simulator::Chip;
use quick_xml::DeError;
use serde::Serialize;

use quick_xml::se::to_string;


// ========= STRUCTS ========== //

#[derive(Serialize)]
struct Circuit {
    name: String,
    components: Vec<Component>,
}

#[derive(Serialize)]
struct Coordinate {
    x: i32,
    y: i32,
}


#[derive(Serialize)]
struct Component {
    name: String,
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

pub fn export(chip: &Chip) -> Result<String, DeError> {
    let c = Circuit {
        name: String::from("test"),
        components: Vec::new(),
    };

    let serialized = to_string(&c)?;

    Ok(serialized)
}