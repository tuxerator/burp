use geozero::geojson::read_geojson;
use graph_rs::{Graph, graph::rstar::RTreeGraph};
use rand::{rng, seq::index::sample};
use rustc_hash::FxHashSet;
use std::{
    fs::File,
    iter::{self, successors},
};

use burp::oracle::oracle::Oracle;
use criterion::measurement::{Measurement, ValueFormatter};
use geo::{CoordFloat, CoordNum};
use rstar::RTreeNum;

use burp::{input::geo_zero::GraphWriter, oracle::PoiGraph, types::Poi};
use criterion::{
    BenchmarkId, Criterion, SamplingMode, Throughput, criterion_group, criterion_main,
};
use memmap2::MmapOptions;

pub fn build_oracle_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("oracle");

    let in_file = File::open("../resources/Konstanz_Paradies.geojson").unwrap();
    let in_file_mmap = unsafe { MmapOptions::new().map(&in_file).unwrap() };

    let mut graph_writer = GraphWriter::default();

    read_geojson(in_file_mmap.as_ref(), &mut graph_writer).unwrap();

    let mut graph = PoiGraph::new(RTreeGraph::new_from_graph(graph_writer.get_graph()));

    let size = successors(Some(1), |n| Some(n + 2));

    for size in size.take(10) {
        let pois = sample(&mut rng(), graph.graph().node_count(), size)
            .into_iter()
            .map(|node_id| (node_id, vec![]))
            .collect();
        graph.add_node_pois(pois);

        group.sample_size(10);
        group.sampling_mode(SamplingMode::Flat);
        group.throughput(Throughput::Elements(size as u64));
        group.bench_function(BenchmarkId::new("time", size), |b| {
            b.iter(|| {
                for poi in graph.poi_nodes.iter() {
                    Oracle::new().build_for_node(*poi, 0.25, graph.graph());
                }
            });
        });
    }
    group.finish();
}

criterion_group!(oracle, build_oracle_time);
criterion_main!(oracle);

pub struct OracleSize;

impl Measurement for OracleSize {
    type Intermediate = usize;
    type Value = usize;

    fn start(&self) -> Self::Intermediate {
        0
    }

    fn end(&self, i: Self::Intermediate) -> Self::Value {
        i
    }

    fn add(&self, v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        v1 + v2
    }

    fn zero(&self) -> Self::Value {
        0
    }

    fn to_f64(&self, value: &Self::Value) -> f64 {
        *value as f64
    }

    fn formatter(&self) -> &dyn criterion::measurement::ValueFormatter {
        &OracleSizeFormatter
    }
}

pub struct OracleSizeFormatter;

impl ValueFormatter for OracleSizeFormatter {
    fn scale_values(&self, typical_value: f64, values: &mut [f64]) -> &'static str {
        "block-pairs"
    }

    fn scale_throughputs(
        &self,
        typical_value: f64,
        throughput: &criterion::Throughput,
        values: &mut [f64],
    ) -> &'static str {
        "block-pairs/poi"
    }

    fn scale_for_machines(&self, values: &mut [f64]) -> &'static str {
        "block-pairs"
    }
}
