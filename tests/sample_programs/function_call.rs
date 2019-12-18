pub fn main() {
    let mut x = 5;
    x = call(x);
}

fn call(i: usize) -> usize {
    // generates an assert terminator
    i * 2
}
