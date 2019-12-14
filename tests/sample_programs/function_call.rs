pub fn main() {
    let mut x = 5;
    x = call(x);
}

fn call(i: usize) -> usize {
    i * 2
}
