use std::sync::{Arc, RwLock};

use galileo::Map;
use galileo_types::{
    cartesian::Point2d,
    geo::impls::GeoPoint2d,
};

pub struct MapPositions {
    pointer_pos: Point2d,
    click_pos: Option<Point2d>,
    map: Arc<RwLock<Map>>,
}

impl MapPositions {
    pub fn new(map: Arc<RwLock<Map>>) -> Self {
        MapPositions {
            pointer_pos: Point2d::default(),
            click_pos: None,
            map,
        }
    }

    pub fn pointer_pos(&self) -> Option<GeoPoint2d> {
        self.map
            .try_read()
            .ok()
            .and_then(|map| map.view().screen_to_map_geo(self.pointer_pos))
    }

    pub fn click_pos(&self) -> Option<GeoPoint2d> {
        self.click_pos.and_then(|click_pos| {
            self.map
                .try_read()
                .ok()
                .and_then(|map| map.view().screen_to_map_geo(click_pos))
        })
    }

    pub fn take_click_pos(&mut self) -> Option<GeoPoint2d> {
        self.click_pos.take().and_then(|click_pos| {
            self.map
                .try_read()
                .ok()
                .and_then(|map| map.view().screen_to_map_geo(click_pos))
        })
    }

    pub fn map_center_pos(&self) -> Option<GeoPoint2d> {
        self.map
            .try_read()
            .ok()
            .and_then(|map| map.view().position())
    }

    pub fn set_pointer_pos(&mut self, pointer_pos: Point2d) {
        self.pointer_pos = pointer_pos;
    }

    pub fn set_click_pos(&mut self, click_pos: Point2d) {
        self.click_pos = Some(click_pos);
    }
}
