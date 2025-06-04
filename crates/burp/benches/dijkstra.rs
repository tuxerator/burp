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
use memmap2::MmapOptions;
use rand::{
    rng,
    seq::index::{self, sample},
    thread_rng,
};

pub fn dijkstra_full_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("dijkstra");

    let graph_file = File::open("../resources/medium_poi.gfb").unwrap();
    let graph_mmap = unsafe { MmapOptions::new().map(&graph_file).unwrap() };
    let graph: PoiGraph<Poi> = PoiGraph::read_flexbuffer(&graph_mmap);

    let nodes = sample(&mut rng(), graph.graph().node_count(), 100);

    for node in nodes {
        group.sample_size(10);
        group.bench_with_input(BenchmarkId::new("dijkstra_full", node), &node, |b, n| {
            b.iter(|| graph.dijkstra_full(*n, Direction::Outgoing).unwrap())
        });
    }

    group.finish();
}

criterion_group!(dijkstra, dijkstra_full_bench);
criterion_main!(dijkstra);
