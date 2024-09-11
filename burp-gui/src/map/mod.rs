use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use ::galileo::{
    control::{
        EventProcessor, EventPropagation, MapController, MouseButton, MouseEvent, RawUserEvent,
        UserEvent,
    },
    layer::Layer,
    Map as GalileoMap,
};
use burp::galileo::{self};
use galileo_types::geo::{self, impls::GeoPoint2d};

use crate::types::MapPositions;

mod layers;
mod symbols;

pub struct Map<K>
where
    K: Hash + Eq,
{
    map: Arc<RwLock<GalileoMap>>,
    layers: HashMap<K, Box<dyn Layer>>,
    map_positions: Arc<RwLock<MapPositions>>,
    event_processor: EventProcessor,
}

impl<K: Hash + Eq> Map<K> {
    pub fn new(map: Arc<RwLock<GalileoMap>>, layers: HashMap<K, Box<dyn Layer>>) -> Self {
        let map_positions = Arc::new(RwLock::new(MapPositions::new(map.clone())));
        let map_positions_clone = map_positions.clone();
        let mut event_processor = EventProcessor::default();

        event_processor.add_handler(move |ev: &UserEvent, map: &mut GalileoMap| {
            match ev {
                UserEvent::PointerMoved(MouseEvent {
                    screen_pointer_position,
                    ..
                }) => map_positions_clone
                    .write()
                    .expect("poisoned lock")
                    .set_pointer_pos(*screen_pointer_position),
                UserEvent::Click(
                    MouseButton::Left,
                    MouseEvent {
                        screen_pointer_position,
                        ..
                    },
                ) => {
                    {
                        map_positions_clone
                            .write()
                            .expect("poisoned lock")
                            .set_click_pos(*screen_pointer_position);
                    }
                    let map_positions = map_positions_clone.read().expect("poisoned lock");
                }
                _ => (),
            }

            EventPropagation::Propagate
        });
        event_processor.add_handler(MapController::default());
        Self {
            map,
            layers,
            map_positions,
            event_processor,
        }
    }

    pub fn new_empty(map: Arc<RwLock<GalileoMap>>) -> Self {
        Self::new(map, HashMap::default())
    }

    pub fn map_ref(&self) -> Arc<RwLock<GalileoMap>> {
        self.map.clone()
    }

    pub fn map_read_lock(
        &self,
    ) -> Result<RwLockReadGuard<'_, GalileoMap>, PoisonError<RwLockReadGuard<'_, GalileoMap>>> {
        self.map.read()
    }

    pub fn map_write_lock(
        &self,
    ) -> Result<RwLockWriteGuard<'_, GalileoMap>, PoisonError<RwLockWriteGuard<'_, GalileoMap>>>
    {
        self.map.write()
    }

    pub fn map_positions(&self) -> Arc<RwLock<MapPositions>> {
        self.map_positions.clone()
    }

    pub fn pointer_pos(
        &self,
    ) -> Result<Option<GeoPoint2d>, PoisonError<RwLockReadGuard<'_, MapPositions>>> {
        let map_positions = self.map_positions.read()?;

        Ok(map_positions.pointer_pos())
    }

    pub fn click_pos(
        &self,
    ) -> Result<Option<GeoPoint2d>, PoisonError<RwLockReadGuard<'_, MapPositions>>> {
        let map_positions = self.map_positions.read()?;

        Ok(map_positions.click_pos())
    }

    pub fn take_click_pos(
        &self,
    ) -> Result<Option<GeoPoint2d>, PoisonError<RwLockWriteGuard<'_, MapPositions>>> {
        let mut map_positions = self.map_positions.write()?;

        Ok(map_positions.take_click_pos())
    }

    pub fn insert(&mut self, key: K, layer: impl Layer + 'static) {
        let mut map = self.map.write().expect("poisoned lock");
        let layer_col = map.layers_mut();
        let layer = Arc::new(RwLock::new(layer));

        layer_col.push(layer.clone());
        self.layers.insert(key, Box::new(layer));
    }

    pub fn get_layer(&self, key: K) -> Option<&Box<dyn Layer>> {
        self.layers.get(&key)
    }

    pub fn get_layer_mut(&mut self, key: K) -> Option<&mut Box<dyn Layer>> {
        self.layers.get_mut(&key)
    }

    pub fn handle_event(
        &mut self,
        event: RawUserEvent,
    ) -> Result<(), PoisonError<RwLockWriteGuard<'_, GalileoMap>>> {
        let mut map = self.map.write()?;
        self.event_processor.handle(event, &mut map);

        Ok(())
    }
}
