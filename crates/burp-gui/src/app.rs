use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use ashpd::{WindowIdentifier, desktop::file_chooser::FileFilter};
use burp::{
    input::geo_zero::{ColumnValueClonable, GraphWriter},
    oracle::{
        DefaultOracleParams, MinSplitParams, MinimalSplitStrategy, PoiGraph, SimpleSplitStrategy,
        block_pair::BlockPair, oracle::Oracle,
    },
    tree::Tree,
    types::{CoordNode, Poi},
};
use eframe::{App, CreationContext};
use egui::{Frame, FrameDurations, Modal, widget_text};
use galileo::{
    Color,
    render::text::{RustybuzzRasterizer, text_service::TextService},
    symbol::{CirclePointSymbol, SimpleContourSymbol},
};
use galileo_types::{
    cartesian::{NewCartesianPoint2d, Point2},
    geo::{Crs, GeoPoint},
};
use geo::Coord;
use geozero::geojson::read_geojson;
use graph_rs::{
    CoordGraph, Graph,
    graph::{csr::DirectedCsrGraph, rstar::RTreeGraph},
};
use log::info;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use tokio::task::LocalSet;
use tracing::{Level, instrument};
use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};

use crate::{
    event_handler::{Event, EventHandler},
    map::{
        Map,
        egui::{EguiMapState, MapResponse},
        layers::{
            block_pair_layer::BlockPairLayer,
            line_layer::ContourLayer,
            node_layer::{NodeLayer, NodeMarker, NodeSymbol},
        },
    },
    widgets::{self, TreeView},
};

mod app_data;

pub use app_data::AppData;

#[derive(PartialEq, Clone, Copy)]
enum SplitStrategy {
    SimpleSplitStrategy,
    MinimalSplitStrategy,
}

impl Display for SplitStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::SimpleSplitStrategy => "Simple Split Stragety",
                Self::MinimalSplitStrategy => "Minimal Split Stragety",
            }
        )
    }
}

pub struct BurpApp {
    app_id: String,
    pub map: EguiMapState<String>,
    pub runtime: tokio::runtime::Runtime,
    pub data: AppData,
    event_handler: EventHandler,
    sender: tokio::sync::mpsc::Sender<Event>,
    build_oracle: bool,
    split_strategy: SplitStrategy,
    merge_blocks: bool,
}

impl BurpApp {
    #[instrument(skip(cc))]
    pub fn new(app_id: impl Into<String> + Debug, cc: &CreationContext<'_>) -> Self {
        let app_id = app_id.into();
        let rasterizer = RustybuzzRasterizer::default();
        TextService::initialize(rasterizer).load_fonts("/home/jakob/.nix-profile/share/fonts");

        log::debug!("Initialising map state");
        let map_state = EguiMapState::new(
            cc.egui_ctx.clone(),
            egui::Id::new("galileo_map"),
            cc.wgpu_render_state
                .clone()
                .expect("failed to get wgpu context"),
            Map::default(),
        );

        let runtime = tokio::runtime::Runtime::new().unwrap();

        let (sender, recv) = tokio::sync::mpsc::channel(10);
        let event_handler = EventHandler::new(recv);

        let data = AppData::load_from_path(eframe::storage_dir(app_id.as_str()).unwrap());

        Self {
            app_id,
            map: map_state,
            runtime,
            data,
            event_handler,
            sender,
            build_oracle: false,
            split_strategy: SplitStrategy::SimpleSplitStrategy,
            merge_blocks: true,
        }
    }

    pub fn app_id(&self) -> &str {
        &self.app_id
    }
}

impl eframe::App for BurpApp {
    #[instrument(skip_all)]
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let storage_dir = eframe::storage_dir(self.app_id.as_str());

        let data = self.data.clone();

        if let Some(graph) = data.graph
            && graph.read().is_dirty()
        {
            self.runtime.spawn_blocking({
                let storage_dir = storage_dir.clone();
                move || {
                    tracing::info_span!("graph").in_scope(|| {
                        let mut file_path = storage_dir.expect("Not supportet on Android/iOS");
                        file_path.push("graph.gmp");

                        let mut file =
                            std::io::BufWriter::new(std::fs::File::create(file_path).unwrap());

                        match rmp_serde::encode::write(&mut file, graph.read().deref()) {
                            Ok(_) => tracing::info!("Saved graph"),
                            Err(err) => tracing::error!("Failed to save graph: {err}"),
                        }

                        graph.write().set_clean();
                    })
                }
            });
        }

