use core::panic;
use std::f64;
use std::sync::{Arc, RwLock};

use crate::run_ui::Positions;
use crate::state::WgpuFrame;
use ::geo_types::Geometry::{self, GeometryCollection, LineString, Point};
use ::geo_types::{coord, LineString as LineLineString};
use galileo::control::{EventPropagation, MouseEvent, UserEvent};
use galileo::layer::FeatureLayer;
use galileo::symbol::ArbitraryGeometrySymbol;
use galileo::{
    control::{EventProcessor, MapController},
    render::WgpuRenderer,
    tile_scheme::TileIndex,
    winit::WinitInputHandler,
    Map, MapBuilder, MapView, TileSchema,
};
use galileo_types::cartesian::{CartesianPoint2d, Point2d};
use galileo_types::geo::Crs;
use galileo_types::geometry_type::GeoSpace2d;
use galileo_types::{cartesian::Size, latlon};
use galileo_types::{Disambig, Disambiguate};
use geo::Coord;
use geozero::geojson::GeoJson;
use geozero::{geo_types, ToGeo};
use graph_rs::graph::csr::DirectedCsrGraph;
use graph_rs::input::geo_zero::geozero::geojson::read_geojson;
use graph_rs::input::geo_zero::GraphWriter;
use graph_rs::DirectedGraph;
use ordered_float::OrderedFloat;
use wgpu::{Device, Queue, Surface, SurfaceConfiguration};
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub struct GalileoState {
    input_handler: WinitInputHandler,
    event_processor: EventProcessor,
    renderer: Arc<RwLock<WgpuRenderer>>,
    map: Arc<RwLock<galileo::Map>>,
    pointer_position: Arc<RwLock<Point2d>>,
    hidden: bool,
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

        let pointer_position = Arc::new(RwLock::new(Point2d::default()));
        let pointer_position_clone = pointer_position.clone();

        let mut event_processor = EventProcessor::default();
        event_processor.add_handler(move |ev: &UserEvent, _map: &mut Map| {
            if let UserEvent::PointerMoved(MouseEvent {
                screen_pointer_position,
                ..
            }) = ev
            {
                *pointer_position_clone.write().expect("poisoned lock") = *screen_pointer_position;
            }

            EventPropagation::Propagate
        });
        event_processor.add_handler(MapController::default());

        let view = MapView::new(
            &latlon!(37.566, 126.9784),
            TileSchema::web(18).lod_resolution(8).unwrap(),
        );

        let tile_source = |index: &TileIndex| {
            format!(
                "https://tile.openstreetmap.org/{}/{}/{}.png",
                index.z, index.x, index.y
            )
        };

        let map_layer = Box::new(MapBuilder::create_raster_tile_layer(
            tile_source,
            TileSchema::web(18),
        ));

        let map = Arc::new(RwLock::new(galileo::Map::new(
            view,
            vec![map_layer],
            Some(messenger),
        )));

        GalileoState {
            input_handler,
            event_processor,
            renderer,
            map,
            pointer_position,
            hidden: false,
        }
    }

    pub fn build_graph_layer<T: DirectedGraph<OrderedFloat<f64>, Coord<OrderedFloat<f64>>>>(
        &self,
        g: &T,
    ) {
        let mut lines = Vec::new();
        for node in 0..g.node_count() {
            for edge in g.neighbors(node) {
                let line = LineLineString::new(vec![
                    coord! {x: g.node_value(node).unwrap().x.0, y: g.node_value(node).unwrap().y.0},
                    coord! {x: g.node_value(edge.target()).unwrap().x.0, y: g.node_value(edge.target()).unwrap().y.0},
                ]);
                lines.push(line.to_geo2d());
            }
        }

        let point_layer: FeatureLayer<
            _,
            Disambig<::geo_types::LineString, GeoSpace2d>,
            ArbitraryGeometrySymbol,
            GeoSpace2d,
        > = FeatureLayer::new(lines, ArbitraryGeometrySymbol::default(), Crs::WGS84);

        let mut map = self.map.write().unwrap();

        map.layers_mut().push(point_layer);
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

    pub fn positions(&self) -> Positions {
        let pointer_position = *self.pointer_position.read().expect("poisoned lock");
        let view = self.map.read().expect("poisoned lock").view().clone();
        Positions {
            pointer_position: view.screen_to_map_geo(pointer_position),
            map_center_position: view.position(),
        }
    }
}
