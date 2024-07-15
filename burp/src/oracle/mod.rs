use core::fmt;
use std::fmt::Debug;

use graph_rs::{
    graph::{csr::DirectedCsrGraph, quad_tree::QuadGraph},
    CoordGraph, Coordinate, DirectedGraph, Graph,
};
use log::info;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::types::{CoordNode, Poi};

#[derive(Serialize, Deserialize)]
pub struct Oracle<T, G>
where
    T: Send + Sync + Clone + Debug,
    G: DirectedGraph<f64, CoordNode<T>>,
    QuadGraph<f64, CoordNode<T>, G>: DeserializeOwned,
{
    graph: QuadGraph<f64, CoordNode<T>, G>,
}

impl<T, G> Oracle<T, G>
where
    T: Send + Sync + Clone + Debug,
    G: DirectedGraph<f64, CoordNode<T>>,
    QuadGraph<f64, CoordNode<T>, G>: DeserializeOwned,
{
    pub fn new(graph: G) -> Self {
        Self {
            graph: QuadGraph::new_from_graph(graph),
        }
    }

    pub fn add_poi(&self, poi: CoordNode<T>) -> Result<(), Error> {
        let nearest_node = self.graph.nearest_node(poi.as_coord());
        let mut node = self
            .graph
            .node_value_mut(nearest_node)
            .ok_or(Error::NoValue(format!("node: {:?}", nearest_node)))?;

        info!("Found node \'{}\' with coords {}", nearest_node, node);

        node.set_data(*poi.data());

        Ok(())
    }

    pub fn load_pois(&self, pois: Vec<CoordNode<T>>) -> Result<(), Vec<Error>> {
        let results = Vec::default();
        for poi in pois {
            results.push(tokio::spawn(async { self.add_poi(poi) }));
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    NoValue(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoValue(o) => write!(f, "No value for {:?}", o),
        }
    }
}

impl std::error::Error for Error {}
