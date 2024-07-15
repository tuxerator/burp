use std::{error::Error, fmt::Display, usize};

use ::geo_types::{Coord, CoordNum, Point};
use geozero::geo_types;
use graph::Target;
use petgraph::{stable_graph::StableGraph, Directed};

pub use geozero::{FeatureProcessor, GeomProcessor, PropertyProcessor};

pub mod builder;
pub mod graph;
pub mod input;

mod serde;

#[macro_use]
mod macros;

#[derive(Debug)]
pub enum GraphError {
    NodeNotFound(usize),
    EmptyNode(usize),
}

impl Error for GraphError {}

impl Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeNotFound(node) => write!(f, "node_id: {} not found in graph", node),
            Self::EmptyNode(node) => write!(f, "node \'{}\' has no acociated value", node),
        }
    }
}

pub trait Coordinate<T: CoordNum = f64> {
    fn x_y(&self) -> (T, T);

    fn zero() -> Self;

    fn as_coord(&self) -> Coord<T>;
}

pub trait Graph<EV, NV>: Sync {
    fn node_count(&self) -> usize;

    fn edge_count(&self) -> usize;

    fn neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<EV>>
    where
        EV: 'a;

    fn degree(&self, node: usize) -> usize;

    fn node_value(&self, node: usize) -> Option<&NV>;

    fn node_value_mut(&mut self, node: usize) -> Option<&mut NV>;

    fn set_node_value(&mut self, node: usize, value: NV) -> Result<(), GraphError>;

    fn to_stable_graph(&self) -> StableGraph<Option<NV>, EV, Directed, usize>;
}

pub trait DirectedGraph<EV, NV>: Graph<EV, NV> {
    fn out_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<EV>>
    where
        EV: 'a;

    fn in_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<EV>>
    where
        EV: 'a;

    fn out_degree(&self, node: usize) -> usize;

    fn in_degree(&self, node: usize) -> usize;
}

pub trait CoordGraph<EV, NV: Coordinate>: Graph<EV, NV> {
    fn nearest_node(&self, point: Coord) -> usize;
}
