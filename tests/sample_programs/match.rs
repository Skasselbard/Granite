use std::env;

fn main() {
    let x = match env::args().len() {
        1 => 1,
        2 => 2,
        3 => 3,
        _ => 42,
    };
}
