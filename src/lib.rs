use std::usize;

pub mod graph;

pub trait Graph {
    fn node_count(&self) -> usize;

    fn edge_count(&self) -> usize;
}

pub trait DirectedNeighbors {}
