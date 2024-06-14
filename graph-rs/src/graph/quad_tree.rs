use std::marker::PhantomData;

use geo::{
    point, Coord, GeodesicDestination, GeodesicDistance, HaversineDestination, HaversineDistance,
    Point, RhumbDestination, Translate, VincentyDistance,
};
use log::info;
use ordered_float::OrderedFloat;
use qutee::{Boundary, Coordinate, DynCap, QuadTree};

use crate::{CoordGraph, DirectedGraph, Graph};

use super::csr::DirectedCsrGraph;

pub struct QuadGraph<EV, G>
where
    G: Graph<EV, Coord>,
    EV: Send,
{
    graph: G,
    quad_tree: Box<QuadTree<f64, usize>>,
    _marker: PhantomData<EV>,
}

impl<EV, G> QuadGraph<EV, G>
where
    G: DirectedGraph<EV, Coord>,
    EV: Send,
{
    pub fn new_from_graph(graph: G) -> Self {
        let mut quad_tree = Box::new(QuadTree::new_with_dyn_cap(
            Boundary::new((-180., -90.), 360., 180.),
            20,
        ));

        for node in 0..graph.node_count() {
            quad_tree.insert_at(graph.node_value(node).unwrap().x_y(), node);
        }

        info!("Created quad-tree");

        Self {
            graph,
            quad_tree,
            _marker: PhantomData,
        }
    }

    pub fn graph(&self) -> &G {
        &self.graph
    }
}

impl<EV, G> Graph<EV, Coord> for QuadGraph<EV, G>
where
    G: Graph<EV, Coord>,
    EV: Sync + Send,
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

    fn node_value(&self, node: usize) -> Option<&Coord> {
        self.graph.node_value(node)
    }

    fn to_stable_graph(
        &self,
    ) -> petgraph::prelude::StableGraph<Option<Coord>, EV, petgraph::prelude::Directed, usize> {
        self.graph.to_stable_graph()
    }
}

impl<EV, G> DirectedGraph<EV, Coord> for QuadGraph<EV, G>
where
    G: DirectedGraph<EV, Coord>,
    EV: Send + Sync,
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

impl<EV, G> CoordGraph<EV> for QuadGraph<EV, G>
where
    G: Graph<EV, Coord>,
    EV: Sync + Send,
{
    fn nearest_node(&self, point: Point) -> usize {
        info!("Searching neighbour for: {:?}", point);
        let mut p1 = point.geodesic_destination(315., 100.);
        let mut p2 = point.geodesic_destination(135., 100.);
        let mut res = self
            .quad_tree
            .query_points(Boundary::between_points(p1.x_y(), p2.x_y()));

        while res.next() == None {
            p1 = p1.haversine_destination(315., 100.);
            p2 = p2.haversine_destination(135., 100.);
            let p_1 = qutee::Point::new(13.3530100, 52.536565);
            let p_2 = qutee::Point::new(13.35302, 52.53655);
            let boundary = Boundary::between_points(p_1, p_2);

            res = self
                .quad_tree
                .query_points(Boundary::between_points(p1.x_y(), p2.x_y()));
        }

        let near_node = res
            .fold((f64::MAX, 0), |closest, p| {
                let d = point.haversine_distance(&Point::new(p.0.x, p.0.y));

                if d < closest.0 {
                    (d, p.1)
                } else {
                    closest
                }
            })
            .1;

        info!(
            "Found point: {:?}",
            self.graph.node_value(near_node).unwrap()
        );
        near_node
    }
}

#[cfg(test)]
mod test {
    use std::{
        fs::File,
        io::{BufReader, Read},
    };

    use approx::assert_relative_eq;
    use geo::{point, GeodesicDestination, HaversineDestination, Point};
    use geozero::geojson::read_geojson;

    use crate::{input::geo_zero::GraphWriter, CoordGraph, Graph};

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

        let mut graph_writer = GraphWriter::default();
        graph_writer.filter_features();

        let p_1 = Point::new(13.355102, 52.5364593).haversine_destination(30., 1000.);

        read_geojson(geojson.as_bytes(), &mut graph_writer);

        let graph = graph_writer.get_graph();

        let quad_graph = QuadGraph::new_from_graph(graph);

        let nearest_node = quad_graph.nearest_node(p_1);

        assert_relative_eq!(
            Point::new(13.355102, 52.5364593),
            Point::from(quad_graph.graph.node_value(nearest_node).unwrap().x_y()),
            epsilon = 1e-6
        );
    }

    #[test]
    fn nearest_neighbour_search_big() {
        let file = File::open("../resources/Berlin.geojson").unwrap();
        let reader = BufReader::new(file);
        let mut graph_writer = GraphWriter::default();
        graph_writer.filter_features();

        let p_1 = Point::new(13.4865, 52.5668);

        read_geojson(reader, &mut graph_writer);

        let graph = graph_writer.get_graph();

        let quad_graph = QuadGraph::new_from_graph(graph);

        let nearest_node = quad_graph.nearest_node(p_1);

        assert_relative_eq!(
            Point::new(13.4864, 52.5659),
            Point::from(quad_graph.graph.node_value(nearest_node).unwrap().x_y()),
            epsilon = 1e-6
        );
    }
}
