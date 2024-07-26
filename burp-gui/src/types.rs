use std::sync::{Arc, RwLock};

use galileo::Map;
use galileo_types::{
    cartesian::Point2d,
    geo::{impls::GeoPoint2d, Projection},
};

pub struct PointerPos {
    screen_pos: Point2d,
    geo_pos: Option<GeoPoint2d>,
    map: Arc<RwLock<Map>>,
}

impl PointerPos {
    pub fn new_from_screen_pos(screen_pos: Point2d, map: Arc<RwLock<Map>>) -> Self {
        let geo_pos = map
            .read()
            .expect("poisoned lock")
            .view()
            .screen_to_map_geo(screen_pos);
        Self {
            screen_pos,
            geo_pos,
            map,
        }
    }

    pub fn screen_pos(&self) -> Point2d {
        self.screen_pos
    }

    pub fn set_screen_pos(&mut self, screen_pos: Point2d) {
        self.screen_pos = screen_pos;
        self.geo_pos = self
            .map
            .read()
            .expect("poisoned lock")
            .view()
            .screen_to_map_geo(self.screen_pos);
    }

    pub fn geo_pos(&self) -> Option<GeoPoint2d> {
        self.geo_pos
    }
}
