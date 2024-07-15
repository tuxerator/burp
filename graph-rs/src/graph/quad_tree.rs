use std::{
    fmt::{self, Debug},
    marker::PhantomData,
};

use geo::{
    point, Coord, CoordNum, EuclideanDistance, GeodesicDestination, GeodesicDistance,
    HaversineDestination, HaversineDistance, Point, RhumbDestination, Translate, VincentyDistance,
};
use log::info;
use ordered_float::OrderedFloat;
use qutee::{Boundary, DynCap, QuadTree};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::{CoordGraph, Coordinate, DirectedGraph, Graph};

use super::csr::DirectedCsrGraph;

#[derive(Serialize, PartialEq, Debug)]
pub struct QuadGraph<EV, NV, G>
where
    G: Graph<EV, NV>,
    NV: Coordinate + Debug,
{
    graph: G,

    #[serde(skip_serializing)]
    quad_tree: Box<QuadTree<f64, usize>>,

    #[serde(skip_serializing)]
    _marker_0: PhantomData<EV>,

    #[serde(skip_serializing)]
    _marker_1: PhantomData<NV>,
}

impl<EV, NV, G> QuadGraph<EV, NV, G>
where
    G: DirectedGraph<EV, NV>,
    NV: Coordinate + Debug,
{
    pub fn new_from_graph(graph: G) -> Self {
        let mut quad_tree = Box::new(QuadTree::new_with_dyn_cap(
            Boundary::new((-180., -90.), 360., 180.),
            20,
        ));

        for node in 0..graph.node_count() {
            quad_tree.insert_at(
                graph.node_value(node).expect("no value for node").x_y(),
                node,
            );
        }

        info!("Created quad-tree");

        Self {
            graph,
            quad_tree,
            _marker_0: PhantomData,
            _marker_1: PhantomData,
        }
    }

    pub fn graph(&self) -> &G {
        &self.graph
    }
}

impl<EV, NV, G> Graph<EV, NV> for QuadGraph<EV, NV, G>
where
    G: Graph<EV, NV>,
    NV: Coordinate + Debug,
{
    fn degree(&self, node: usize) -> usize {
        self.graph.degree(node)
    }

    fn neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a super::Target<EV>>
    where
        EV: 'a,
    {
        self.graph.neighbors(node)
    }

    fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    fn node_value(&self, node: usize) -> Option<&NV> {
        self.graph.node_value(node)
    }

    fn node_value_mut(&mut self, node: usize) -> Option<&mut NV> {
        self.graph.node_value_mut(node)
    }

    fn iter<'a>(&'a self) -> impl Iterator<Item = (usize, &'a NV)>
    where
        NV: 'a,
    {
        self.graph.iter()
    }

    fn set_node_value(&mut self, node: usize, value: NV) -> Result<(), crate::GraphError> {
        self.graph.set_node_value(node, value)
    }

    fn to_stable_graph(
        &self,
    ) -> petgraph::prelude::StableGraph<Option<NV>, EV, petgraph::prelude::Directed, usize> {
        self.graph.to_stable_graph()
    }
}

impl<EV, NV, G> DirectedGraph<EV, NV> for QuadGraph<EV, NV, G>
where
    G: DirectedGraph<EV, NV>,
    NV: Coordinate + Debug,
{
    fn in_degree(&self, node: usize) -> usize {
        self.graph.in_degree(node)
    }

    fn out_degree(&self, node: usize) -> usize {
        self.graph.out_degree(node)
    }

    fn in_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a super::Target<EV>>
    where
        EV: 'a,
    {
        self.graph.in_neighbors(node)
    }

    fn out_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a super::Target<EV>>
    where
        EV: 'a,
    {
        self.graph.out_neighbors(node)
    }
}

