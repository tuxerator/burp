use std::{collections::HashSet, fs::File, iter};

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

    let s_t_pairs = Vec::from_iter(
        iter::repeat_with(|| {
            let s_coord = Coord::from((
                rng.gen_range(bounding_box.min().x..bounding_box.max().y),
                rng.gen_range(bounding_box.min().y..bounding_box.max().y),
            ));
            let t_coord = Coord::from((
                rng.gen_range(bounding_box.min().x..bounding_box.max().y),
                rng.gen_range(bounding_box.min().y..bounding_box.max().y),
            ));
            (s_coord, t_coord)
        })
        .take(10),
    );

    println!("Testing {} pois", graph.poi_nodes().len());

    for s_t_pair in s_t_pairs {
        let dijkstra_result = graph.beer_path_dijkstra_fast(
            graph.get_node_value_at(&s_t_pair.0, f64::MAX).unwrap().0,
            graph.get_node_value_at(&s_t_pair.1, f64::MAX).unwrap().0,
            graph.poi_nodes(),
            0.2,
        );

        let oracle_result = oracle.get_pois(&s_t_pair.0, &s_t_pair.1);

        info!("Dijkstra result: {:?}", dijkstra_result);
        info!("Oracle result: {:?}", oracle_result);

        assert_eq!(
            HashSet::from_iter(dijkstra_result.into_keys()),
            oracle_result
        );
    }
}