        if let Some(oracle) = data.oracle
            && oracle.lock().is_dirty()
        {
            self.runtime.spawn_blocking({
                let storage_dir = storage_dir.clone();
                move || {
                    tracing::info_span!("oracle").in_scope(|| {
                        let mut file_path = storage_dir.expect("Not supportet on Android/iOS");
                        file_path.push("oracle.ocmp");

                        let mut file =
                            std::io::BufWriter::new(std::fs::File::create(file_path).unwrap());

                        let mut oracle = oracle.lock();

                        match rmp_serde::encode::write(&mut file, oracle.deref()) {
                            Ok(_) => tracing::info!("Saved oracle"),
                            Err(err) => tracing::error!("Failed to save oracle: {err}"),
                        }

                        oracle.set_clean();
                    })
                }
            });
        }

        if let Some(split_tree) = data.split_tree
            && split_tree.read().is_dirty()
        {
            self.runtime.spawn_blocking({
                let storage_dir = storage_dir.clone();
                move || {
                    tracing::info_span!("split_tree").in_scope(|| {
                        let mut file_path = storage_dir.expect("Not supportet on Android/iOS");
                        file_path.push("split_tree.scmp");

                        let mut file =
                            std::io::BufWriter::new(std::fs::File::create(file_path).unwrap());

                        match rmp_serde::encode::write(&mut file, split_tree.read().deref()) {
                            Ok(_) => tracing::info!("Saved split_tree"),
                            Err(err) => tracing::error!("Failed to save split_tree: {err}"),
                        }

                        split_tree.write().set_clean();
                    })
                }
            });
        }
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::new(5 * 60, 0)
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    #[instrument(skip_all)]
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.request_repaint();

        let mut dialog_modal = egui_modal::Modal::new(ctx, "dialog_modal");
        dialog_modal.show_dialog();

        let mut error_modal = widgets::modals::ErrorModal::new(ctx, "error_modal");
        error_modal.show();

        let _rt_ctx = self.runtime.enter();

        self.event_handler.handle_events(&mut self.data);