impl<EV, NV, G> CoordGraph<EV, NV> for QuadGraph<EV, NV, G>
where
    G: Graph<EV, NV>,
    NV: Coordinate + Debug,
{
    fn nearest_node(&self, point: Coord) -> Option<usize> {
        self.nearest_node_bound(point, f64::MAX)
    }
    fn nearest_node_bound(&self, coord: Coord, tolerance: f64) -> Option<usize> {
        info!("Searching neighbour for: {:?}", coord);
        let point: Point = coord.into();
        let mut p1 = point.haversine_destination(315., 50.);
        let mut p2 = point.haversine_destination(135., 50.);
        let mut res = self
            .quad_tree
            .query_points(Boundary::between_points(p1.x_y(), p2.x_y()));

        while res.next().is_none() && p1.haversine_distance(&p2) <= tolerance {
            p1 = p1.haversine_destination(315., 50.);
            p2 = p2.haversine_destination(135., 50.);

            info!("Searching between {:?}, {:?}", p1, p2);

            res = self
                .quad_tree
                .query_points(Boundary::between_points(p1.x_y(), p2.x_y()));
        }

        info!("Found points");

        res.fold((f64::MAX, None), |closest, p| {
            let d = point.euclidean_distance(&Point::new(p.0.x, p.0.y));

            if d < closest.0 && d <= tolerance {
                (d, Some(p.1))
            } else {
                closest
            }
        })
        .1
    }
}

