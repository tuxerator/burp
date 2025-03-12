use std::{error::Error, fmt::Display};

use ::geo_types::{Coord, CoordNum, CoordinateType, Point};
use geo::{CoordinatePosition, Rect};
use graph::Target;
use num_traits::Num;

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

pub trait Graph: Default {
    type EV;
    type NV;
    fn node_count(&self) -> usize;

    fn edge_count(&self) -> usize;

    fn neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<Self::EV>>
    where
        Self::EV: 'a;

    fn edges(&self) -> impl Iterator<Item = (usize, usize)> + '_;

    fn degree(&self, node: usize) -> usize;

    fn node_value(&self, node: usize) -> Option<&Self::NV>;

    fn nodes_iter<'a>(&'a self) -> impl Iterator<Item = (usize, &'a Self::NV)>
    where
        Self::NV: 'a;

    fn node_value_mut(&mut self, node: usize) -> Option<&mut Self::NV>;

    fn set_node_value(&mut self, node: usize, value: Self::NV) -> Result<(), GraphError>;

    fn add_node(&mut self, weight: Self::NV) -> usize;

    fn add_edge(&mut self, a: usize, b: usize, weight: Self::EV) -> bool;

    fn remove_node(&mut self, node: usize) -> Option<Self::NV>;

    fn remove_edge(&mut self, edge: (usize, usize)) -> Option<Self::EV>;
}

pub trait DirectedGraph: Graph {
    fn out_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<Self::EV>>
    where
        Self::EV: 'a;

    fn in_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<Self::EV>>
    where
        Self::EV: 'a;

    fn out_degree(&self, node: usize) -> usize;

    fn in_degree(&self, node: usize) -> usize;
}

pub trait CoordGraph: Graph {
    type C: CoordNum;
    fn nearest_node(&self, point: &Coord<Self::C>) -> Option<usize>;

    fn nearest_node_bound(&self, point: &Coord<Self::C>, tolerance: Self::C) -> Option<usize>;

    fn bounding_rect(&self) -> Option<Rect<Self::C>>;
}
