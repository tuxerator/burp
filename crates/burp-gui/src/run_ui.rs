extern crate geozero;

use std::collections::HashMap;
use std::error::Error;
use std::f64;
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, RwLock};

use burp::oracle::PoiGraph;
use burp::oracle::block_pair::BlockPair;
use burp::oracle::oracle::{Oracle, OracleCollection};
use burp::tree::Tree;
use burp::types::{CoordNode, Poi};
use egui::{Context, InnerResponse};
use galileo::Color;
use galileo::symbol::{CirclePointSymbol, SimpleContourSymbol};
use galileo_types::geo::{GeoPoint, NewGeoPoint};
use geo::{Coord, LineString};
use geozero::geojson::read_geojson;
use graph_rs::Graph;
use graph_rs::graph::csr::DirectedCsrGraph;
use graph_rs::graph::rstar::RTreeGraph;
use log::{info, warn};
use rfd::FileDialog;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::map::layers::block_pair_layer::BlockPairLayer;
// use crate::map::layers::oracle_layer::{BlocksLayer, BlocksSymbol};
use crate::map::Map;
use crate::map::layers::{
    line_layer::ContourLayer,
    node_layer::{NodeLayer, NodeMarker},
};
use crate::state::Events;
use crate::widgets::tree_view::TreeView;
use burp::input::geo_zero::{ColumnValueClonable, GraphWriter, PoiWriter};
use memmap2::MmapOptions;
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

type StartEndPos<T> = (
    Option<(usize, CoordNode<f64, T>)>,
    Option<(usize, CoordNode<f64, T>)>,
);

type OracleType<NV> = OracleCollection<RTreeGraph<DirectedCsrGraph<f64, CoordNode<f64, NV>>, f64>>;

pub struct UiState {
    graph: Option<PoiGraph<Poi>>,
    oracle: Option<Arc<Mutex<OracleType<Poi>>>>,
    debug_tree: Option<Tree<BlockPair<f64, f64>>>,
    map: Arc<RwLock<Map<String>>>,
    sender: Sender<Events>,
    state: State,
    epsilon: f64,
}

#[derive(PartialEq)]
enum State {
    Init,
    LoadedGraph,
    LoadedPois,
    Dijkstra(
        (
            Option<(usize, CoordNode<f64, Poi>)>,
            Option<(usize, CoordNode<f64, Poi>)>,
        ),
    ),
    DoubleDijkstra(
        (
            Option<(usize, CoordNode<f64, Poi>)>,
            Option<(usize, CoordNode<f64, Poi>)>,
        ),
    ),
    DoubleDijkstraResult(FxHashMap<usize, f64>),
    Oracle(Option<(usize, CoordNode<f64, Poi>)>),
}

impl Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Init => write!(f, "Init"),
            State::LoadedGraph => write!(f, "LoadedGraph"),
            State::LoadedPois => write!(f, "LoadedPois"),
            State::Dijkstra(points) => write!(f, "Dijkstra({:?})", points),
            State::DoubleDijkstra(points) => write!(f, "DoubleDijkstra({:?})", points),
            State::DoubleDijkstraResult(_) => write!(f, "DoubleDijkstraResult"),
            State::Oracle(point) => write!(f, "Oracle({:?})", point),
        }
    }
}

impl UiState {
    pub fn new(map: Arc<RwLock<Map<String>>>, sender: Sender<Events>) -> Self {
        Self {
            graph: None,
            oracle: None,
            debug_tree: None,
            map,
            sender,
            state: State::Init,
            epsilon: 0.2,
        }
    }
}

