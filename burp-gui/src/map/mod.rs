use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, RwLock},
};

use galileo::layer::Layer;

mod layers;
mod symbols;

struct Map<K>
where
    K: Hash + Eq,
{
    map: Arc<RwLock<galileo::Map>>,
    layers: HashMap<K, Box<dyn Layer>>,
}

impl<K: Hash + Eq> Map<K> {
    pub fn new(map: Arc<RwLock<galileo::Map>>, layers: HashMap<K, Box<dyn Layer>>) -> Self {
        Self { map, layers }
    }

    pub fn new_empty(map: Arc<RwLock<galileo::Map>>) -> Self {
        Self {
            map,
            layers: HashMap::default(),
        }
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
}
