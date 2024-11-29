use core::fmt;
use std::{
    cmp::{self, Reverse},
    collections::{HashMap, HashSet},
    f64::{self, consts::E},
    fmt::Debug,
    hash::Hash,
    io::Read,
    ops::Deref,
    sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
    thread, usize,
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
    graph::{csr::DirectedCsrGraph, quad_tree::QuadGraph, rstar::RTreeGraph, Target},
    types::Direction,
    CoordGraph, Coordinate, DirectedGraph, Graph,
};
use log::{info, warn};
use ordered_float::{FloatCore, OrderedFloat};
use priority_queue::PriorityQueue;
use rayon::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};

use crate::{
    serde::OrderedFloatDef,
    types::{CoordNode, Poi},
};

pub mod oracle;

pub trait NodeTrait: Clone + Debug + Send + Sync {}

type RTreeGraphType<T> =
    RTreeGraph<f64, CoordNode<f64, T>, DirectedCsrGraph<f64, CoordNode<f64, T>>, f64>;
type RwLockGraph<T> = RwLock<RTreeGraphType<T>>;

#[derive(Serialize, Deserialize)]
pub struct PoiGraph<T>
where
    T: NodeTrait,
{
    graph: RwLockGraph<T>,
    poi_nodes: HashSet<usize>,
}

impl<T> PoiGraph<T>
where
    T: NodeTrait + Serialize + DeserializeOwned,
{
    pub fn new(graph: RTreeGraphType<T>) -> Self {
        let poi_nodes = graph
            .node_values()
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

    pub fn add_poi(&mut self, mut poi: CoordNode<f64, T>) -> Result<(), Error> {
        let nearest_node;
        {
            nearest_node = self
                .graph
                .read()
                .expect("poisioned lock")
                .nearest_node(poi.get_coord())
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
        coord: &Coord<f64>,
        tolerance: f64,
    ) -> Result<(usize, CoordNode<f64, T>), Error> {
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

    pub fn add_pois(&mut self, pois: &[CoordNode<f64, T>]) -> Result<(), Vec<Error>> {
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

    pub fn poi_nodes(&self) -> &HashSet<usize> {
        &self.poi_nodes
    }
}

impl<T: NodeTrait> PoiGraph<T> {
    pub fn graph(&self) -> RwLockReadGuard<RTreeGraphType<T>> {
        self.graph.read().expect("poisoned lock")
    }

    pub fn graph_mut(&self) -> RwLockWriteGuard<RTreeGraphType<T>> {
        self.graph.write().expect("poisoned lock")
    }

    pub fn dijkstra(
        &self,
        start_node: usize,
        targets: HashSet<usize>,
        direction: Direction,
    ) -> Option<DijkstraResult<f64>> {
        self.graph().dijkstra(start_node, targets, direction)
    }

    pub fn dijkstra_full(
        &self,
        start_node: usize,
        direction: Direction,
    ) -> Option<DijkstraResult<f64>> {
        self.graph().dijkstra_full(start_node, direction)
    }

    pub fn beer_path_dijkstra_base(
        &self,
        start_node: usize,
        end_node: usize,
        pois: HashSet<usize>,
    ) -> Option<BeerPathResult<f64>> {
        info!(
            "Calculating {} beer paths between nodes {}, {}",
            self.poi_nodes.len(),
            &start_node,
            &end_node
        );
        let start_result = self.dijkstra(start_node, pois.clone(), Direction::Outgoing)?;
        let end_result = self.dijkstra(end_node, pois.clone(), Direction::Incoming)?;

        Some(BeerPathResult {
            start_result,
            end_result,
            pois,
        })
    }

    pub fn beer_path_dijkstra_fast(
        &self,
        start_id: usize,
        end_id: usize,
        pois_id: &HashSet<usize>,
        epsilon: f64,
    ) -> HashMap<usize, f64> {
        info!(
            "Calculating {} beer paths between nodes {}, {}",
            self.poi_nodes.len(),
            &start_id,
            &end_id
        );
        let mut frontier = PriorityQueue::new();
        let result = Arc::new(RwLock::new(HashMap::new()));
        let visited = Arc::new(RwLock::new(HashMap::new()));
        let bound = Arc::new(RwLock::new(f64::INFINITY));
        let Some(_) = self.graph().node_value(start_id) else {
            warn!("start_id {} not found in graph", start_id);
            return Arc::into_inner(result)
                .expect("More than one strong reference")
                .into_inner()
                .expect("poisioned lock");
        };
        let Some(_) = self.graph().node_value(end_id) else {
            warn!("end_id {} not found in graph", end_id);
            return Arc::into_inner(result)
                .expect("More than one strong reference")
                .into_inner()
                .expect("poisioned lock");
        };
        frontier.push((start_id, Label::Forward), Reverse(OrderedFloat(0.0)));
        frontier.push((end_id, Label::Backward), Reverse(OrderedFloat(0.0)));

        let frontier = Arc::new(RwLock::new(frontier));

        thread::scope(|s| {
            s.spawn(|| {
                shared_dijkstra(
                    self.graph().deref(),
                    start_id,
                    Label::Forward,
                    visited.clone(),
                    pois_id,
                    result.clone(),
                    bound.clone(),
                    epsilon,
                )
            });

            s.spawn(|| {
                shared_dijkstra(
                    self.graph().deref(),
                    end_id,
                    Label::Backward,
                    visited.clone(),
                    pois_id,
                    result.clone(),
                    bound.clone(),
                    epsilon,
                )
            });
        });
        info!("finished beer paths");
        Arc::into_inner(result)
            .expect("More than one strong reference")
            .into_inner()
            .expect("poisioned lock")
    }
}
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
enum Label {
    Forward,
    Backward,
    Poi,
}

impl Label {
    fn inverse(&self) -> Self {
        match self {
            Label::Forward => Label::Backward,
            Label::Backward => Label::Forward,
            Label::Poi => Label::Poi,
        }
    }
}

fn shared_dijkstra<EV, NV, G>(
    graph: &G,
    start_node: usize,
    direction: Label,
    visited: Arc<RwLock<HashMap<(usize, Label), OrderedFloat<EV>>>>,
    targets: &HashSet<usize>,
    result: Arc<RwLock<HashMap<usize, EV>>>,
    bound: Arc<RwLock<EV>>,
    epsilon: EV,
) where
    EV: FloatCore + Send + Sync + Debug,
    NV: Send + Sync,
    G: DirectedGraph<EV, NV>,
{
    let mut frontier = PriorityQueue::new();
    frontier.push((start_node, direction), Reverse(OrderedFloat(EV::zero())));
    let mut next_node = frontier.pop();
    while let Some(node) = next_node.take() {
        if node.1 .0 .0 > *bound.read().expect("poisoned lock") {
            info!("Bound exceded. Stoping.");
            break;
        }

        if matches!(node.0 .1, Label::Poi) {
            result
                .write()
                .expect("poisoned lock")
                .insert(node.0 .0, *node.1 .0);
            next_node = frontier.pop();
            continue;
        }

        if visited
            .read()
            .expect("poisoned lock")
            .get(&(node.0 .0, direction))
            .is_some()
        {
            next_node = frontier.pop();
            continue;
        }
        if let Some(visited_node) = visited
            .read()
            .expect("poisoned lock")
            .get(&(node.0 .0, direction.inverse()))
        {
            let distance = node.1 .0 + *visited_node;
            let mut bound = bound.write().expect("poisoned lock");

            *bound = *cmp::min(
                OrderedFloat(*bound),
                distance * OrderedFloat(EV::from(1.0).unwrap() + epsilon),
            );

            if targets.contains(&node.0 .0) {
                frontier.push((node.0 .0, Label::Poi), Reverse(distance));
            }
        }

        visited
            .write()
            .expect("poisoned lock")
            .insert((node.0 .0, direction), node.1 .0);

        let neighbours: Box<dyn Iterator<Item = &Target<EV>>> = match node.0 .1 {
            Label::Forward => Box::new(graph.neighbors(node.0 .0)),
            Label::Backward => Box::new(graph.neighbors(node.0 .0)),
            _ => continue,
        };

        neighbours.for_each(|n| {
            let path_cost = OrderedFloat(*node.1 .0 + *n.value());
            let new_node = (n.target(), node.0 .1);
            if frontier.change_priority_by(&new_node, |p| {
                if p.0 > path_cost {
                    p.0 = path_cost
                }
            }) {
                return;
            }
            frontier.push(new_node, Reverse(path_cost));
        });

        next_node = frontier.pop();
    }
}

impl<T: Debug + NodeTrait> Debug for PoiGraph<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.graph)
    }
}

impl<T> PartialEq for PoiGraph<T>
where
    T: NodeTrait + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        *self.graph.read().expect("poisoned lock") == *other.graph.read().expect("poisoned lock")
    }
}