pub fn run_ui(state: &mut UiState, ctx: &Context) {
    match state.state {
        State::Dijkstra(_) => dijkstra(state),
        State::DoubleDijkstra(_) => double_dijkstra(state),
        State::DoubleDijkstraResult(_) => draw_resutl_path(state),
        State::Oracle(_) => build_oracle(state),
        _ => (),
    };

    egui::Window::new("Galileo map").show(ctx, |ui| {
        ui.label("Pointer position:");
        if let Some(pointer_position) = state
            .map
            .read()
            .expect("poisoned lock")
            .pointer_pos()
            .expect("poisoned lock")
        {
            ui.label(format!(
                "Lat: {:.4} Lon: {:.4}",
                pointer_position.lat(),
                pointer_position.lon()
            ));
        } else {
            ui.label("<unavaliable>");
        }

        ui.separator();

        ui.label("Map center position:");
        if let Some(map_center_position) = state
            .map
            .read()
            .expect("poisoned lock")
            .map_center_pos()
            .expect("poisoned lock")
        {
            ui.label(format!(
                "Lat: {:.4} Lon: {:.4}",
                map_center_position.lat(),
                map_center_position.lon()
            ));
        } else {
            ui.label("<unavaliable>");
        }

        ui.separator();

        ui.label("State:");
        ui.label(format!("{:?}", state.state));
    });

    egui::SidePanel::right("Left panel").show(ctx, |ui| {
        if ui.add(egui::Button::new("Load graph")).clicked() {
            let file_path = FileDialog::new().pick_file().unwrap();

            let file = File::open(file_path).unwrap();
            let buf_reader = BufReader::new(file);

            let filter = |p: &HashMap<String, ColumnValueClonable>| {
                let footway = p.get("footway");
                let highway = p.get("highway");

                match highway {
                    None => return false,
                    Some(ColumnValueClonable::String(s)) if s == "null" => return false,
                    Some(ColumnValueClonable::String(s)) if s == "cycleway" => return false,
                    Some(ColumnValueClonable::String(s)) if s == "path" => return false,
                    Some(ColumnValueClonable::String(s)) if s == "footway" => return false,
                    Some(ColumnValueClonable::String(s)) if s == "steps" => return false,
                    Some(ColumnValueClonable::String(s)) if s == "corridor" => return false,
                    _ => (),
                }

                match footway {
                    None => true,
                    Some(ColumnValueClonable::String(s)) => s == "null",
                    _ => false,
                }
            };
            let mut graph_writer = GraphWriter::new(filter);

            read_geojson(buf_reader, &mut graph_writer);
            let graph = RTreeGraph::new_from_graph(graph_writer.get_graph());
            state.graph = Some(PoiGraph::new(graph));

            state.state = State::LoadedGraph;
        }

        if ui
            .add_enabled(
                state.state == State::LoadedGraph,
                egui::Button::new("Load POIs"),
            )
            .clicked()
        {
            let sender_clone = state.sender.clone();
            let file_path = FileDialog::new().pick_file().unwrap();

            let file = File::open(file_path).unwrap();
            let buf_reader = BufReader::new(file);

            let mut poi_writer = PoiWriter::new(|_| true);
            read_geojson(buf_reader, &mut poi_writer);

            if let Some(ref mut oracel) = state.graph {
                oracel.add_coord_pois(poi_writer.pois());

                info!("Loaded Pois");
            }

            state.state = State::LoadedPois;
        }

        if ui
            .add_enabled(state.graph.is_some(), egui::Button::new("Toggle graph"))
            .clicked()
        {
            let mut map = state.map.write().expect("poisoned lock");

            let _ = map
                .toggle_layer(&String::from("graph"))
                .or_else(|_| -> Result<(), String> {
                    let layer: &mut Arc<RwLock<ContourLayer<SimpleContourSymbol, f64>>> = map
                        .or_insert(
                            "graph".to_string(),
                            ContourLayer::new(SimpleContourSymbol::new(Color::GREEN, 2.0)),
                        )
                        .as_any_mut()
                        .downcast_mut()
                        .ok_or("Couldn't downcast layer".to_string())?;

                    layer
                        .write()
                        .expect("poisoned lock")
                        .insert_coord_graph::<RTreeGraph<
                            DirectedCsrGraph<f64, CoordNode<f64, Poi>>, f64>,
                        >(state.graph.as_ref().unwrap().graph());

                    Ok(())
                });

            let _ = map
                .toggle_layer(&String::from("nodes"))
                .or_else(|_| -> Result<(), String> {
                    let layer: &mut Arc<RwLock<NodeLayer<CirclePointSymbol, ()>>> = map
                        .or_insert(
                            "nodes".to_string(),
                            NodeLayer::<CirclePointSymbol, ()>::new(CirclePointSymbol::new(
                                Color::RED,
                                3.0,
                            )),
                        )
                        .as_any_mut()
                        .downcast_mut()
                        .ok_or("Couldn't downcast layer".to_string())?;

                    layer.write().expect("poisoned lock").insert_nodes(
                        state
                            .graph
                            .as_ref()
                            .unwrap()
                            .graph()
                            .nodes_iter()
                            .map(|node| {
                                NodeMarker::new(*node.1.get_coord(), node.0, None)
                                    .expect("could not project point")
                            })
                            .collect(),
                    );

                    Ok(())
                });
        }

        if ui
            .add_enabled(
                state.state == State::LoadedPois,
                egui::Button::new("Dijkstra"),
            )
            .clicked()
        {
            state
                .map
                .write()
                .expect("poisoned lock")
                .take_click_pos()
                .expect("poisoned lock");
            state.state = State::Dijkstra((None, None));
        }

        if ui
            .add_enabled(
                state.state == State::LoadedPois,
                egui::Button::new("Double Dijkstra"),
            )
            .clicked()
        {
            state
                .map
                .write()
                .expect("poisoned lock")
                .take_click_pos()
                .expect("poisoned lock");
            state.state = State::DoubleDijkstra((None, None));
        }

        if ui
            .add_enabled(
                matches!(state.state, State::LoadedPois | State::LoadedGraph),
                egui::Button::new("Build Oracle"),
            )
            .clicked()
        {
            state
                .map
                .write()
                .expect("poisoned lock")
                .take_click_pos()
                .expect("poisoned lock");
            state.state = State::Oracle(None);
        }

        ui.add(egui::Slider::new(&mut state.epsilon, 0.0..=50.0));

        if ui.button("Save").clicked()
            && let Some(path) = FileDialog::new().save_file()
            && let Ok(file) = File::create(path)
            && let Some(ref oracle) = state.graph
        {
            let buf_writer = BufWriter::new(file);
            let mut rmp_serializer = Serializer::new(buf_writer);
            oracle.serialize(&mut rmp_serializer).unwrap();
        };

        if ui.button("Load").clicked()
            && let Some(path) = FileDialog::new().pick_file()
            && let Ok(in_file) = File::open(path)
        {
            let in_file_mmap = unsafe { MmapOptions::new().map(&in_file).unwrap() };

            let mut rmp_deserializer = Deserializer::new(in_file_mmap.as_ref());

            let graph: PoiGraph<Poi> = PoiGraph::deserialize(&mut rmp_deserializer).unwrap();
            info!(
                "Loaded graph: {} nodes, {} edges",
                graph.graph().node_count(),
                graph.graph().edge_count()
            );
            state.graph = Some(graph);
            state.state = State::LoadedPois;
        }
    });

    if let Some(ref debug_tree) = state.debug_tree {
        egui::Window::new("Oracle Split").show(ctx, |ui| {
            ui.add(TreeView::new(debug_tree, |ui, node, id| {
                if ui.button("Show on Map").clicked() {
                    info!("Drawing block pair");
                    let mut map = state.map.write().expect("poisoned lock");
                    let crs = map
                        .map_read_lock()
                        .expect("poisoned lock")
                        .view()
                        .crs()
                        .clone();

                    let layer: &mut Arc<RwLock<BlockPairLayer<f64>>> = map
                        .or_insert("block_pair".to_string(), BlockPairLayer::new(crs))
                        .as_any_mut()
                        .downcast_mut()
                        .expect("Couldn't downcast layer");

                    layer
                        .write()
                        .expect("poisoned lock")
                        .show_block_pair(node.get_data().clone());
                }
            }))
        });
    }
}

