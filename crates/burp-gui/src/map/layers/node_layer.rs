use std::fmt::Debug;

use galileo::{
    Map,
    control::{MouseButton, UserEvent},
    layer::{FeatureLayer, Layer as GalileoLayer, feature_layer::Feature},
    symbol::{CirclePointSymbol, Symbol},
};
use galileo_types::{
    Disambig, Geometry,
    cartesian::Point2,
    geo::{Crs, impls::GeoPoint2d},
    geometry::Geom,
    geometry_type::{CartesianSpace2d, GeoSpace2d},
};
use geo::Coord;
use log::info;
use maybe_sync::{MaybeSend, MaybeSync};

use super::EventLayer;

pub struct NodeMarker<T> {
    coord: GeoPoint2d,
    node: usize,
    data: Option<Vec<T>>,
}

impl<T> NodeMarker<T> {
    pub fn new(coord: Coord, node: usize, data: Option<Vec<T>>) -> Option<Self> {
        Some(Self {
            coord: GeoPoint2d::from(&coord),
            node,
            data,
        })
    }

    pub fn node(&self) -> usize {
        self.node
    }

    pub fn data(&self) -> Option<&Vec<T>> {
        self.data.as_ref()
    }
}

impl<T> Feature for NodeMarker<T> {
    type Geom = GeoPoint2d;
    fn geometry(&self) -> &Self::Geom {
        &self.coord
    }
}

struct NodeSymbol {
    point_symbol: CirclePointSymbol,
}

impl<T> Symbol<NodeMarker<T>> for NodeSymbol {
    fn render(
        &self,
        feature: &NodeMarker<T>,
        geometry: &galileo_types::geometry::Geom<galileo_types::cartesian::Point3>,
        min_resolution: f64,
        bundle: &mut galileo::render::render_bundle::RenderBundle,
    ) {
        self.point_symbol
            .render(feature, geometry, min_resolution, bundle);
    }
}

pub struct NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>>,
{
    layer: FeatureLayer<GeoPoint2d, NodeMarker<T>, S, GeoSpace2d>,
}

impl<S, T> NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>>,
    T: MaybeSend + MaybeSync + 'static,
{
    pub fn new(style: S, crs: Crs) -> Self {
        Self {
            layer: FeatureLayer::new(vec![], style, crs),
        }
    }

    pub fn insert_node(&mut self, node: NodeMarker<T>) {
        self.layer.features_mut().add(node);
    }

    pub fn insert_nodes(&mut self, nodes: Vec<NodeMarker<T>>) {
        nodes.into_iter().for_each(|node| self.insert_node(node));
    }
}

impl<S, T> GalileoLayer for NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>> + MaybeSend + MaybeSync + 'static,
    T: MaybeSend + MaybeSync + 'static,
{
    fn render(&self, view: &galileo::MapView, canvas: &mut dyn galileo::render::Canvas) {
        self.layer.render(view, canvas)
    }

    fn prepare(&self, view: &galileo::MapView) {
        self.layer.prepare(view)
    }

    fn set_messenger(&mut self, messenger: Box<dyn galileo::Messenger>) {
        self.layer.set_messenger(messenger)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attribution(&self) -> Option<galileo::layer::attribution::Attribution> {
        None
    }
}

impl<S, T> EventLayer for NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>> + MaybeSend + MaybeSync + 'static,
    T: MaybeSend + MaybeSync + Debug + 'static,
{
    fn handle_event(&self, event: &UserEvent, map: &mut Map) {
        match event {
            UserEvent::Click(MouseButton::Left, event_data) => {
                let Some(position) = map.view().screen_to_map(event_data.screen_pointer_position)
                else {
                    return;
                };
            }
            _ => (),
        };
    }
}
