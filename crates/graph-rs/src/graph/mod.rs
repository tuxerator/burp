use std::hash::Hash;

use num_traits::Num;
use serde::{Deserialize, Serialize};

pub mod csr;
pub mod rstar;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Target<EV> {
    target: usize,
    value: EV,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Path<EV> {
    pub start: Option<Target<EV>>,
    pub path: Vec<Target<EV>>,
}

impl<EV: Default> Path<EV> {
    pub fn new(start: usize, path: Vec<Target<EV>>) -> Self {
        let start = Some(Target::new(start, EV::default()));
        Self { start, path }
    }

    pub fn push(&mut self, target: Target<EV>) {
        self.path.push(target);
    }

    pub fn last(&self) -> Option<&Target<EV>> {
        if self.path.is_empty() {
            return self.start.as_ref();
        }
        self.path.last()
    }

    pub fn last_node(&mut self) -> Option<usize> {
        if self.path.is_empty() {
            return self.start.take().map(|n| n.target());
        }
        self.path.last().map(|n| n.target())
    }
}

impl<EV: Num + Copy> Path<EV> {
    pub fn pop(&mut self) -> Option<Target<EV>> {
        if self.path.is_empty() {
            return self.start.take();
        }
        self.path.pop()
    }

    pub fn cost(&self) -> EV {
        self.path.iter().fold(EV::zero(), |c, n| c + n.value)
    }
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
