use std::{error::Error, fmt::Display};

use ::geo_types::{Coord, CoordNum, Point};
use geo::Rect;
use graph::Target;
use num_traits::Num;
use petgraph::{stable_graph::StableGraph, Directed};

pub use geozero::{FeatureProcessor, GeomProcessor, PropertyProcessor};
use qutee::Boundary;

pub mod algorithms;
pub mod builder;
pub mod geo_types;
pub mod graph;
pub mod input;

mod serde;

#[macro_use]
mod macros;
pub mod types;

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

pub trait Coordinate<T: CoordNum + Num = f64> {
    fn x_y(&self) -> (T, T);

    fn zero() -> Self;

    fn as_coord(&self) -> Coord<T>;
}

pub trait Graph<EV, NV> {
    fn node_count(&self) -> usize;

    fn edge_count(&self) -> usize;

    fn neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<EV>>
    where
        EV: 'a;

    fn edges(&self) -> impl Iterator<Item = (usize, usize)> + '_;

    fn degree(&self, node: usize) -> usize;

    fn node_value(&self, node: usize) -> Option<&NV>;

    fn node_values<'a>(&'a self) -> impl Iterator<Item = (usize, &'a NV)>
    where
        NV: 'a;

    fn node_value_mut(&mut self, node: usize) -> Option<&mut NV>;

    fn set_node_value(&mut self, node: usize, value: NV) -> Result<(), GraphError>;

    fn add_node(&mut self, weight: NV) -> usize;

    fn add_edge(&mut self, a: usize, b: usize, weight: EV) -> bool;
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

pub trait CoordGraph<EV, NV: Coordinate<C>, C: CoordNum>: Graph<EV, NV> {
    fn nearest_node(&self, point: &Coord<C>) -> Option<usize>;

    fn nearest_node_bound(&self, point: &Coord<C>, tolerance: C) -> Option<usize>;

    fn bounding_rect(&self) -> Option<Rect<C>>;
}
