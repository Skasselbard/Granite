use log::warn;
use petri_to_star::{NodeRef, PetriNet, PlaceRef, Result};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;

use super::function::{Function, Local};

#[derive(Debug)]
pub struct MutexList {
    list: Vec<Mutex>,
    links: HashMap<Local, MutexRef>,
    guards: HashMap<Local, MutexRef>,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct MutexRef {
    index: usize,
}

#[derive(Debug)]
pub struct Mutex {
    uninitialized: NodeRef,
    unlocked: NodeRef,
    locked: NodeRef,
    dead: NodeRef,
}

impl MutexRef {
    pub fn uninitialized(&self, list: &MutexList) -> NodeRef {
        list.list
            .get(self.index)
            .expect("mutex not found")
            .uninitialized
    }
    pub fn unlocked(&self, list: &MutexList) -> NodeRef {
        list.list.get(self.index).expect("mutex not found").unlocked
    }
    pub fn locked(&self, list: &MutexList) -> NodeRef {
        list.list.get(self.index).expect("mutex not found").locked
    }
    pub fn dead(&self, list: &MutexList) -> NodeRef {
        list.list.get(self.index).expect("mutex not found").dead
    }
}

impl MutexList {
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            links: HashMap::new(),
            guards: HashMap::new(),
        }
    }

    pub fn get_linked(&mut self, local: Local) -> Option<&MutexRef> {
        self.links.get(&local)
    }

    pub fn add(&mut self, net: &mut PetriNet) -> Result<MutexRef> {
        let index = self.list.len();
        let uninitialized = net.add_place();
        uninitialized.name(net, format!("Mutex_{} uninitialized", index))?;
        PlaceRef::try_from(uninitialized)?.marking(net, 1)?;
        let locked = net.add_place();
        locked.name(net, format!("Mutex_{} locked", index))?;
        let unlocked = net.add_place();
        unlocked.name(net, format!("Mutex_{} unlocked", index))?;
        let dead = net.add_place();
        dead.name(net, format!("Mutex_{} dead", index))?;
        self.list.push(Mutex {
            uninitialized,
            unlocked,
            locked,
            dead,
        });
        Ok(MutexRef { index })
    }

    pub fn add_guard(&mut self, guard: Local, mutex: MutexRef) {
        self.guards.insert(guard, mutex);
    }

    pub fn is_linked(&self, local: Local) -> Option<&MutexRef> {
        self.links.get(&local)
    }

    pub fn link(&mut self, local: Local, mutex: MutexRef) {
        match self.links.insert(local, mutex) {
            None => {}
            Some(old_mutex) => {
                if old_mutex != mutex {
                    warn!("Local '{:?}' was already linked to mutex '{:?}'. The old value will be overridden with mutex '{:?}'", local, old_mutex, mutex)
                }
            }
        };
    }
}
