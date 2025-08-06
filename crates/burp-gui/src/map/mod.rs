use std::{
    borrow::{Borrow, BorrowMut},
    collections::HashMap,
    hash::Hash,
    sync::{Arc, PoisonError},
};

use ::galileo::{
    Map as GalileoMap,
    control::{
        EventProcessor, EventPropagation, MapController, MouseButton, MouseEvent, RawUserEvent,
        UserEvent,
    },
};
use galileo::{Messenger, layer::raster_tile_layer::RasterTileLayerBuilder};
use galileo_types::geo::impls::GeoPoint2d;
use layers::EventLayer;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

pub mod egui;
pub mod features;
pub mod layers;
pub mod symbols;

pub struct Map<K>
where
    K: Hash + Eq,
{
    map: GalileoMap,
    layers: FxHashMap<K, (Box<dyn EventLayer>, usize)>,
    event_processor: EventProcessor,
}

impl<K: Hash + Eq> Map<K> {
    pub fn new(map: GalileoMap, layers: FxHashMap<K, (Box<dyn EventLayer>, usize)>) -> Self {
        let mut event_processor = EventProcessor::default();

        event_processor.add_handler(move |ev: &UserEvent, map: &mut GalileoMap| {
            match ev {
                UserEvent::PointerMoved(MouseEvent {
                    screen_pointer_position,
                    ..
                }) => (),
                UserEvent::Click(
                    MouseButton::Left,
                    MouseEvent {
                        screen_pointer_position,
                        ..
                    },
                ) => {
                    ();
                }
                _ => (),
            }

            EventPropagation::Propagate
        });

        event_processor.add_handler(MapController::default());
        Self {
            map,
            layers,
            event_processor,
        }
    }

    pub fn new_empty(map: GalileoMap) -> Self {
        Self::new(map, HashMap::default())
    }

    pub fn or_insert(
        &mut self,
        key: K,
        layer: impl EventLayer + 'static,
    ) -> &mut Box<dyn EventLayer> {
        let layer_col = self.map.layers_mut();
        let layer = Arc::new(RwLock::new(layer));
        let layer_ref = layer.clone();

        &mut self
            .layers
            .entry(key)
            .or_insert_with(|| {
                layer_col.push(layer.clone());
                self.event_processor
                    .add_handler(move |event: &UserEvent, map: &mut GalileoMap| {
                        layer_ref.handle_event(event, map);
                        EventPropagation::Propagate
                    });
                log::debug!("Inserted layer. Total layers: {}", layer_col.len());
                (Box::new(layer), layer_col.len() - 1)
            })
            .0
    }

    pub fn get_layer<Q>(&self, key: &Q) -> Option<&dyn EventLayer>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.layers.get(key).map(|layer| layer.0.as_ref())
    }

    pub fn get_layer_mut<Q>(&mut self, key: &Q) -> Option<&mut Box<dyn EventLayer>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.layers.get_mut(key).map(|layer| &mut layer.0)
    }

    pub fn show_layer<Q>(&mut self, key: &Q) -> Result<(), String>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let index = self.layers.get(key).ok_or("Layer not found".to_string())?.1;
        self.map.layers_mut().show(index);

        Ok(())
    }

    pub fn hide_layer<Q>(&mut self, key: &Q) -> Result<(), String>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let index = self.layers.get(key).ok_or("Layer not found".to_string())?.1;
        self.map.layers_mut().hide(index);

        Ok(())
    }

    pub fn toggle_layer<Q>(&mut self, key: &Q) -> Result<(), String>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let index = self.layers.get(key).ok_or("Layer not found".to_string())?.1;
        let layers = self.map.layers_mut();
        if layers.is_visible(index) {
            layers.hide(index)
        } else {
            layers.show(index)
        };

        Ok(())
    }

    pub fn handle_event(&mut self, event: RawUserEvent) {
        self.event_processor.handle(event, &mut self.map);
    }

    pub fn add_handler(&mut self, handler: impl galileo::control::UserEventHandler + 'static) {
        self.event_processor.add_handler(handler)
    }

    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<(Box<dyn EventLayer>, usize)>
    where
        K: std::borrow::Borrow<Q>,
        Q: Hash + Eq,
    {
        self.layers.remove(k)
    }

    pub fn remove_entry<Q: ?Sized>(&mut self, k: &Q) -> Option<(K, (Box<dyn EventLayer>, usize))>
    where
        K: std::borrow::Borrow<Q>,
        Q: Hash + Eq,
    {
        self.layers.remove_entry(k)
    }

    pub fn map(&self) -> &GalileoMap {
        &self.map
    }

    pub fn map_mut(&mut self) -> &mut GalileoMap {
        &mut self.map
    }

    pub fn redraw(&self) {
        self.map.redraw()
    }
}

impl<K> Default for Map<K>
where
    K: Hash + Eq,
{
    fn default() -> Self {
        let tile_layer = RasterTileLayerBuilder::new_rest(|index| {
            format!(
                "https://api.maptiler.com/maps/openstreetmap/256/{}/{}/{}.jpg?key=8vBMrBmo8MIbxzh6yNkC",
                index.z, index.x, index.y
            )
        }).with_file_cache("./.tile_cache").build().unwrap();
        let map = galileo::MapBuilder::default()
            .with_latlon(52.5, 13.3)
            .with_layer(tile_layer)
            .build();

        Self::new(map, FxHashMap::default())
    }
}
