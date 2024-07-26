use std::{
    collections::HashMap,
    fs::File,
    future::Future,
    io::BufReader,
    sync::{Arc, RwLock},
};

use burp::{
    input::geo_zero::{ColumnValueClonable, GraphWriter, PoiWriter},
    oracle::Oracle,
    types::Poi,
};
use egui::{Context, InnerResponse, SidePanel, Ui as EguiUi};
use galileo::Map;
use galileo_types::geo::{impls::GeoPoint2d, GeoPoint};
use geozero::geojson::read_geojson;
use graph_rs::graph::quad_tree::QuadGraph;
use rfd::FileDialog;
use tokio::task::JoinHandle;

use crate::types::PointerPos;

use super::{galileo_state::GalileoState, Events};

macro_rules! impl_state {
    ($($struct:ty), + ) => {
        $(
            impl State for $struct {}
        )*
    };
}

pub trait Ui: Sized {
    fn side_panel(state_data: StateData<Self>, ui: &mut EguiUi) -> State;
}

#[derive(Clone)]
pub struct Init {}

impl Ui for Init {
    fn side_panel(state_data: StateData<Init>, ui: &mut EguiUi) -> State {
        if ui.add(egui::Button::new("Load graph")).clicked() {
            return State::LoadOracle(state_data.change_state(LoadingOracle {
                oracle_join_hanle: Init::load_oracle(),
            }));
        }
        State::Init(state_data)
    }
}

impl Init {
    fn load_oracle() -> JoinHandle<Oracle<Poi>> {
        let file_path = FileDialog::new().set_directory("~/").pick_file().unwrap();

        tokio::spawn(async move {
            let file = File::open(file_path).unwrap();
            let buf_reader = BufReader::new(file);

            let filter = |p: &HashMap<String, ColumnValueClonable>| {
                let footway = p.get("footway");
                let highway = p.get("highway");

                match highway {
                    None => return false,
                    Some(ColumnValueClonable::String(s)) if s == "null" => return false,
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
            let graph = QuadGraph::new_from_graph(graph_writer.get_graph());
            Oracle::from(graph)
        })
    }
}

struct LoadingOracle {
    oracle_join_hanle: JoinHandle<Oracle<Poi>>,
}

impl Ui for LoadingOracle {
    fn side_panel(state_data: StateData<Self>, ui: &mut EguiUi) -> State {
        State::LoadOracle(state_data)
    }
}

#[derive(Clone)]
pub struct StateData<T: Ui> {
    pub map: Arc<RwLock<Map>>,
    pub state: T,
    pub pointer_pos: Arc<RwLock<PointerPos>>,
}

impl<T: Ui> StateData<T> {
    fn change_state<V: Ui>(self, new_state: V) -> StateData<V> {
        StateData {
            map: self.map,
            state: new_state,
            pointer_pos: self.pointer_pos,
        }
    }
}

impl StateData<Init> {
    pub fn new(map: Arc<RwLock<Map>>, pointer_pos: Arc<RwLock<PointerPos>>) -> Self {
        StateData {
            map,
            pointer_pos,
            state: Init {},
        }
    }
}

pub struct UiState {
    state: State,
    map: Arc<RwLock<Map>>,
    pointer_pos: Arc<RwLock<PointerPos>>,
}

impl UiState {
    pub fn new(map: Arc<RwLock<Map>>, pointer_pos: Arc<RwLock<PointerPos>>) -> Self {
        Self {
            state: State::Init(StateData::new(map, pointer_pos)),
        }
    }
    pub fn run_ui(&mut self, ctx: &Context) {
        self.state = self.state.run_ui(ctx);
    }
}

enum State {
    Init(Init),
    LoadOracle(LoadingOracle),
    Default(Init),
    Dijkstra(Init),
}

impl State {
    fn ui<T: Ui>(state_data: StateData<T>, ctx: &Context) -> State {
        State::map_window(&state_data, ctx);
        State::side_panel(state_data, ctx)
    }

    fn side_panel<T: Ui>(state_data: StateData<T>, ctx: &Context) -> State {
        SidePanel::right("right_panel")
            .show(ctx, |ui| T::side_panel(state_data, ui))
            .inner
    }

    fn map_window<T: Ui>(state_data: &StateData<T>, ctx: &Context) {
        egui::Window::new("Galileo map").show(ctx, |ui| {
            ui.label("Pointer position:");
            if let Some(pointer_position) = state_data
                .pointer_pos
                .read()
                .expect("poisoned lock")
                .geo_pos()
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
            if let Some(map_center_position) = state_data
                .map
                .read()
                .expect("poisoned lock")
                .view()
                .position()
            {
                ui.label(format!(
                    "Lat: {:.4} Lon: {:.4}",
                    map_center_position.lat(),
                    map_center_position.lon()
                ));
            } else {
                ui.label("<unavaliable>");
            }
        });
    }
}

// impl Ui<Box<InitState>> {
//     fn load_oracle(&self) -> JoinHandle<Oracle<Poi>> {
//         let file_path = FileDialog::new().set_directory("~/").pick_file().unwrap();
//
//         tokio::spawn(async move {
//             let file = File::open(file_path).unwrap();
//             let buf_reader = BufReader::new(file);
//
//             let filter = |p: &HashMap<String, ColumnValueClonable>| {
//                 let footway = p.get("footway");
//                 let highway = p.get("highway");
//
//                 match highway {
//                     None => return false,
//                     Some(ColumnValueClonable::String(s)) if s == "null" => return false,
//                     _ => (),
//                 }
//
//                 match footway {
//                     None => true,
//                     Some(ColumnValueClonable::String(s)) => s == "null",
//                     _ => false,
//                 }
//             };
//             let mut graph_writer = GraphWriter::new(filter);
//
//             read_geojson(buf_reader, &mut graph_writer);
//             let graph = QuadGraph::new_from_graph(graph_writer.get_graph());
//             Oracle::from(graph)
//         })
//     }
// }
