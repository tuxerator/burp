use std::{collections::HashSet, fmt::Debug};

use geo::{BoundingRect, Coord, CoordFloat, MultiPoint, Point, Rect};
use log::info;
use ordered_float::{FloatCore, OrderedFloat};
use rstar::{RTree, RTreeNum, RTreeObject, iterators::LocateInEnvelope, primitives::GeomWithData};
use serde::{Deserialize, Serialize};

use crate::{
    CoordGraph, Coordinate, DirectedGraph, Graph, algorithms::dijkstra::Dijkstra, types::Direction,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct RTreeGraph<G, C>
where
    G: Graph,
    G::NV: Coordinate<C>,
    G::EV: Debug + Default,
    C: RTreeNum + CoordFloat,
{
    graph: G,

    r_tree: Box<RTree<GeomWithData<Coord<C>, usize>>>,
}

impl<G, C> RTreeGraph<G, C>
where
    G: Graph,
    G::NV: Coordinate<C>,
    G::EV: Debug + Default,
    C: RTreeNum + CoordFloat,
{
    pub fn new_from_graph(graph: G) -> Self {
        info!("Creating r-tree for graph...");

        let r_tree = Box::new(RTree::bulk_load(
            graph
                .nodes_iter()
                .map(|n| GeomWithData::new(n.1.as_coord(), n.0))
                .collect(),
        ));

        info!("Created r-tree: {} elements", r_tree.size());

        Self { graph, r_tree }
    }

    /// Returns the underlying graph data structure.
    pub fn graph(&self) -> &G {
        &self.graph
    }

    pub fn query(
        &self,
        envelope: &<GeomWithData<Coord<C>, usize> as RTreeObject>::Envelope,
    ) -> LocateInEnvelope<'_, GeomWithData<Coord<C>, usize>> {
        self.r_tree.locate_in_envelope(envelope)
    }
}

impl<G, C> RTreeGraph<G, C>
where
    G: DirectedGraph + Dijkstra,
    G::NV: Coordinate<C>,
    G::EV: FloatCore + Default + Debug + Clone,
    C: RTreeNum + CoordFloat,
{
    pub fn radius(
        &mut self,
        node: usize,
        envelope: &<GeomWithData<Coord<C>, usize> as RTreeObject>::Envelope,
        direction: Direction,
    ) -> Option<G::EV> {
        let nodes = self.query(envelope).map(|node| node.data);
        let nodes = HashSet::from_iter(nodes);

        if !nodes.contains(&node) {
            info!("Node not found");
            return None;
        }

        let distances = self.graph.dijkstra(node, nodes, direction);

        Some(
            *distances
                .0
                .into_iter()
                .max_by(|rhs, lhs| OrderedFloat(*rhs.cost()).cmp(&OrderedFloat(*lhs.cost())))?
                .cost(),
        )
    }
}

impl<G, C> Default for RTreeGraph<G, C>
where
    G: Graph,
    G::NV: Coordinate<C>,
    G::EV: Debug + Default,
    C: RTreeNum + CoordFloat,
{
    fn default() -> Self {
        let graph = G::default();
        RTreeGraph::new_from_graph(graph)
    }
}
impl<G, C> PartialEq for RTreeGraph<G, C>
where
    G: Graph + PartialEq,
    G::NV: Coordinate<C>,
    G::EV: Debug + Default,
    C: RTreeNum + CoordFloat,
{
    fn eq(&self, other: &Self) -> bool {
        self.graph.eq(&other.graph)
    }
}

impl<G, C> Graph for RTreeGraph<G, C>
where
    G: Graph,
    G::NV: Coordinate<C>,
    G::EV: Debug + Default,
    C: RTreeNum + CoordFloat,
{
    type EV = G::EV;
    type NV = G::NV;
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

    fn set_node_value(&mut self, node: usize, value: Self::NV) -> Result<(), crate::GraphError> {
        self.graph.set_node_value(node, value)
    }

    fn add_node(&mut self, weight: Self::NV) -> usize {
        let coord = weight.as_coord();
        let node_id = self.graph.add_node(weight);

        self.r_tree.insert(GeomWithData::new(coord, node_id));

        node_id
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

impl<G, C> DirectedGraph for RTreeGraph<G, C>
where
    G: DirectedGraph,
    G::NV: Coordinate<C>,
    G::EV: Debug + Default,
    C: RTreeNum + CoordFloat,
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

impl<G, C> CoordGraph for RTreeGraph<G, C>
where
    G: DirectedGraph,
    G::NV: Coordinate<C>,
    G::EV: Debug + Default,
    C: RTreeNum + CoordFloat,
{
    type C = C;
    fn nearest_node(&self, point: &Coord<C>) -> Option<usize> {
        self.r_tree.nearest_neighbor(point).map(|n| n.data)
    }
    fn nearest_node_bound(&self, coord: &Coord<C>, tolerance: C) -> Option<usize> {
        info!("Searching neighbour for: {:?}", coord);
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

impl<G, C> Dijkstra for RTreeGraph<G, C>
where
    G: DirectedGraph + Dijkstra,
    G::NV: Coordinate<C>,
    G::EV: FloatCore + Debug + Default + Clone,
    C: RTreeNum + CoordFloat,
{
    fn dijkstra(
        &self,
        start_node: usize,
        target_set: rustc_hash::FxHashSet<usize>,
        direction: Direction,
    ) -> crate::algorithms::dijkstra::DijkstraResult<Self::EV> {
        self.graph.dijkstra(start_node, target_set, direction)
    }

    fn dijkstra_full(
        &self,
        start_node: usize,
        direction: Direction,
    ) -> crate::algorithms::dijkstra::DijkstraResult<Self::EV> {
        self.graph.dijkstra_full(start_node, direction)
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, fs::File, io::BufReader};

    use approx::assert_relative_eq;
    use geo::{HaversineDestination, Point};
    use geozero::geojson::read_geojson;

    use crate::{
        CoordGraph, Coordinate, Graph,
        input::geo_zero::{ColumnValueClonable, GraphWriter},
    };

    use super::RTreeGraph;

    #[test]
    fn nearest_neighbour_search() {
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

        let p_1 = Point::new(13.355102, 52.5364593).haversine_destination(30., 1000.);

        read_geojson(geojson.as_bytes(), &mut graph_writer).unwrap();

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

        read_geojson(reader, &mut graph_writer).unwrap();

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
