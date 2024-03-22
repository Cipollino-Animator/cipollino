
pub mod curve;
pub mod geo;
pub mod ui;
pub mod color;
pub mod fs;

pub fn next_unique_name<'a, T>(name: &String, names: T) -> String where T: Iterator<Item = &'a str> + Clone {
    if names.clone().position(|other_name| other_name == name).is_none() {
        return name.clone();
    }

    for i in 1.. {
        let potential_name = format!("{} ({})", name, i);
        if names.clone().position(|other_name| other_name == &potential_name).is_none() {
            return potential_name;
        }
    }

    "".to_owned()
}
