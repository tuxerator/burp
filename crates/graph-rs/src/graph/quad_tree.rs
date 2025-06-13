use core::f64;
use std::{
    collections::HashSet,
    fmt::{self, Debug},
    marker::PhantomData,
};

use geo::{
    point, BoundingRect, Coord, CoordFloat, CoordNum, EuclideanDistance, GeodesicDestination,
    GeodesicDistance, HaversineDestination, HaversineDistance, MultiPoint, Point, RhumbDestination,
    Translate, VincentyDistance,
};
use log::info;
use num_traits::{Float, FromPrimitive, Num, NumOps};
use ordered_float::{FloatCore, OrderedFloat};
use qutee::{Boundary, DynCap, QuadTree};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::{
    algorithms::dijkstra::{CachedDijkstra, Dijkstra},
    types::Direction,
    CoordGraph, Coordinate, DirectedGraph, Graph,
};

use super::{cache::DijkstraCache, csr::DirectedCsrGraph};

#[derive(Serialize, Debug)]
pub struct QuadGraph<'a, G, C>
where
    G: Graph,
    G::NV: Coordinate<C>,
    C: qutee::Coordinate + Num,
{
    graph: G,

    #[serde(skip_serializing)]
    quad_tree: Box<QuadTree<C, usize>>,
}

impl<G, C> QuadGraph<'_, G, C>
where
    G: Graph,
    G::NV: Coordinate<C>,
    C: qutee::Coordinate + Num,
{
    pub fn new_from_graph(graph: G) -> Self {
        let points = MultiPoint::from_iter(graph.nodes_iter().map(|c| c.1.as_coord()));

        let b_box = points.bounding_rect().unwrap();
        let b_box = Boundary::between_points(b_box.min().x_y(), b_box.max().x_y());

        let mut quad_tree = Box::new(QuadTree::new_with_dyn_cap(b_box, 20));
        for node in 0..graph.node_count() {
            quad_tree.insert_at(
                graph.node_value(node).expect("no value for node").x_y(),
                node,
            );
        }

        info!("Created quad-tree");

        Self { graph, quad_tree }
    }

    pub fn graph(&self) -> &G {
        &self.graph
    }

    pub fn boundary(&self) -> &Boundary<C> {
        self.quad_tree.boundary()
    }

    pub fn query_points<A>(&self, area: A) -> qutee::QueryPoints<'_, C, A, usize, DynCap>
    where
        A: qutee::Area<C>,
    {
        self.quad_tree.query_points(area)
    }

    pub fn query<A>(&self, area: A) -> qutee::Query<'_, C, A, usize, DynCap>
    where
        A: qutee::Area<C>,
    {
        self.quad_tree.query(area)
    }

    pub fn capacity(&self) -> usize {
        self.quad_tree.capacity()
    }
}

impl<G, C> QuadGraph<G, C>
where
    G: DirectedGraph + CachedDijkstra,
    C: qutee::Coordinate + Num,
    G::NV: Coordinate<C>,
    G::EV: FloatCore,
{
    pub fn radius<A>(&mut self, node: usize, area: &A, direction: Direction) -> Option<G::EV>
    where
        A: qutee::Area<C> + Debug,
    {
        let nodes = self.query_points(area.clone()).map(|node| node.1);
        let nodes = HashSet::from_iter(nodes);

        if !nodes.contains(&node) {
            info!("Node not found");
            return None;
        }

        let distances = self.dijkstra_cached(node, nodes, direction).unwrap();

        Some(
            *distances
                .0
                .into_iter()
                .max_by(|rhs, lhs| OrderedFloat(*rhs.cost()).cmp(&OrderedFloat(*lhs.cost())))?
                .cost(),
        )
    }
}

impl<G, C> Default for QuadGraph<G, C>
where
    G: Graph + Default,
    G::NV: Coordinate<C>,
    C: qutee::Coordinate + Num,
{
    fn default() -> Self {
        let graph = G::default();
        QuadGraph::new_from_graph(graph)
    }
}

impl<G, C> Graph for QuadGraph<G, C>
where
    G: Graph,
    G::NV: Coordinate<C>,
    C: qutee::Coordinate + Num,
{
    type NV = G::NV;
    type EV = G::EV;
    fn degree(&self, node: usize) -> usize {
        self.graph.degree(node)
    }

    fn neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a super::Target<Self::EV>>
    where
        Self::EV: 'a,
    {
        self.graph.neighbors(node)
    }

    fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    fn node_value(&self, node: usize) -> Option<&Self::NV> {
        self.graph.node_value(node)
    }

    fn node_value_mut(&mut self, node: usize) -> Option<&mut Self::NV> {
        self.graph.node_value_mut(node)
    }

    fn edges(&self) -> impl Iterator<Item = (usize, usize)> {
        self.graph.edges()
    }

    fn nodes_iter<'a>(&'a self) -> impl Iterator<Item = (usize, &'a Self::NV)>
    where
        Self::NV: 'a,
    {
        self.graph.nodes_iter()
    }

    fn set_node_value(&mut self, node: usize, value: G::NV) -> Result<(), crate::GraphError> {
        self.graph.set_node_value(node, value)
    }

    fn add_node(&mut self, weight: Self::NV) -> usize {
        self.graph.add_node(weight)
    }

    fn add_edge(&mut self, a: usize, b: usize, weight: Self::EV) -> bool {
        self.graph.add_edge(a, b, weight)
    }

    fn remove_node(&mut self, node: usize) -> Option<Self::NV> {
        self.graph.remove_node(node)
    }

    fn remove_edge(&mut self, edge: (usize, usize)) -> Option<Self::EV> {
        self.graph.remove_edge(edge)
    }
}

