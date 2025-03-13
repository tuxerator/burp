use std::{collections::HashSet, fs::File};

use burp::graph::oracle::Oracle;
use geo::Coord;
use graph_rs::CoordGraph;
use log::info;
use rand::Rng;

mod common;

#[test]
fn oracle() {
    let (graph, oracle) = common::setup();

    let mut rng = rand::thread_rng();

    let bounding_box = graph.graph().bounding_rect().unwrap();

    let s_coord = Coord::from((
        rng.gen_range(bounding_box.min().x..bounding_box.max().y),
        rng.gen_range(bounding_box.min().y..bounding_box.max().y),
    ));
    let t_coord = Coord::from((
        rng.gen_range(bounding_box.min().x..bounding_box.max().y),
        rng.gen_range(bounding_box.min().y..bounding_box.max().y),
    ));

    println!("Testing {} pois", graph.poi_nodes().len());

    let dijkstra_result = graph.beer_path_dijkstra_fast(
        graph.get_node_value_at(&s_coord, f64::MAX).unwrap().0,
        graph.get_node_value_at(&t_coord, f64::MAX).unwrap().0,
        graph.poi_nodes(),
        0.2,
    );

    let oracle_result = oracle.get_pois(&s_coord, &t_coord);

    assert_eq!(
        HashSet::from_iter(dijkstra_result.into_keys()),
        oracle_result
    );
}
