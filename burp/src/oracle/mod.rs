use core::fmt;
use std::{
    collections::{HashMap, HashSet},
    f64::{self, consts::E},
    fmt::Debug,
    hash::Hash,
    io::Read,
    sync::{Arc, Mutex, RwLock, RwLockReadGuard},
    usize,
};

use galileo::{
    galileo_types::{
        cartesian::CartesianPoint2d,
        geo::{impls::GeoPoint2d, GeoPoint, NewGeoPoint},
    },
    Map,
};
use geo::{Coord, Point};
use graph_rs::{
    algorithms::dijkstra::{Dijkstra, DijkstraResult, ResultNode},
    graph::{csr::DirectedCsrGraph, quad_tree::QuadGraph},
    CoordGraph, Coordinate, DirectedGraph, Graph,
};
use log::info;
use num_traits::Num;
use ordered_float::{FloatCore, OrderedFloat};
use rayon::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};

use crate::{
    galileo::{GalileoMap, NodeMarker},
    serde::OrderedFloatDef,
    types::{CoordNode, Poi},
};

pub trait NodeTrait: Clone + Debug + Send + Sync {}

type QuadGraphType<T> = QuadGraph<f64, CoordNode<T>, DirectedCsrGraph<f64, CoordNode<T>>>;
type RwLockGraph<T> = RwLock<QuadGraphType<T>>;

#[derive(Serialize, Deserialize)]
pub struct Oracle<T>
where
    T: NodeTrait,
{
    graph: RwLockGraph<T>,
    poi_nodes: HashSet<usize>,
}

impl<T> Oracle<T>
where
    T: NodeTrait + Serialize + DeserializeOwned,
{
    pub fn new(graph: QuadGraphType<T>) -> Self {
        let poi_nodes = graph
            .iter()
            .fold(HashSet::default(), |mut poi_nodes, node| {
                if node.1.has_data() {
                    poi_nodes.insert(node.0);
                }
                poi_nodes
            });
        Self {
            graph: RwLock::new(graph),
            poi_nodes,
        }
    }

    pub fn add_poi(&mut self, mut poi: CoordNode<T>) -> Result<(), Error> {
        let nearest_node;
        {
            nearest_node = self
                .graph
                .read()
                .expect("poisioned lock")
                .nearest_node(poi.as_coord())
                .ok_or(Error::NoValue(format!("quad graph empty")))?;
        }
        {
            let mut graph = self.graph.write().expect("poisioned lock");
            let node = graph
                .node_value_mut(nearest_node)
                .ok_or(Error::NoValue(format!("node: {:?}", nearest_node)))?;

            info!("Found node: {}", &node);

            node.append_data(poi.data_mut());
            self.poi_nodes.insert(nearest_node);
        }

        Ok(())
    }

    pub fn get_node_value_at(
        &self,
        coord: Coord,
        tolerance: f64,
    ) -> Result<(usize, CoordNode<T>), Error> {
        let graph = self.graph.read().expect("poisoned lock");

        let node_id = graph
            .nearest_node_bound(coord, tolerance)
            .ok_or(Error::NoValue(format!(
                "no node at {:?} with tolerance {}",
                &coord.x_y(),
                tolerance
            )))?;

        let node_value = graph
            .node_value(node_id)
            .cloned()
            .ok_or(Error::NoValue(format!("no node with id {}", node_id)))?;

        Ok((node_id, node_value))
    }

    pub fn add_pois(&mut self, pois: &[CoordNode<T>]) -> Result<(), Vec<Error>> {
        pois.iter().for_each(|poi| {
            self.add_poi(poi.to_owned());
        });

        Ok(())
    }

    pub fn to_flexbuffer(&self) -> Vec<u8> {
        let mut ser = flexbuffers::FlexbufferSerializer::new();
        self.serialize(&mut ser).unwrap();

        ser.view().to_vec()
    }

    pub fn read_flexbuffer(f_buf: &[u8]) -> Self {
        let reader = flexbuffers::Reader::get_root(f_buf).unwrap();

        Self::deserialize(reader).unwrap()
    }
}

impl<T: NodeTrait> Oracle<T> {
    pub fn graph(&self) -> RwLockReadGuard<QuadGraphType<T>> {
        self.graph.read().expect("poisoned lock")
    }

    pub fn dijkstra(
        &self,
        start_node: usize,
        targets: HashSet<usize>,
    ) -> Result<DijkstraResult<f64>, String> {
        self.graph().dijkstra(start_node, targets)
    }

