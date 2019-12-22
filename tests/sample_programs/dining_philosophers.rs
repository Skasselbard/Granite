use std::sync::{Arc, Mutex};
use std::{thread, time};

struct Philosopher {
    left_fork: Arc<Mutex<Fork>>,
    right_fork: Arc<Mutex<Fork>>,
}

struct Fork;

impl Philosopher {
    pub fn run(&self, dinner_count: usize) {
        for _ in 0..dinner_count {
            let lf;
            while true {
                let result = self.left_fork.lock();
                if result.is_ok() {
                    lf = result.unwrap();
                    break;
                }
            }
            {
                let rf;
                while true {
                    let result = self.right_fork.lock();
                    if result.is_ok() {
                        rf = result.unwrap();
                        break;
                    }
                }
                thread::sleep(time::Duration::from_millis(100));
            } // right fork put down
        } // left fork put dow
    }
}

fn main() {
    let mut forks = Vec::new();
    let mut philosophers = Vec::new();
    for _ in 0..5 {
        let fork = Arc::new(Mutex::new(Fork {}));
        forks.push(fork.clone());
    }
    for index in 0..5 {
        let left_fork = forks.get(index).unwrap().clone();
        let right_fork = forks.get((index + 1) % 5).unwrap().clone();
        philosophers.push(Philosopher {
            left_fork,
            right_fork,
        });
    }
    for philosopher in philosophers {
        thread::spawn(move || philosopher.run(10))
            .join()
            .expect("thread::spawn failed");
    }
}
