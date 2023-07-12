// ========= STRUCTS ========== //

struct Circuit {
    name: String,
    components: Vec<Component>,
}

struct Coordinate {
    x: i32,
    y: i32,
}


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