impl<'de, EV, NV> Deserialize<'de> for QuadGraph<EV, NV, DirectedCsrGraph<EV, NV>>
where
    EV: Copy + Deserialize<'de>,
    NV: Coordinate + Debug + Clone + Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field {
            Graph,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`graph` field")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "graph" => Ok(Field::Graph),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }

                    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
                    where
                        E: de::Error,
                    {
                        match value.as_str() {
                            "graph" => Ok(Field::Graph),
                            _ => Err(de::Error::unknown_field(value.as_str(), FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct QuadGraphVisitor<EV, NV> {
            _marker_0: PhantomData<EV>,
            _marker_1: PhantomData<NV>,
        }

        impl<EV, NV> QuadGraphVisitor<EV, NV> {
            fn new() -> Self {
                QuadGraphVisitor {
                    _marker_0: PhantomData,
                    _marker_1: PhantomData,
                }
            }
        }

        impl<'de, EV, NV> Visitor<'de> for QuadGraphVisitor<EV, NV>
        where
            EV: Copy + Deserialize<'de>,
            NV: Coordinate + Debug + Clone + Deserialize<'de>,
        {
            type Value = QuadGraph<EV, NV, DirectedCsrGraph<EV, NV>>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct QuadGraph")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let graph = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;

                Ok(QuadGraph::new_from_graph(graph))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut graph = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Graph => {
                            if graph.is_some() {
                                return Err(de::Error::duplicate_field("graph"));
                            }
                            graph = Some(map.next_value()?);
                        }
                    }
                }

                let graph = graph.ok_or_else(|| de::Error::missing_field("graph"))?;

                Ok(QuadGraph::new_from_graph(graph))
            }
        }

        const FIELDS: &[&str] = &["graph"];
        deserializer.deserialize_struct("QuadGraph", FIELDS, QuadGraphVisitor::new())
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::HashMap,
        fs::File,
        io::{BufReader, Read},
    };

    use approx::assert_relative_eq;
    use geo::{point, GeodesicDestination, HaversineDestination, Point};
    use geozero::geojson::read_geojson;
    use serde_test::{assert_tokens, Token};

    use crate::{
        graph::csr::DirectedCsrGraph,
        input::geo_zero::{ColumnValueClonable, GraphWriter},
        CoordGraph, Coordinate, Graph,
    };

    use super::QuadGraph;

    #[test]
    fn nearest_neighbour_search() {
        let mut geojson = r#" {
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

        let p_1 = Point::new(13.355102, 52.5364593).haversine_destination(30., 1000.);

        read_geojson(geojson.as_bytes(), &mut graph_writer);

        let graph = graph_writer.get_graph();

        let quad_graph = QuadGraph::new_from_graph(graph);

        let nearest_node = quad_graph.nearest_node(p_1.into());

        assert_relative_eq!(
            Point::new(13.355102, 52.5364593),
            Point::from(
                quad_graph
                    .graph
                    .node_value(nearest_node.unwrap())
                    .unwrap()
                    .x_y()
            ),
            epsilon = 1e-6
        );
    }

    #[test]
    fn nearest_neighbour_search_big() {
        let file = File::open("../resources/Berlin.geojson").unwrap();
        let reader = BufReader::new(file);
        let filter = |p: &HashMap<String, ColumnValueClonable>| {
            let footway = p.get("footway");
            let highway = p.get("highway");

            if highway.is_none() {
                return false;
            }

            match footway {
                Some(ColumnValueClonable::String(s)) => s == "null",
                _ => true,
            }
        };
        let mut graph_writer = GraphWriter::new(filter);

        let p_1 = Point::new(13.4865, 52.5668);

        read_geojson(reader, &mut graph_writer);

        let graph = graph_writer.get_graph();

        let quad_graph = QuadGraph::new_from_graph(graph);

        let nearest_node = quad_graph.nearest_node(p_1.into());

        assert_relative_eq!(
            Point::new(13.4864, 52.5659),
            Point::from(
                quad_graph
                    .graph
                    .node_value(nearest_node.unwrap())
                    .unwrap()
                    .x_y()
            ),
            epsilon = 1e-6
        );
    }

    #[test]
    fn ser_de() {
        let mut geojson = r#" {
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

        read_geojson(geojson.as_bytes(), &mut graph_writer);

        let graph = graph_writer.get_graph();

        let quad_graph = QuadGraph::new_from_graph(graph);

        println!("{:?}", serde_json::to_string(&quad_graph));
        assert_tokens(
            &quad_graph,
            &[
                Token::Struct {
                    name: "QuadGraph",
                    len: 1,
                },
                Token::Str("graph"),
                Token::Struct {
                    name: "DirectedCsrGraph",
                    len: 3,
                },
                Token::Str("node_values"),
                Token::Seq { len: Some(2) },
                Token::Struct {
                    name: "Coord",
                    len: 2,
                },
                Token::Str("x"),
                Token::F64(13.3530166),
                Token::Str("y"),
                Token::F64(52.5365623),
                Token::StructEnd,
                Token::Struct {
                    name: "Coord",
                    len: 2,
                },
                Token::Str("x"),
                Token::F64(13.3531553),
                Token::Str("y"),
                Token::F64(52.5364245),
                Token::StructEnd,
                Token::SeqEnd,
                Token::String("csr_out"),
                Token::Struct {
                    name: "Csr",
                    len: 2,
                },
                Token::Str("offsets"),
                Token::Seq { len: Some(3) },
                Token::U64(0),
                Token::U64(1),
                Token::U64(1),
                Token::SeqEnd,
                Token::Str("targets"),
                Token::Seq { len: Some(1) },
                Token::Struct {
                    name: "Target",
                    len: 2,
                },
                Token::Str("target"),
                Token::U64(1),
                Token::Str("value"),
                Token::F64(17.96628678846495),
                Token::StructEnd,
                Token::SeqEnd,
                Token::StructEnd,
                Token::Str("csr_inc"),
                Token::Struct {
                    name: "Csr",
                    len: 2,
                },
                Token::Str("offsets"),
                Token::Seq { len: Some(3) },
                Token::U64(0),
                Token::U64(0),
                Token::U64(1),
                Token::SeqEnd,
                Token::Str("targets"),
                Token::Seq { len: Some(1) },
                Token::Struct {
                    name: "Target",
                    len: 2,
                },
                Token::Str("target"),
                Token::U64(0),
                Token::Str("value"),
                Token::F64(17.96628678846495),
                Token::StructEnd,
                Token::SeqEnd,
                Token::StructEnd,
                Token::StructEnd,
                Token::StructEnd,
            ],
        )
    }
}
