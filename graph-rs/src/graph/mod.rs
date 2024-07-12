use std::{hash::Hash, usize};

use serde::{Deserialize, Serialize};

pub mod csr;
pub mod quad_tree;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Target<EV> {
    target: usize,
    value: EV,
}

impl<EV> Target<EV> {
    pub fn new(target: usize, value: EV) -> Target<EV> {
        Self { target, value }
    }

    pub fn target(&self) -> usize {
        self.target
    }

    pub fn value(&self) -> &EV {
        &self.value
    }
}

impl<EV> Hash for Target<EV> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.target.hash(state)
    }
}

impl<EV> PartialEq for Target<EV> {
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target
    }
}

impl<EV> Eq for Target<EV> {}

impl Target<()> {
    pub fn new_without_value(target: usize) -> Target<()> {
        self::Target::new(target, ())
    }
}
