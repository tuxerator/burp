use core::panic;
use std::f64;
use std::sync::{Arc, RwLock};

use crate::run_ui::Positions;
use crate::state::WgpuFrame;
use crate::types::PointerPos;
use ::geo_types::Geometry::{self, GeometryCollection, LineString, Point};
use ::geo_types::{coord, LineString as LineLineString, Point as PointPoint};
use burp::oracle::Oracle;
use burp::types::{CoordNode, Poi};
use galileo::control::{EventPropagation, MouseButton, MouseEvent, UserEvent};
use galileo::layer::feature_layer::{self, Feature, FeatureStore};
use galileo::layer::vector_tile_layer::style::VectorTileStyle;
use galileo::layer::FeatureLayer;
use galileo::symbol::{ArbitraryGeometrySymbol, SimpleContourSymbol};
use galileo::Color;
use galileo::{
    control::{EventProcessor, MapController},
    render::WgpuRenderer,
    tile_scheme::TileIndex,
    winit::WinitInputHandler,
    Map, MapBuilder, MapView, TileSchema,
};
use galileo_types::cartesian::{CartesianPoint2d, Point2d};
use galileo_types::geo::impls::GeoPoint2d;
use galileo_types::geo::{Crs, GeoPoint, NewGeoPoint};
use galileo_types::geometry_type::GeoSpace2d;
use galileo_types::impls::Contour;
use galileo_types::{cartesian::Size, latlon};
use galileo_types::{Disambig, Disambiguate};
use geo::Coord;
use geozero::geojson::GeoJson;
use geozero::{geo_types, ToGeo};
use graph_rs::graph::csr::DirectedCsrGraph;
use graph_rs::graph::quad_tree::QuadGraph;
use graph_rs::input::geo_zero::geozero::geojson::read_geojson;
use graph_rs::input::geo_zero::GraphWriter;
use graph_rs::{CoordGraph, Coordinate, DirectedGraph, Graph};
use log::info;
use ordered_float::OrderedFloat;
use wgpu::{Device, Queue, Surface, SurfaceConfiguration};
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub struct GalileoState {
    input_handler: WinitInputHandler,
    event_processor: EventProcessor,
    renderer: Arc<RwLock<WgpuRenderer>>,
    map: Arc<RwLock<galileo::Map>>,
    pointer_position: Arc<RwLock<PointerPos>>,
    click_position: Arc<RwLock<PointerPos>>,
    oracle: Arc<RwLock<Option<Oracle<Poi>>>>,
    hidden: bool,
}

