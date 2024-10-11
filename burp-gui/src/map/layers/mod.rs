use std::sync::{Arc, RwLock};

use galileo::{
    control::{EventPropagation, UserEvent, UserEventHandler},
    layer::Layer as GalileoLayer,
    Map,
};
pub mod line_layer;
pub mod node_layer;
pub mod poly_layer;

pub trait EventLayer: GalileoLayer {
    fn handle_event(&self, event: &UserEvent, map: &mut Map);
}

impl<T> EventLayer for Arc<RwLock<T>>
where
    T: EventLayer + 'static,
{
    fn handle_event(&self, event: &UserEvent, map: &mut Map) {
        self.read().expect("poisoned lock").handle_event(event, map)
    }
}

pub struct Layer<T>(Arc<RwLock<T>>);

impl<T> UserEventHandler for Layer<T>
where
    T: EventLayer,
{
    fn handle(&self, event: &UserEvent, map: &mut Map) -> EventPropagation {
        self.0
            .read()
            .expect("poisoned lock")
            .handle_event(event, map);
        EventPropagation::Propagate
    }
}