    pub fn dijkstra_full(&self, start_node: usize) -> Result<DijkstraResult<f64>, String> {
        self.graph().dijkstra_full(start_node)
    }

    pub fn beer_path_dijkstra(
        &self,
        start_node: usize,
        end_node: usize,
    ) -> Result<BeerPathResult<f64>, String> {
        info!(
            "Calculating {} beer paths between nodes {}, {}",
            self.poi_nodes.len(),
            &start_node,
            &end_node
        );
        let start_result = self.dijkstra(start_node, self.poi_nodes.clone())?;
        let end_result = self.dijkstra(end_node, self.poi_nodes.clone())?;

        Ok(BeerPathResult {
            start_result,
            end_result,
            pois: self.poi_nodes.clone(),
        })
    }
}

impl<T: Debug + NodeTrait> Debug for Oracle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.graph)
    }
}

impl<T> PartialEq for Oracle<T>
where
    T: NodeTrait + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        *self.graph.read().expect("poisoned lock") == *other.graph.read().expect("poisoned lock")
    }
}

impl<T> From<QuadGraphType<T>> for Oracle<T>
where
    T: NodeTrait,
{
    fn from(graph: QuadGraphType<T>) -> Self {
        let poi_nodes = graph
            .iter()
            .fold(HashSet::default(), |mut poi_nodes, node| {
                if node.1.has_data() {
                    poi_nodes.insert(node.0);
                }
                poi_nodes
            });
        Self {
            graph: RwLock::new(graph),
            poi_nodes,
        }
    }
}

pub struct BeerPathResult<T> {
    start_result: DijkstraResult<T>,
    end_result: DijkstraResult<T>,
    pois: HashSet<usize>,
}

impl<T: FloatCore + Debug> BeerPathResult<T> {
    pub fn len(&self) -> usize {
        self.pois.len()
    }
    pub fn path(&self, node: usize) -> Option<Vec<&ResultNode<OrderedFloat<T>>>> {
        if !self.pois.contains(&node) {
            return None;
        }

        let mut start_path = self.start_result.path(node)?;
        let mut end_path = self.end_result.path(node)?;

        end_path.reverse();
        start_path.append(&mut end_path);

        Some(start_path)
    }

    pub fn shortest_path(&self) -> Option<Vec<&ResultNode<OrderedFloat<T>>>> {
        let shortest = self
            .pois
            .iter()
            .fold((0, OrderedFloat(T::zero())), |shortest, poi| {
                if let Some(start) = self.start_result.get(*poi) {
                    if let Some(end) = self.end_result.get(*poi) {
                        let cost = *start.cost() + *end.cost();
                        if shortest.1 > cost {
                            return (*poi, cost);
                        }
                    }
                }
                shortest
            });

        self.path(shortest.0)
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

#[cfg(test)]
mod test {
    use graph_rs::graph::quad_tree::QuadGraph;

    use crate::{
        input::geo_zero::{read_geojson, GraphWriter},
        oracle::{self, Oracle},
        types::Poi,
    };

    #[test]
    fn flexbuffer() {
        let geojson = r#" {
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "geometry": {
                "type": "LineString",
                "coordinates": [
                [
                    13.3530166,
                    52.5365623
                ],
                [
                    13.3531553,
                    52.5364245
                ],
                [
                    13.3538338,
                    52.5364855
                ],
                [
                    13.3542415,
                    52.536498
                ],
                [
                    13.3546724,
                    52.5364904
                ],
                [
                    13.355102,
                    52.5364593
                ]
                ]
            },
            "properties": {
                "osm_id": 54111470,
                "osm_type": "ways_line",
                "tunnel": null,
                "surface": "paving_stones",
                "name": null,
                "width": null,
                "highway": "service",
                "oneway": null,
                "layer": null,
                "bridge": null,
                "smoothness": null
            }
        }]
        }"#;

        let mut graph_writer = GraphWriter::new_from_filter(|_| true);
        assert!(read_geojson(geojson.as_bytes(), &mut graph_writer).is_ok());
        let graph = graph_writer.get_graph();
        let oracle = Oracle::from(QuadGraph::new_from_graph(graph));

        let flexbuff = oracle.to_flexbuffer();

        let oracle2: Oracle<Poi> = Oracle::read_flexbuffer(flexbuff.as_slice());

        assert_eq!(oracle, oracle2);
    }
}
