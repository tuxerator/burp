use std::fmt::Debug;

use galileo::{
    control::{MouseButton, UserEvent},
    layer::{feature_layer::Feature, FeatureLayer, Layer as GalileoLayer},
    symbol::Symbol,
    Map,
};
use galileo_types::{geo::Crs, geometry_type::CartesianSpace2d, Disambig, Geometry};
use geo::Coord;
use log::info;
use maybe_sync::{MaybeSend, MaybeSync};

use super::EventLayer;

pub struct NodeMarker<T> {
    coord: Disambig<Coord, CartesianSpace2d>,
    node: usize,
    data: Option<Vec<T>>,
}

impl<T> NodeMarker<T> {
    pub fn new(coord: Coord, node: usize, data: Option<Vec<T>>) -> Option<Self> {
        let projection = Crs::EPSG3857.get_projection()?;
        Some(Self {
            coord: projection.project(&coord)?,
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
    type Geom = Disambig<Coord, CartesianSpace2d>;
    fn geometry(&self) -> &Self::Geom {
        &self.coord
    }
}

pub struct NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>>,
{
    layer: FeatureLayer<
        <Disambig<Coord, CartesianSpace2d> as Geometry>::Point,
        NodeMarker<T>,
        S,
        CartesianSpace2d,
    >,
}

impl<S, T> NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>>,
{
    pub fn new(style: S) -> Self {
        Self {
            layer: FeatureLayer::new(vec![], style, Crs::EPSG3857),
        }
    }

    pub fn insert_node(&mut self, node: NodeMarker<T>) {
        self.layer.features_mut().insert(node);
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
                for feature in self
                    .layer
                    .get_features_at(&position, map.view().resolution() * 20.0)
                {
                    info!("Data: {:?}", feature.as_ref().data);
                }
            }
            _ => (),
        };
    }
}
