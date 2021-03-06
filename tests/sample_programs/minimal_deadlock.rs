//https://stackoverflow.com/a/55959254
use std::sync::{Arc, Mutex};

pub fn main() {
    let data = Arc::new(Mutex::new(0));
    let _d1 = data.lock();
    let _d2 = data.lock(); // cannot lock, since d1 is still active
}
