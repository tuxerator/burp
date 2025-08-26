use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

use geo::LineString;
use num_traits::Num;
use serde::{Deserialize, Serialize};

use crate::{CoordGraph, EdgeTrait, NodeTrait};

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

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Path<EV> {
    pub path: Vec<Target<EV>>,
}

impl<EV> Path<EV> {
    pub fn new(path: Vec<Target<EV>>) -> Self {
        Self { path }
    }

    pub fn push(&mut self, target: Target<EV>) {
        self.path.push(target);
    }

    pub fn last(&self) -> Option<&Target<EV>> {
        self.path.last()
    }

    pub fn last_node(&mut self) -> Option<usize> {
        self.path.last().map(|e| e.target())
    }

    /// Creates a `geo::LineString` from `Path<EV>` using the given `CoordGraph`.
    ///
    /// Retruns `None` when in case no coordinate could be retrived for one or more nodes.
    pub fn line_string<G: CoordGraph>(&self, graph: &G) -> Option<LineString<G::C>> {
        let mut coords = Vec::new();

        for node in self.path.iter() {
            let coord = graph.node_coord(node.target())?;
            coords.push(coord);
        }

        Some(LineString::new(coords))
    }
}

impl<EV: Num + Copy> Path<EV> {
    pub fn pop(&mut self) -> Option<Target<EV>> {
        self.path.pop()
    }

    /// Retruns the cost of the [Path] asuming the values of each [Target]
    /// are the cost from the first node in the path.
    pub fn cost(&self) -> EV {
        self.path.last().map(|e| *e.value()).unwrap_or(EV::zero())
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
