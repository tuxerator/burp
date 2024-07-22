use core::fmt;
use std::{
    f64::consts::E,
    fmt::Debug,
    io::Read,
    sync::{Arc, Mutex, RwLock},
};

use galileo::{
    galileo_types::{
        cartesian::CartesianPoint2d,
        geo::{impls::GeoPoint2d, GeoPoint, NewGeoPoint},
    },
    Map,
};
use geo::Point;
use graph_rs::{
    graph::{csr::DirectedCsrGraph, quad_tree::QuadGraph},
    CoordGraph, Coordinate, DirectedGraph, Graph,
};
use log::info;
use rayon::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};

use crate::{
    galileo::{GalileoMap, NodeMarker},
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

    #[serde(skip)]
    map: Option<GalileoMap>,
}

impl<T> Oracle<T>
where
    T: NodeTrait + Serialize + DeserializeOwned,
{
    pub fn new(graph: QuadGraphType<T>, map: Arc<RwLock<Map>>) -> Self {
        let map = GalileoMap::new(map);
        map.draw_coord_graph(&graph);

        Self {
            graph: RwLock::new(graph),
            map: Some(map),
        }
    }

    pub fn add_poi(&self, mut poi: CoordNode<T>) -> Result<(), Error> {
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

            if let Some(ref map) = self.map {
                let feature = NodeMarker::new(
                    NewGeoPoint::latlon(node.get_coord().lat(), node.get_coord().lon()),
                    nearest_node,
                );

                map.draw_node(feature);
            }
        }

        Ok(())
    }

    pub fn get_node_value_at(&self, point: Point, tolerance: f64) -> Result<CoordNode<T>, Error> {
        let graph = self.graph.read().expect("poisoned lock");

        let node_id = graph
            .nearest_node_bound(point.into(), tolerance)
            .ok_or(Error::NoValue(format!(
                "no node at {:?} with tolerance {}",
                &point.x_y(),
                tolerance
            )))?;

        graph
            .node_value(node_id)
            .cloned()
            .ok_or(Error::NoValue(format!("no node with id {}", node_id)))
    }

    pub fn add_pois(&mut self, pois: &[CoordNode<T>]) -> Result<(), Vec<Error>> {
        let self_arc = Arc::new(self);
        pois.par_iter().for_each(|poi| {
            self_arc.add_poi(poi.to_owned());
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

    pub fn draw_to_map(&mut self, map: Arc<RwLock<Map>>) {
        let map = GalileoMap::new(map);
        let graph = self.graph.read().expect("poisoned lock");

        map.draw_coord_graph(&*graph);
        self.map = Some(map);
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
        Self {
            graph: RwLock::new(graph),
            map: None,
        }
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
