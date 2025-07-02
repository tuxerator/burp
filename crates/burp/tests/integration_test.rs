use std::{collections::HashSet, fs::File, iter};

use burp::oracle::oracle::Oracle;
use geo::Coord;
use graph_rs::CoordGraph;
use log::info;
use rand::Rng;

mod common;

#[test]
fn oracle() {
    let (graph, oracle) = common::setup();

    let mut rng = rand::rng();

    let bounding_box = graph.graph().bounding_rect().unwrap();

    let s_t_pairs = Vec::from_iter(
        iter::repeat_with(|| {
            let s_coord = Coord::from((
                rng.random_range(bounding_box.min().x..bounding_box.max().y),
                rng.random_range(bounding_box.min().y..bounding_box.max().y),
            ));
            let t_coord = Coord::from((
                rng.random_range(bounding_box.min().x..bounding_box.max().y),
                rng.random_range(bounding_box.min().y..bounding_box.max().y),
            ));
            (s_coord, t_coord)
        })
        .take(10),
    );

    println!("Testing {} pois", graph.poi_nodes().len());

    for s_t_pair in s_t_pairs {
        let dijkstra_result = graph
            .beer_path_dijkstra_base(
                graph.graph().nearest_node(&s_t_pair.0).unwrap(),
                graph.graph().nearest_node(&s_t_pair.1).unwrap(),
                graph.poi_nodes(),
                0.25,
            )
            .unwrap();

        let oracle_result = oracle.get_pois(&s_t_pair.0, &s_t_pair.1);

        info!("Dijkstra result: {:?}", dijkstra_result);
        info!("Oracle result: {:?}", oracle_result);

        assert_eq!(
            HashSet::from_iter(dijkstra_result.into_keys()),
            oracle_result
        );
    }
}

#[test]
fn oracle_invariant() {
    let (graph, oracle) = common::setup();

    let pois = graph.poi_nodes();

    for poi in pois {
        assert!(
            oracle.invariant(graph.graph(), poi),
            "Found more than one block pair in oracle for node {:#?}",
            poi
        );
    }
}
