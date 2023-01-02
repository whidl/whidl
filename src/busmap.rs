use crate::simulator::Bus;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Write;

// Convenience for creating a bus with width 1
impl From<String> for Bus {
    fn from(name: String) -> Self {
        Bus { name, range: None }
    }
}

impl From<&str> for Bus {
    fn from(name: &str) -> Self {
        Bus::from(String::from(name))
    }
}

impl<const N: usize> TryFrom<[(&str, bool); N]> for BusMap {
    type Error = String;

    fn try_from(values: [(&str, bool); N]) -> Result<Self, String> {
        let x = values.map(|(name, val)| {
            (
                Bus {
                    name: String::from(name),
                    range: None,
                },
                vec![val],
            )
        });

        let mut r = BusMap::new();
        for (b, v) in x {
            r.create_bus(&b.name, N)?;
            r.insert(b, v);
        }
        Ok(r)
    }
}

impl<const N: usize> TryFrom<[(&str, Vec<bool>); N]> for BusMap {
    type Error = String;

    fn try_from(values: [(&str, Vec<bool>); N]) -> Result<Self, String> {
        let x = values.map(|(name, val)| {
            (
                Bus {
                    name: String::from(name),
                    range: None,
                },
                val,
            )
        });

        let mut r = BusMap::new();
        for (b, v) in x {
            r.create_bus(&b.name, v.len())?;
            r.insert(b, v);
        }
        Ok(r)
    }
}

impl TryFrom<HashMap<String, Vec<bool>>> for BusMap {
    type Error = String;
    fn try_from(values: HashMap<String, Vec<bool>>) -> Result<Self, String> {
        let mut r = BusMap::new();
        for (b, v) in values {
            r.create_bus(&b, v.len())?;
            r.insert(
                Bus {
                    name: b,
                    range: None,
                },
                v,
            );
        }
        Ok(r)
    }
}

impl PartialOrd for BusMap {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut less = true;
        let mut greater = true;

        for k in other.buses.keys() {
            if self.buses.get(k) != other.buses.get(k) {
                greater = false;
            }
        }

        for k in self.buses.keys() {
            if self.buses.get(k) != other.buses.get(k) {
                less = false;
            }
        }

        if less && greater {
            Some(Ordering::Equal)
        } else if less {
            Some(Ordering::Less)
        } else if greater {
            Some(Ordering::Greater)
        } else {
            None
        }
    }
}

impl std::fmt::Debug for BusMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted: Vec<(String, String)> = self
            .buses
            .iter()
            .map(|(key, val)| {
                let k = key.clone();
                let v = val
                    .iter()
                    .map(|x| match x {
                        None => String::from("?"),
                        Some(true) => String::from("1"),
                        Some(false) => String::from("0"),
                    })
                    .collect();
                (k, v)
            })
            .collect();
        write!(f, "{:?}", formatted)
    }
}

impl std::fmt::Display for BusMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted: Vec<(String, String)> = self
            .buses
            .iter()
            .map(|(key, val)| {
                let k = key.clone();
                let v = val
                    .iter()
                    .map(|x| match x {
                        None => String::from("?"),
                        Some(true) => String::from("1"),
                        Some(false) => String::from("0"),
                    })
                    .collect();
                (k, v)
            })
            .collect();

        let mut s = String::new();
        for sig in &formatted {
            writeln!(&mut s, "{}: {}", sig.0, sig.1)?;
        }

        write!(f, "{}", s)
    }
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, Clone)]
pub struct BusMap {
    buses: BTreeMap<String, Box<[Option<bool>]>>,
}

impl Default for BusMap {
    fn default() -> Self {
        BusMap::new()
    }
}

impl BusMap {
    pub fn new() -> BusMap {
        BusMap {
            buses: BTreeMap::new(),
        }
    }

    // Returns a copy if we have all the values for the bus.
    // For dry-run maps returns false values for everything requested.
    pub fn get_bus(&self, bus: &Bus) -> Vec<Option<bool>> {
        let msg = &format!(
            "Attempt to get wire {:?} but we don't have that name.",
            &bus.name
        );
        let current = self.buses.get(&bus.name).expect(msg);

        let range = match &bus.range {
            None => 0..current.len(),
            Some(r) => r.clone(),
        };

        let current_length = current.len();
        if range.end > current_length {
            panic!(
                "Attempt to use range {:?} outside of the declared bus width {:?}.",
                range.end, current_length
            );
        }

        let mut res = vec![None; range.len()];

        // Bus indices are zero-indexed from the right.
        let mut reversal = (0..current_length).collect::<Vec<usize>>();
        reversal.reverse();
        for i in range.clone() {
            res[i - range.start] = current[reversal[i]];
        }
        res.reverse();

        res
    }

    pub fn get_name(&self, name: &str) -> Vec<Option<bool>> {
        self.buses.get(name).unwrap().to_vec()
    }

    pub fn insert(&mut self, bus: Bus, values: Vec<bool>) {
        if let Some(r) = &bus.range {
            if r.len() != values.len() {
                panic!("busmap insert: inconsistent widths");
            }
        };

        let x: Vec<Option<bool>> = values.iter().map(|x| Some(*x)).collect();
        self.insert_option(&bus, x);
    }

    /// A bus must be created before it can be used.
    pub fn create_bus(&mut self, name: &str, width: usize) -> Result<(), String> {
        if !self.buses.contains_key(name) {
            self.buses
                .insert(name.to_string(), vec![None; width].into_boxed_slice());
        } else {
            let current = self.buses.get_mut(name).unwrap();
            if current.len() != width {
                return Err(format!(
                    "Inconsistent width for signal {}. Current width: {}, asked for: {}",
                    name,
                    current.len(),
                    width
                ));
            }
        }
        Ok(())
    }

    /// Inserts bus values and merges with existing value for bus. Overwrites wire numbers.
    pub fn insert_option(&mut self, bus: &Bus, values: Vec<Option<bool>>) {
        if !self.buses.contains_key(&bus.name) {
            panic!("Attempt to use a bus that has not been created yet.");
        }
        let current = self.buses.get_mut(&bus.name).unwrap();

        let range = match &bus.range {
            None => 0..values.len(),
            Some(r) => {
                if r.len() != values.len() {
                    panic!("busmap insert: inconsistent widths");
                } else {
                    r.clone()
                }
            }
        };

        let current_length = current.len();
        if range.end > current_length {
            panic!("Attempt to use range outside of the declared bus width.");
        }

        // Bus indices are zero-indexed from the right.
        let mut reversal = (0..current_length).collect::<Vec<usize>>();
        reversal.reverse();
        let mut rvalues = values;
        rvalues.reverse();
        for i in range.clone() {
            current[reversal[i]] = rvalues[i - range.start];
        }
    }

    pub fn get_width(&self, name: &str) -> Option<usize> {
        self.buses.get(name).map(|x| x.len())
    }

    pub fn signals(&self) -> Vec<String> {
        self.buses.keys().cloned().collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_busmap_from() {
        let b = BusMap::try_from([("a", false)]).expect("Error creating bus.");
        assert_eq!(b.get_bus(&Bus::from("a")), vec![Some(false)]);
    }
}