        egui::TopBottomPanel::top("Menu").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.add(widgets::OpenFile::new(
                        "Open",
                        vec![
                            FileFilter::new("geojson").glob("*.geojson"),
                            FileFilter::new("gmp").glob("*.gmp"),
                            FileFilter::new("omp").glob("*.omp"),
                            FileFilter::new("smp").glob("*.smp"),
                        ],
                        frame,
                        &self.runtime,
                        self.sender.clone(),
                        |path| {
                            let file_type = path
                                .extension()
                                .unwrap()
                                .to_str()
                                .expect("Could not convert OsStr to str");
                            let file = std::fs::File::open(path).unwrap();
                            let buf_reader = std::io::BufReader::new(file);

                            match file_type {
                                "geojson" => {
                                    let filter = |p: &HashMap<String, ColumnValueClonable>| {
                                        let footway = p.get("footway");
                                        let highway = p.get("highway");

                                        match highway {
                                            None => return false,
                                            Some(ColumnValueClonable::String(s)) if s == "null" => {
                                                return false;
                                            }
                                            Some(ColumnValueClonable::String(s))
                                                if s == "cycleway" =>
                                            {
                                                return false;
                                            }
                                            Some(ColumnValueClonable::String(s)) if s == "path" => {
                                                return false;
                                            }
                                            Some(ColumnValueClonable::String(s))
                                                if s == "footway" =>
                                            {
                                                return false;
                                            }
                                            Some(ColumnValueClonable::String(s))
                                                if s == "steps" =>
                                            {
                                                return false;
                                            }
                                            Some(ColumnValueClonable::String(s))
                                                if s == "corridor" =>
                                            {
                                                return false;
                                            }
                                            _ => (),
                                        }

                                        match footway {
                                            None => true,
                                            Some(ColumnValueClonable::String(s)) => s == "null",
                                            _ => false,
                                        }
                                    };
                                    let mut graph_writer = GraphWriter::new(filter);

                                    read_geojson(buf_reader, &mut graph_writer)
                                        .expect("Failed to parse geojson");

                                    let graph =
                                        RTreeGraph::new_from_graph(graph_writer.get_graph());

                                    Some(Event::GraphLoaded(PoiGraph::new(graph)))
                                }
                                "gmp" => {
                                    let graph: PoiGraph<Poi> =
                                        rmp_serde::from_read(buf_reader).unwrap();

                                    Some(Event::GraphLoaded(graph))
                                }
                                "omp" => {
                                    let oracle: Oracle<f64, f64> =
                                        rmp_serde::from_read(buf_reader).unwrap();

                                    Some(Event::OracleLoaded(oracle))
                                }
                                "smp" => {
                                    let split_tree = rmp_serde::from_read(buf_reader).unwrap();

                                    Some(Event::SplitTreeLoaded(split_tree))
                                }
                                _ => None,
                            }
                        },
                    ));
                });
            });
        });
        egui::SidePanel::right("Right Panel").show(ctx, |ui| {
            if ui
                .add_enabled(self.data.graph.is_some(), egui::Button::new("Show Graph"))
                .clicked()
            {
                if self.data.graph.as_ref().unwrap().read().is_dirty() {
                    self.map.map.remove("graph");
                    self.map.map.remove("nodes");
                }

                let _ = self.map.map.toggle_layer(&String::from("graph")).or_else(
                    |_| -> Result<(), String> {
                        let layer: &mut Arc<RwLock<ContourLayer<SimpleContourSymbol, f64>>> = self
                            .map
                            .map
                            .or_insert(
                                "graph".to_string(),
                                ContourLayer::new(
                                    SimpleContourSymbol::new(Color::GREEN, 2.0),
                                    Crs::WGS84,
                                ),
                            )
                            .as_any_mut()
                            .downcast_mut()
                            .ok_or("Couldn't downcast layer".to_string())?;

                        layer
                        .write()
                        .insert_coord_graph::<RTreeGraph<
                            DirectedCsrGraph<f64, CoordNode<f64, Poi>>, f64>,
                        >(self.data.graph.as_ref().unwrap().read().graph());

                        Ok(())
                    },
                );

                let _ = self.map.map.toggle_layer(&String::from("nodes")).or_else(
                    |_| -> Result<(), String> {
                        let layer: &mut Arc<RwLock<NodeLayer<NodeSymbol, ()>>> = self
                            .map
                            .map
                            .or_insert(
                                "nodes".to_string(),
                                NodeLayer::<NodeSymbol, ()>::new(
                                    NodeSymbol::new(CirclePointSymbol::new(Color::RED, 3.0)),
                                    Crs::WGS84,
                                ),
                            )
                            .as_any_mut()
                            .downcast_mut()
                            .ok_or("Couldn't downcast layer".to_string())?;

                        layer.write().insert_nodes(
                            self.data
                                .graph
                                .as_ref()
                                .unwrap()
                                .read()
                                .graph()
                                .nodes_iter()
                                .map(|node| {
                                    NodeMarker::new(*node.1.get_coord(), node.0, None)
                                        .expect("could not project point")
                                })
                                .collect(),
                        );

                        Ok(())
                    },
                );
                self.map.map.redraw();
            }

            if ui
                .add_enabled(self.data.graph.is_some(), egui::Button::new("Build Oracle"))
                .clicked()
            {
                self.build_oracle = true;
            }

            ui.toggle_value(&mut self.merge_blocks, "Merge blocks");

            egui::ComboBox::from_label("Split stragety")
                .selected_text(format!("{}", self.split_strategy))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.split_strategy,
                        SplitStrategy::SimpleSplitStrategy,
                        "Simple Split Stragety",
                    );
                    ui.selectable_value(
                        &mut self.split_strategy,
                        SplitStrategy::MinimalSplitStrategy,
                        "Minimal Split Stragety",
                    );
                });

            if self.build_oracle
                && self.map.clicked()
                && let Some(map_interact_pos) = self.map.map_interact_pos()
            {
                self.build_oracle = false;
                let node = error_modal.handle_error(ui, |ui| {
                    self.data
                        .graph
                        .as_ref()
                        .ok_or(ErrorMsg("No graph loaded"))?
                        .read()
                        .graph()
                        .nearest_node_bound(
                            &Coord::new(map_interact_pos.lon(), map_interact_pos.lat()),
                            20.,
                        )
                        .ok_or(Box::new(ErrorMsg(
                            "Could not find a node within the tolerance.",
                        )))
                });

                if let Some(node) = node
                    && let Some(graph) = self.data.graph.clone()
                {
                    log::info!("Building oracle...");
                    tokio::task::spawn_blocking({
                        let sender = self.sender.clone();
                        let split_strategy = self.split_strategy;
                        let merge_blocks = self.merge_blocks;
                        move || {
                            let graph = graph.read();
                            let oracle = match split_strategy {
                                SplitStrategy::SimpleSplitStrategy => Oracle::build_for_node(
                                    node,
                                    0.25,
                                    graph.graph(),
                                    DefaultOracleParams {
                                        merge_blocks: merge_blocks,
                                    },
                                )
                                .unwrap(),
                                SplitStrategy::MinimalSplitStrategy => Oracle::build_for_node(
                                    node,
                                    0.25,
                                    graph.graph(),
                                    MinSplitParams {
                                        merge_blocks: merge_blocks,
                                    },
                                )
                                .unwrap(),
                            };

                            sender
                                .try_send(Event::OracleBuild(oracle.0, oracle.1))
                                .unwrap();
                        }
                    });
                }
            }
        });
        egui::CentralPanel::default()
            .frame(Frame::new().inner_margin(0).outer_margin(0))
            .show(ctx, |ui| {
                self.map.render(ui);

                egui::Window::new("Map Position")
                    .constrain_to(ui.max_rect())
                    .show(ctx, |ui| {
                        ui.label("Map Pointer Position:");

                        ui.label(
                            if let Some(map_pointer_pos) = self.map.map_interact_pos().as_ref() {
                                format!(
                                    "Lat: {:.4} Lon: {:.4}",
                                    map_pointer_pos.lat(),
                                    map_pointer_pos.lon()
                                )
                            } else {
                                "Pointer not hovering over map".to_string()
                            },
                        )
                    });
                if self.build_oracle {
                    egui::Window::new("Build Oracle")
                        .collapsible(false)
                        .title_bar(false)
                        .anchor(egui::Align2::RIGHT_BOTTOM, [-5., -5.])
                        .constrain_to(ui.max_rect())
                        .auto_sized()
                        .show(ctx, |ui| {
                            ui.label("Click on a Node on the Map");
                        });
                }

                if self.data.split_tree.is_some() || self.data.oracle.is_some() {
                    egui::Window::new("Oracle").show(ctx, |ui| {
                        if let Some(oracles) = self.data.oracle.as_ref() {
                            let mut delete = Vec::new();
                            for oracle in oracles.lock().iter() {
                                ui.label(format!("Poi: {}", oracle.0));
                                ui.label(format!("Size: {}", oracle.1.size()));
                                if let Some(graph) = self.data.graph.as_ref() {
                                    ui.label(format!(
                                        "Average block ocupancy: {:.4}",
                                        oracle.1.avg_block_ocupancy(graph.read().deref().graph())
                                    ));
                                }
                                if ui.button("Delete").clicked() {
                                    delete.push(*oracle.0);
                                }
                                ui.separator();
                            }

                            for del in delete {
                                oracles.lock().remove(&del);
                            }
                        }

                        if let Some(split_trees) = self.data.split_tree.as_ref() {
                            ui.heading("Split Trees");
                            let mut delete = Vec::new();
                            for split_tree in split_trees.read().iter() {
                                ui.collapsing(split_tree.0.to_string(), |ui| {
                                    ui.add(widgets::TreeView::new(split_tree.1, |ui, node, id| {
                                        if node.data().1 {
                                            ui.visuals_mut().widgets.noninteractive.fg_stroke =
                                                egui::Stroke::new(1., egui::Color32::GREEN);
                                        }
                                        if node.data().0.values().not_in_path() {
                                            ui.visuals_mut().widgets.noninteractive.fg_stroke =
                                                egui::Stroke::new(1., egui::Color32::RED);
                                        }

                                        ui.label("BlockPair");

                                        ui.reset_style();

                                        if ui.button("Show on Map").clicked() {
                                            info!("Drawing block pair");
                                            error_modal.handle_error(ui, |ui| {
                                                let layer: &mut Arc<RwLock<BlockPairLayer<f64>>> =
                                                    self.map
                                                        .map
                                                        .or_insert(
                                                            "block_pair".to_string(),
                                                            BlockPairLayer::new(Crs::WGS84),
                                                        )
                                                        .as_any_mut()
                                                        .downcast_mut()
                                                        .ok_or(ErrorMsg(
                                                            "Couldn't downcast layer",
                                                        ))?;

                                                if let Some(ref graph) = self.data.graph {
                                                    let graph_lock = graph.read();

                                                    layer.write().show_block_pair(
                                                        node.data().0.clone(),
                                                        graph_lock.graph(),
                                                    );
                                                    Ok(())
                                                } else {
                                                    Err(Box::new(ErrorMsg("No graph loaded")))
                                                }
                                            });
                                            self.map.map.redraw();
                                        }

                                        let values =
                                            egui::CollapsingHeader::new("Values").id_salt((id, 0));
                                        values
                                            .show(ui, |ui| ui.label(format!("{}", node.data().0)));
                                    }));

                                    if ui.button("Delete").clicked() {
                                        delete.push(*split_tree.0);
                                    }
                                });
                            }

                            for del in delete {
                                split_trees.write().remove(&del);
                            }
                        }
                    });
                }
            });

        #[cfg(feature = "tracy")]
        tracy_client::frame_mark();
    }
}

#[derive(Debug)]
pub struct ErrorMsg<'a>(&'a str);

impl std::error::Error for ErrorMsg<'_> {}

impl std::fmt::Display for ErrorMsg<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
