use std::sync::{Arc, RwLock};

use galileo::Map;
use galileo_types::{
    cartesian::Point2d,
    geo::{impls::GeoPoint2d, Projection},
};

pub struct MapPositions {
    pointer_pos: Point2d,
    click_pos: Point2d,
    map: Arc<RwLock<Map>>,
}

impl MapPositions {
    pub fn new(map: Arc<RwLock<Map>>) -> Self {
        MapPositions {
            pointer_pos: Point2d::default(),
            click_pos: Point2d::default(),
            map,
        }
    }

    pub fn pointer_pos(&self) -> Option<GeoPoint2d> {
        self.map
            .read()
            .expect("poisoned lock")
            .view()
            .screen_to_map_geo(self.pointer_pos)
    }

    pub fn click_pos(&self) -> Option<GeoPoint2d> {
        self.map
            .read()
            .expect("poisoned lock")
            .view()
            .screen_to_map_geo(self.click_pos)
    }

    pub fn map_center_pos(&self) -> Option<GeoPoint2d> {
        self.map.read().expect("poisoned lock").view().position()
    }

    pub fn set_pointer_pos(&mut self, pointer_pos: Point2d) {
        self.pointer_pos = pointer_pos;
    }

    pub fn set_click_pos(&mut self, click_pos: Point2d) {
        self.click_pos = click_pos;
    }
}
