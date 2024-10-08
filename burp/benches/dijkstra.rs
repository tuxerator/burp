use std::{
    env,
    fmt::Debug,
    fs::{self, File},
    io::{BufReader, Read},
    path::PathBuf,
};

use burp::{graph::PoiGraph, types::Poi};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use graph_rs::{types::Direction, Graph};
use rand::{
    seq::index::{self, sample},
    thread_rng,
};

pub fn dijkstra_full_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("dijkstra");

    let path = PathBuf::from("../resources/berlin.ocl");
    let mut file = File::open(path).unwrap();
    let mut f_buf = vec![];
    file.read_to_end(&mut f_buf);
    let oracle: PoiGraph<Poi> = PoiGraph::read_flexbuffer(f_buf.as_slice());

    let nodes = sample(&mut thread_rng(), oracle.graph().node_count(), 100);

    for node in nodes {
        group.sample_size(10);
        group.bench_with_input(BenchmarkId::new("dijkstra_full", &node), &node, |b, n| {
            b.iter(|| oracle.dijkstra_full(*n, Direction::Outgoing).unwrap())
        });
    }

    group.finish();
}

pub fn beer_path_dijkstra_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("beer_path");

    let path = PathBuf::from("../resources/berlin.ocl");
    let mut file = File::open(path).unwrap();
    let mut f_buf = vec![];
    file.read_to_end(&mut f_buf);
    let oracle: PoiGraph<Poi> = PoiGraph::read_flexbuffer(f_buf.as_slice());

    let nodes = sample(&mut thread_rng(), oracle.graph().node_count(), 100)
        .into_iter()
        .zip(sample(&mut thread_rng(), oracle.graph().node_count(), 100));

    for path in nodes {
        group.sample_size(10);
        group.bench_with_input(
            BenchmarkId::new("beer_path_dijkstra", format!("({}, {})", &path.0, &path.1)),
            &path,
            |b, n| b.iter(|| oracle.beer_path_dijkstra_base(n.0, n.1, oracle.poi_nodes().clone())),
        );
    }
}

criterion_group!(dijkstra, dijkstra_full_bench);
criterion_group!(beer_path_dijkstra, beer_path_dijkstra_bench);
criterion_main!(dijkstra, beer_path_dijkstra);
