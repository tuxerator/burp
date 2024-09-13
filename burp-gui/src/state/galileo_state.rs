use core::panic;
use std::f64;
use std::sync::{Arc, RwLock};

use crate::map::Map;
use crate::state::WgpuFrame;
use crate::types::MapPositions;
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
    Map as GalileoMap, MapBuilder, MapView, TileSchema,
};
use galileo_types::cartesian::{CartesianPoint2d, NewCartesianPoint2d, Point2d};
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
    renderer: Arc<RwLock<WgpuRenderer>>,
    map: Arc<RwLock<Map<String>>>,
}

impl GalileoState {
    pub fn new(
        window: Arc<Window>,
        device: Arc<Device>,
        surface: Arc<Surface<'static>>,
        queue: Arc<Queue>,
        config: SurfaceConfiguration,
    ) -> Self {
        let messenger = galileo::winit::WinitMessenger::new(window);

        let renderer = WgpuRenderer::new_with_device_and_surface(device, surface, queue, config);
        let renderer = Arc::new(RwLock::new(renderer));

        let input_handler = WinitInputHandler::default();

        let view = MapView::new(
            &latlon!(52.5, 13.3),
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

        let map = Arc::new(RwLock::new(Map::new_empty(Arc::new(RwLock::new(
            GalileoMap::new(view, vec![map_layer], Some(messenger)),
        )))));

        let map_positions = Arc::new(RwLock::new(MapPositions::new(
            map.read().expect("poisoned lock").map_ref(),
        )));

        GalileoState {
            input_handler,
            renderer,
            map,
        }
    }

    pub fn map(&self) -> Arc<RwLock<Map<String>>> {
        self.map.clone()
    }

    /// Returns pointers to current pointer position and last click position.
    pub fn positions(&self) -> Arc<RwLock<MapPositions>> {
        self.map.read().expect("poisoned lock").map_positions()
    }

    pub fn about_to_wait(&self) {
        self.map
            .read()
            .expect("poisoned lock")
            .map_write_lock()
            .unwrap()
            .animate();
    }

    pub fn resize(&self, size: PhysicalSize<u32>) {
        self.renderer
            .write()
            .expect("poisoned lock")
            .resize(Size::new(size.width, size.height));
        self.map
            .read()
            .expect("poisoned lock")
            .map_write_lock()
            .expect("poisoned lock")
            .set_size(Size::new((size.width) as f64, (size.height) as f64));
    }

    pub fn render(&self, wgpu_frame: &WgpuFrame<'_>) {
        let galileo_map = self.map.read().expect("poisoned lock");
        let galileo_map = galileo_map.map_read_lock().unwrap();
        galileo_map.load_layers();

        self.renderer
            .write()
            .expect("poisoned lock")
            .render_to_texture_view(&galileo_map, wgpu_frame.texture_view);
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
            self.map
                .write()
                .expect("poisoned lock")
                .handle_event(raw_event);
        }
    }
}

fn get_layer_style() -> VectorTileStyle {
    const STYLE: &str = "resources/map_styles/bright.json";
    let file = std::fs::File::open(STYLE).unwrap();
    serde_json::from_reader(file).unwrap()
}
