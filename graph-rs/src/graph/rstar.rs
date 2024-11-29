use core::f64;
use std::{
    collections::HashSet,
    fmt::{self, Debug},
    marker::PhantomData,
};

use geo::{
    point, BoundingRect, Coord, CoordFloat, CoordNum, EuclideanDistance, GeodesicDestination,
    GeodesicDistance, HaversineDestination, HaversineDistance, MultiPoint, Point, Rect,
    RhumbDestination, Translate, VincentyDistance,
};
use log::info;
use num_traits::{Float, FromPrimitive, Num, NumOps};
use ordered_float::{FloatCore, OrderedFloat};
use qutee::{Boundary, DynCap, QuadTree};
use rstar::{
    iterators::LocateInEnvelope, primitives::GeomWithData, Envelope, RTree, RTreeNum, RTreeObject,
};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::{
    algorithms::dijkstra::{CachedDijkstra, Dijkstra},
    types::Direction,
    CoordGraph, Coordinate, DirectedGraph, Graph,
};

use super::csr::DirectedCsrGraph;

#[derive(Serialize, Deserialize, Debug)]
pub struct RTreeGraph<EV, NV, G, C>
where
    G: Graph<EV, NV>,
    NV: Coordinate<C>,
    C: RTreeNum + CoordFloat,
{
    graph: G,

    r_tree: Box<RTree<GeomWithData<Coord<C>, usize>>>,

    #[serde(skip)]
    _marker_0: PhantomData<EV>,

    #[serde(skip)]
    _marker_1: PhantomData<NV>,
}

impl<EV, NV, G, C> RTreeGraph<EV, NV, G, C>
where
    G: Graph<EV, NV>,
    NV: Coordinate<C>,
    C: RTreeNum + CoordFloat,
{
    pub fn new_from_graph(graph: G) -> Self {
        let points = MultiPoint::from_iter(graph.node_values().map(|c| c.1.as_coord()));

        let b_box = points.bounding_rect().unwrap();
        let b_box = Boundary::between_points(b_box.min().x_y(), b_box.max().x_y());

        let r_tree = Box::new(RTree::bulk_load(
            graph
                .node_values()
                .map(|n| GeomWithData::new(n.1.as_coord(), n.0))
                .collect(),
        ));

        info!("Created r-tree");

        Self {
            graph,
            r_tree,
            _marker_0: PhantomData,
            _marker_1: PhantomData,
        }
    }

    pub fn graph(&self) -> &G {
        &self.graph
    }

    pub fn query(
        &self,
        envelope: &<GeomWithData<Coord<C>, usize> as RTreeObject>::Envelope,
    ) -> LocateInEnvelope<GeomWithData<Coord<C>, usize>> {
        self.r_tree.locate_in_envelope(envelope)
    }
}

impl<EV, NV, G, C> RTreeGraph<EV, NV, G, C>
where
    G: DirectedGraph<EV, NV> + CachedDijkstra<EV, NV>,
    NV: Coordinate<C>,
    EV: FloatCore,
    C: RTreeNum + CoordFloat,
{
    pub fn radius(
        &mut self,
        node: usize,
        envelope: &<GeomWithData<Coord<C>, usize> as RTreeObject>::Envelope,
        direction: Direction,
    ) -> Option<EV> {
        let nodes = self.query(envelope).map(|node| node.data);
        let nodes = HashSet::from_iter(nodes);

        if !nodes.contains(&node) {
            info!("Node not found");
            return None;
        }

        let distances = self.cached_dijkstra(node, nodes, direction).unwrap();

        Some(
            *distances
                .0
                .into_iter()
                .max_by(|rhs, lhs| OrderedFloat(*rhs.cost()).cmp(&OrderedFloat(*lhs.cost())))?
                .cost(),
        )
    }
}

impl<EV, NV, G, C> PartialEq for RTreeGraph<EV, NV, G, C>
where
    G: Graph<EV, NV> + PartialEq,
    NV: Coordinate<C>,
    C: RTreeNum + CoordFloat,
{
    fn eq(&self, other: &Self) -> bool {
        self.graph.eq(&other.graph)
    }
}

impl<EV, NV, G, C> Graph<EV, NV> for RTreeGraph<EV, NV, G, C>
where
    G: Graph<EV, NV>,
    NV: Coordinate<C>,
    C: RTreeNum + CoordFloat,
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

    fn edges(&self) -> impl Iterator<Item = (usize, usize)> {
        self.graph.edges()
    }

    fn node_values<'a>(&'a self) -> impl Iterator<Item = (usize, &'a NV)>
    where
        NV: 'a,
    {
        self.graph.node_values()
    }

    fn set_node_value(&mut self, node: usize, value: NV) -> Result<(), crate::GraphError> {
        self.graph.set_node_value(node, value)
    }

    fn add_node(&mut self, weight: NV) -> usize {
        let coord = weight.as_coord();
        let node_id = self.graph.add_node(weight);

        self.r_tree.insert(GeomWithData::new(coord, node_id));

        node_id
    }

    fn add_edge(&mut self, a: usize, b: usize, weight: EV) -> bool {
        self.graph.add_edge(a, b, weight)
    }
}

impl<EV, NV, G, C> DirectedGraph<EV, NV> for RTreeGraph<EV, NV, G, C>
where
    G: DirectedGraph<EV, NV>,
    NV: Coordinate<C>,
    C: RTreeNum + CoordFloat,
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

impl<EV, NV, G, C> CoordGraph<EV, NV, C> for RTreeGraph<EV, NV, G, C>
where
    G: DirectedGraph<EV, NV>,
    NV: Coordinate<C>,
    C: RTreeNum + CoordFloat,
{
    fn nearest_node(&self, point: &Coord<C>) -> Option<usize> {
        self.r_tree.nearest_neighbor(point).map(|n| n.data)
    }
    fn nearest_node_bound(&self, coord: &Coord<C>, tolerance: C) -> Option<usize> {
        info!("Searching neighbour for: {:?}", coord);
        let point = Point::from(*coord);

        let mut neighbors = self.r_tree.nearest_neighbor_iter_with_distance_2(coord);

        let neighbor_bound = neighbors.find(|node| node.1 <= tolerance);

        info!("Found points");

        neighbor_bound.map(|n| n.0.data)
    }

    fn bounding_rect(&self) -> Option<Rect<C>> {
        let points = MultiPoint::new(
            self.r_tree
                .iter()
                .map(|geom| Point::from(*geom.geom()))
                .collect(),
        );

        points.bounding_rect()
    }
}

impl<EV, NV, G, C> CachedDijkstra<EV, NV> for RTreeGraph<EV, NV, G, C>
where
    G: DirectedGraph<EV, NV> + CachedDijkstra<EV, NV>,
    EV: FloatCore,
    NV: Coordinate<C>,
    C: RTreeNum + CoordFloat,
{
    fn cached_dijkstra(
        &mut self,
        start_node: usize,
        target_set: HashSet<usize>,
        direction: Direction,
    ) -> Option<crate::algorithms::dijkstra::DijkstraResult<EV>> {
        self.graph
            .cached_dijkstra(start_node, target_set, direction)
    }

    fn cached_dijkstra_full(
        &mut self,
        start_node: usize,
        direction: Direction,
    ) -> Option<crate::algorithms::dijkstra::DijkstraResult<EV>> {
        self.graph.cached_dijkstra_full(start_node, direction)
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

    use super::RTreeGraph;

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

        let quad_graph = RTreeGraph::new_from_graph(graph);

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

        let quad_graph = RTreeGraph::new_from_graph(graph);

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
}
