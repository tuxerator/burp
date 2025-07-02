use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

use num_traits::Num;
use serde::{Deserialize, Serialize};

use crate::{EdgeTrait, NodeTrait};

pub mod csr;
pub mod node;
pub mod rstar;

#[derive(Clone, Copy, Debug)]
pub struct Node<NV> {
    id: usize,
    weight: NV,
}

impl<NV> Node<NV> {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn weight(&self) -> &NV {
        &self.weight
    }

    pub fn weight_mut(&mut self) -> &mut NV {
        &mut self.weight
    }

    pub fn id_mut(&mut self) -> &mut usize {
        &mut self.id
    }

    pub fn set_id(&mut self, id: usize) {
        self.id = id;
    }
}

impl<NV> PartialEq for Node<NV> {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl<NV> NodeTrait<NV> for Node<NV> {
    fn index(&self) -> usize {
        self.id
    }

    fn weight(&self) -> &NV {
        &self.weight
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Target<EV> {
    target: usize,
    value: EV,
}

#[derive(Debug)]
pub struct Edge<'a, EV, NV> {
    start: &'a Node<NV>,
    target: &'a Node<NV>,
    weight: EV,
}

impl<'a, EV, NV> Edge<'a, EV, NV> {
    pub fn target_mut(&mut self) -> &mut &'a Node<NV> {
        &mut self.target
    }

    pub fn set_target(&mut self, target: &'a Node<NV>) {
        self.target = target;
    }
}

impl<EV, NV> PartialEq for Edge<'_, EV, NV> {
    fn eq(&self, other: &Self) -> bool {
        (self.start() == other.start()) && (self.target() == other.target())
    }
}

impl<EV, NV> EdgeTrait<EV, NV> for Edge<'_, EV, NV> {
    type Node = Node<NV>;
    fn start(&self) -> &Self::Node {
        self.start
    }

    fn target(&self) -> &Self::Node {
        self.target
    }

    fn weight(&self) -> &EV {
        &self.weight
    }
}

// #[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Path<'a, EV, NV> {
    path: Vec<Edge<'a, EV, NV>>,
}

impl<'a, EV: Default, NV> Path<'a, EV, NV>
where
    NV: PartialEq,
{
    pub fn new(path: Vec<Edge<'a, EV, NV>>) -> Self {
        Self { path }
    }

    pub fn push(&mut self, edge: Edge<'a, EV, NV>) -> Result<(), PathError<'a, EV, NV>> {
        if self.last_node().unwrap_or(edge.start()) != edge.start() {
            return Err(PathError::NoConnectionError(
                self.last_node().expect("This is a bug!").id(),
                edge,
            ));
        }
        self.path.push(edge);
        Ok(())
    }

    pub fn last(&self) -> Option<&Edge<'a, EV, NV>> {
        self.path.last()
    }

    pub fn last_node(&mut self) -> Option<&Node<NV>> {
        self.path.last().map(|e| e.target())
    }
}

impl<'a, EV: Num + Copy, NV> Path<'a, EV, NV> {
    pub fn pop(&mut self) -> Option<Edge<'a, EV, NV>> {
        self.path.pop()
    }

    pub fn cost(&self) -> EV {
        self.path.iter().fold(EV::zero(), |c, n| c + n.weight)
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

pub enum PathError<'a, EV, NV> {
    NoConnectionError(usize, Edge<'a, EV, NV>),
}

default impl<EV, NV> Display for PathError<'_, EV, NV> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Edge does not connect to path end.")
    }
}
impl<EV, NV> Display for PathError<'_, EV, NV>
where
    EV: Debug,
    NV: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoConnectionError(node, edge) => {
                write!(f, "Edge {edge:?} does not connect to path end {node:?}.")
            }
        }
    }
}