fn dijkstra(state: &mut UiState) {
    if let Some(ref graph) = state.graph {
        let State::Dijkstra((Some(ref start), Some(ref end))) = state.state else {
            let State::Dijkstra(ref mut pos) = state.state else {
                return;
            };
            *pos = get_start_end_pos(pos.clone(), state.map.clone(), graph);
            return;
        };
        info!(
            "Calculating shortes path from node {:?} to node {:?}",
            &start, &end
        );
        let mut target = FxHashSet::default();
        target.insert(end.0);
        let result = graph.dijkstra(start.0, target, graph_rs::types::Direction::Outgoing);

        let mut layer = state.map.write().expect("poisoned lock");

        let path = result.path(end.0).unwrap();

        let coords = path
            .into_iter()
            .fold(vec![], |mut coords: Vec<Coord>, node| {
                coords.push(
                    *state
                        .graph
                        .as_ref()
                        .unwrap()
                        .graph()
                        .node_value(node.node_id())
                        .unwrap()
                        .get_coord(),
                );
                coords
            });

        info!("Path found {:?}", &coords);

        let layer: &mut Arc<RwLock<ContourLayer<SimpleContourSymbol, f64>>> = layer
            .or_insert(
                "path".to_string(),
                ContourLayer::new(SimpleContourSymbol::new(Color::BLUE, 1.0)),
            )
            .as_any_mut()
            .downcast_mut()
            .unwrap();
        layer
            .write()
            .expect("poisoned lock")
            .insert_line(LineString::new(coords));
    } else {
        warn!("Couldn't get a lock on graph");
    }

    state.state = State::LoadedPois;
}