impl GalileoState {
    pub fn new(
        window: Arc<Window>,
        device: Arc<Device>,
        surface: Arc<Surface<'static>>,
        queue: Arc<Queue>,
        config: SurfaceConfiguration,
        oracle: Arc<RwLock<Option<Oracle<Poi>>>>,
    ) -> Self {
        let messenger = galileo::winit::WinitMessenger::new(window);

        let renderer = WgpuRenderer::new_with_device_and_surface(device, surface, queue, config);
        let renderer = Arc::new(RwLock::new(renderer));

        let input_handler = WinitInputHandler::default();

        let view = MapView::new(
            &latlon!(37.566, 126.9784),
            TileSchema::web(18).lod_resolution(8).unwrap(),
        );

        let tile_source = |index: &TileIndex| {
            format!(
                "https://api.maptiler.com/maps/openstreetmap/256/{}/{}/{}.jpg?key=8vBMrBmo8MIbxzh6yNkC",
                index.z, index.x, index.y
            )
        };

        let vt_tile_source = |index: &TileIndex| {
            format!(
                "https://api.maptiler.com/tiles/v3-openmaptiles/{}/{}/{}.pbf?key=8vBMrBmo8MIbxzh6yNkC",
                index.z, index.x, index.y
            )
        };

        let map_layer = Box::new(MapBuilder::create_raster_tile_layer(
            tile_source,
            TileSchema::web(20),
        ));

        let map = Arc::new(RwLock::new(galileo::Map::new(
            view,
            vec![map_layer],
            Some(messenger),
        )));

        let pointer_position = Arc::new(RwLock::new(PointerPos::new_from_screen_pos(
            Point2d::default(),
            map.clone(),
        )));
        let pointer_position_clone = pointer_position.clone();

        let click_position = Arc::new(RwLock::new(PointerPos::new_from_screen_pos(
            Point2d::default(),
            map.clone(),
        )));
        let click_position_clone = click_position.clone();

        let mut event_processor = EventProcessor::default();

        let oracle_ref = oracle.clone();
        event_processor.add_handler(move |ev: &UserEvent, map: &mut Map| {
            match ev {
                UserEvent::PointerMoved(MouseEvent {
                    screen_pointer_position,
                    ..
                }) => pointer_position_clone
                    .write()
                    .expect("poisoned lock")
                    .set_screen_pos(*screen_pointer_position),
                UserEvent::Click(
                    MouseButton::Left,
                    MouseEvent {
                        screen_pointer_position,
                        ..
                    },
                ) => {
                    let mut click_pos = click_position_clone.write().expect("poisoned lock");

                    click_pos.set_screen_pos(*screen_pointer_position);

                    if let Some(geo_pos) = click_pos.geo_pos() {
                        let oracle_ref = oracle_ref.read().expect("poisoned lock");
                        if let Some(ref oracle) = *oracle_ref {
                            println!(
                                "node_value: {:?}",
                                oracle.get_node_value_at(
                                    PointPoint::new(geo_pos.lon(), geo_pos.lat()),
                                    20.0
                                )
                            )
                        }
                    }
                }
                _ => (),
            }

            EventPropagation::Propagate
        });
        event_processor.add_handler(MapController::default());

        GalileoState {
            input_handler,
            event_processor,
            renderer,
            map,
            pointer_position,
            click_position,
            oracle,
            hidden: false,
        }
    }

    pub fn build_map_layer(&self) {
        let mut oracle = self.oracle.write().expect("poisoned lock");

        if let Some(ref mut oracle) = *oracle {
            oracle.draw_to_map(self.map.clone());
        }
    }

    pub fn map(&self) -> Arc<RwLock<Map>> {
        self.map.clone()
    }

    /// Returns pointers to current pointer position and last click position.
    pub fn positions(&self) -> MapPositions {
        MapPositions {
            pointer_pos: self.pointer_position.clone(),
            click_pos: self.click_position.clone(),
        }
    }

    pub fn about_to_wait(&self) {
        self.map.write().unwrap().animate();
    }

    pub fn resize(&self, size: PhysicalSize<u32>) {
        self.renderer
            .write()
            .expect("poisoned lock")
            .resize(Size::new(size.width, size.height));
        self.map
            .write()
            .expect("poisoned lock")
            .set_size(Size::new((size.width) as f64, (size.height) as f64));
    }

    pub fn hide(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    pub fn render(&self, wgpu_frame: &WgpuFrame<'_>) {
        if !self.hidden {
            let galileo_map = self.map.read().unwrap();
            galileo_map.load_layers();

            self.renderer
                .write()
                .expect("poisoned lock")
                .render_to_texture_view(&galileo_map, wgpu_frame.texture_view);
        }
    }

    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) {
        // Phone emulator in browsers works funny with scaling, using this code fixes it.
        // But my real phone works fine without it, so it's commented out for now, and probably
        // should be deleted later, when we know that it's not needed on any devices.

        // #[cfg(target_arch = "wasm32")]
        // let scale = window.scale_factor();
        //
        // #[cfg(not(target_arch = "wasm32"))]
        let scale = 1.0;

        if let Some(raw_event) = self.input_handler.process_user_input(event, scale) {
            let mut map = self.map.write().expect("poisoned lock");
            self.event_processor.handle(raw_event, &mut map);
        }
    }
}

pub struct MapPositions {
    pub pointer_pos: Arc<RwLock<PointerPos>>,
    pub click_pos: Arc<RwLock<PointerPos>>,
}

fn get_layer_style() -> VectorTileStyle {
    const STYLE: &str = "resources/map_styles/bright.json";
    println!("Reached!");
    let file = std::fs::File::open(STYLE).unwrap();
    serde_json::from_reader(file).unwrap()
}