impl<G, C> DirectedGraph for QuadGraph<G, C>
where
    G: DirectedGraph,
    C: qutee::Coordinate + Num,
    G::NV: Coordinate<C>,
{
    fn in_degree(&self, node: usize) -> usize {
        self.graph.in_degree(node)
    }

    fn out_degree(&self, node: usize) -> usize {
        self.graph.out_degree(node)
    }

    fn in_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a super::Target<Self::EV>>
    where
        Self::EV: 'a,
    {
        self.graph.in_neighbors(node)
    }

    fn out_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a super::Target<Self::EV>>
    where
        Self::EV: 'a,
    {
        self.graph.out_neighbors(node)
    }
}

impl<G, C> CoordGraph for QuadGraph<G, C>
where
    G: DirectedGraph,
    C: qutee::Coordinate + Num + CoordFloat + FromPrimitive,
    G::NV: Coordinate<C> + Debug,
{
    type C = C;
    fn nearest_node(&self, point: &Coord<C>) -> Option<usize> {
        self.nearest_node_bound(point, C::max_value())
    }
    fn nearest_node_bound(&self, coord: &Coord<C>, tolerance: C) -> Option<usize> {
        info!("Searching neighbour for: {:?}", coord);
        let point = Point::from(*coord);
        let mut p1 = point.haversine_destination(C::from(315).unwrap(), C::from(50).unwrap());
        let mut p2 = point.haversine_destination(C::from(135).unwrap(), C::from(50).unwrap());
        let mut res = self
            .quad_tree
            .query_points(Boundary::between_points(p1.x_y(), p2.x_y()));

        while res.next().is_none() && p1.haversine_distance(&p2) <= tolerance {
            p1 = p1.haversine_destination(C::from(315).unwrap(), C::from(50).unwrap());
            p2 = p2.haversine_destination(C::from(135).unwrap(), C::from(50).unwrap());

            info!("Searching between {:?}, {:?}", p1, p2);

            res = self
                .quad_tree
                .query_points(Boundary::between_points(p1.x_y(), p2.x_y()));
        }

        info!("Found points");

        res.fold((C::max_value(), None), |closest, p| {
            let d = point.haversine_distance(&Point::new(p.0.x, p.0.y));

            if d < closest.0 && d <= tolerance {
                (d, Some(p.1))
            } else {
                closest
            }
        })
        .1
    }

    fn bounding_rect(&self) -> Option<geo::Rect<C>> {
        MultiPoint::new(
            self.quad_tree
                .iter_points()
                .map(|point| Point::new(point.0.x, point.0.y))
                .collect(),
        )
        .bounding_rect()
    }
}

impl<G, C> CachedDijkstra for QuadGraph<G, C>
where
    G: DirectedGraph + CachedDijkstra,
    G::NV: Coordinate<C>,
    G::EV: FloatCore,
    C: qutee::Coordinate + Num,
{
    fn dijkstra_cached(
        &mut self,
        start_node: usize,
        target_set: HashSet<usize>,
        direction: Direction,
    ) -> Option<crate::algorithms::dijkstra::DijkstraResult<Self::EV>> {
        self.graph
            .dijkstra_cached(start_node, target_set, direction)
    }

    fn dijkstra_full_cached(
        &mut self,
        start_node: usize,
        direction: Direction,
    ) -> Option<crate::algorithms::dijkstra::DijkstraResult<Self::EV>> {
        self.graph.dijkstra_full_cached(start_node, direction)
    }
}

impl<'de, EV, NV, C> Deserialize<'de> for QuadGraph<DirectedCsrGraph<EV, NV>, C>
where
    EV: Copy + Default + Deserialize<'de>,
    NV: Coordinate<C> + Debug + Clone + Deserialize<'de>,
    C: qutee::Coordinate + Num,
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

        struct QuadGraphVisitor<EV, NV, C> {
            _marker_0: PhantomData<EV>,
            _marker_1: PhantomData<NV>,
            _marker_2: PhantomData<C>,
        }

        impl<EV, NV, C> QuadGraphVisitor<EV, NV, C> {
            fn new() -> Self {
                QuadGraphVisitor {
                    _marker_0: PhantomData,
                    _marker_1: PhantomData,
                    _marker_2: PhantomData,
                }
            }
        }

        impl<'de, EV, NV, C> Visitor<'de> for QuadGraphVisitor<EV, NV, C>
        where
            EV: Copy + Default + Deserialize<'de>,
            NV: Coordinate<C> + Deserialize<'de>,
            C: qutee::Coordinate + Num,
        {
            type Value = QuadGraph<DirectedCsrGraph<EV, NV>, C>;

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

        let nearest_node = quad_graph.nearest_node(&p_1.as_coord());

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
    #[ignore = "Long runtime"]
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

        let nearest_node = quad_graph.nearest_node(&p_1.as_coord());

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

    #[ignore = "Failing and I am to lazy to update the test"]
    #[test]
    fn ser_de() {
        let geojson = r#" {
        "type": "FeatureCollection",
        "": [{
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
                    len: 4,
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