fn double_dijkstra(state: &mut UiState) {
    if let Some(ref oracle) = state.graph {
        let State::DoubleDijkstra((Some(ref start), Some(ref end))) = state.state else {
            let State::DoubleDijkstra(ref mut pos) = state.state else {
                return;
            };
            *pos = get_start_end_pos(pos.clone(), state.map.clone(), oracle);
            return;
        };
        let mut target = FxHashSet::default();
        target.insert(end.0);
        let result = oracle
            .beer_path_dijkstra_base(start.0, end.0, oracle.poi_nodes(), 0.0)
            .unwrap();

        info!("Finished beer paths");

        state.state = State::DoubleDijkstraResult(result);
    } else {
        warn!("Couldn't get a lock on oracle");
        state.state = State::LoadedPois;
    }
}

fn draw_resutl_path(state: &mut UiState) {
    let State::DoubleDijkstraResult(ref result) = state.state else {
        return;
    };
    let Some(ref oracle) = state.graph else {
        return;
    };

    let pois = result
        .iter()
        .fold(vec![], |mut coords: Vec<NodeMarker<Poi>>, node| {
            if let Some(node_marker) = NodeMarker::new(
                *oracle.graph().node_value(*node.0).unwrap().get_coord(),
                *node.0,
                Some(oracle.graph().node_value(*node.0).unwrap().data().to_vec()),
            ) {
                coords.push(node_marker);
            }
            coords
        });
    let mut map = state.map.write().expect("poisoned lock");
    {
        let layer: &mut Arc<RwLock<NodeLayer<CirclePointSymbol, Poi>>> = map
            .or_insert(
                "points".to_string(),
                NodeLayer::<CirclePointSymbol, Poi>::new(CirclePointSymbol::new(Color::RED, 5.0)),
            )
            .as_any_mut()
            .downcast_mut()
            .unwrap();

        let mut layer = layer.write().expect("poisoned lock");
        layer.insert_nodes(pois);
    }
    map.show_layer(&"points".to_string());

    state.state = State::LoadedPois;
}

fn get_start_end_pos(
    pos: StartEndPos<Poi>,
    map: Arc<RwLock<Map<String>>>,
    oracle: &PoiGraph<Poi>,
) -> StartEndPos<Poi> {
    match pos {
        (None, None) => (get_node_click_pos(map, oracle), None),
        (Some(start), None) => (Some(start), get_node_click_pos(map, oracle)),
        pos => pos,
    }
}

fn get_node_click_pos(
    map: Arc<RwLock<Map<String>>>,
    oracle: &PoiGraph<Poi>,
) -> Option<(usize, CoordNode<f64, Poi>)> {
    let click_pos = map
        .write()
        .expect("poisoned lock")
        .take_click_pos()
        .expect("poisoned lock");
    click_pos.and_then(|click_pos| {
        oracle
            .get_node_value_at(&geo::Coord::lonlat(click_pos.lon(), click_pos.lat()), 100.0)
            .ok()
    })
}

fn build_oracle(state: &mut UiState) {
    let Some(ref mut graph) = state.graph else {
        warn!("No graph loaded");
        return;
    };

    let State::Oracle(Some((node, _))) = state.state else {
        state.state = State::Oracle(get_node_click_pos(state.map.clone(), graph));
        return;
    };

    let mut oracle = OracleCollection::default();
    state.debug_tree = oracle
        .build_for_node(node, state.epsilon, graph.graph())
        .ok();
    state.oracle = Some(Arc::new(Mutex::new(oracle)));

    // let mut map = state.map.write().expect("poisoned lock");
    // {
    //     let layer: &mut Arc<RwLock<BlocksLayer<BlocksSymbol<f64>, f64>>> = map
    //         .or_insert(
    //             "points".to_string(),
    //             BlocksLayer::<BlocksSymbol<f64>, f64>::new(
    //                 state.oracle.clone().expect("Oracle not present"),
    //                 BlocksSymbol::new(),
    //             ),
    //         )
    //         .as_any_mut()
    //         .downcast_mut()
    //         .unwrap();
    //
    //     let layer = layer.write().expect("poisoned lock");
    // }
    state.state = State::LoadedPois;
}

fn unwrap_error(
    response: Option<InnerResponse<Option<Result<(), Box<dyn Error>>>>>,
) -> Result<(), Box<dyn Error>> {
    response.map_or(Ok(()), |inner_response| {
        inner_response.inner.map_or(Ok(()), |res| res)
    })
}
