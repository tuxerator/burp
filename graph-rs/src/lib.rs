use std::usize;

use geo::{Coord, Point};
use graph::Target;
use petgraph::{stable_graph::StableGraph, Directed};

pub use geozero::{FeatureProcessor, GeomProcessor, PropertyProcessor};

pub mod builder;
pub mod graph;
pub mod input;

pub trait Graph<EV, NV>: Sync {
    fn node_count(&self) -> usize;

    fn edge_count(&self) -> usize;

    fn neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<EV>>
    where
        EV: 'a;

    fn degree(&self, node: usize) -> usize;

    fn node_value(&self, node: usize) -> Option<&NV>;

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

pub trait CoordGraph<EV>: Graph<EV, Coord> {
    fn nearest_node(&self, point: Point) -> usize;
}