impl<T> From<RTreeGraphType<T>> for PoiGraph<T>
where
    T: NodeTrait,
{
    fn from(graph: RTreeGraphType<T>) -> Self {
        let poi_nodes = graph
            .node_values()
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

#[derive(PartialEq, Debug)]
pub struct BeerPathResult<T: FloatCore> {
    start_result: DijkstraResult<T>,
    end_result: DijkstraResult<T>,
    pois: HashSet<usize>,
}

impl<T: FloatCore + Debug> BeerPathResult<T> {
    pub fn len(&self) -> usize {
        self.pois.len()
    }
    pub fn path(&self, node: usize) -> Option<Vec<&ResultNode<T>>> {
        if !self.pois.contains(&node) {
            return None;
        }

        let mut start_path = self.start_result.path(node)?;
        let mut end_path = self.end_result.path(node)?;

        end_path.reverse();
        start_path.append(&mut end_path);

        Some(start_path)
    }

    pub fn shortest_path(&self) -> Option<Vec<&ResultNode<T>>> {
        let shortest = self.pois.iter().fold((0, T::zero()), |shortest, poi| {
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
    use graph_rs::graph::{quad_tree::QuadGraph, rstar::RTreeGraph};

    use crate::{
        graph::{self, PoiGraph},
        input::geo_zero::{read_geojson, GraphWriter},
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

        let mut graph_writer = GraphWriter::new(|_| true);
        assert!(read_geojson(geojson.as_bytes(), &mut graph_writer).is_ok());
        let graph = graph_writer.get_graph();
        let oracle = PoiGraph::from(RTreeGraph::new_from_graph(graph));

        let flexbuff = oracle.to_flexbuffer();

        let oracle2: PoiGraph<Poi> = PoiGraph::read_flexbuffer(flexbuff.as_slice());

        assert_eq!(oracle, oracle2);
    }
}
