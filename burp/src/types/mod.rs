use core::fmt;

use crate::serde::CoordDef;
use geo_types::Coord;
use graph_rs::Coordinate;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Poi {
    name: String,
    amenity: Amenity,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Amenity {
    Bar,
    Cafe,
    FastFood,
    FoodCourt,
    IceCream,
    Pub,
    Restaurant,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CoordNode<T> {
    #[serde(with = "CoordDef")]
    coord: Coord,
    data: Option<T>,
}

impl<T> CoordNode<T> {
    pub fn new(coord: Coord, data: Option<T>) -> Self {
        Self { coord, data }
    }

    pub fn set_coord(&mut self, coord: Coord) {
        self.coord = coord;
    }

    pub fn get_coord(&self) -> &Coord {
        &self.coord
    }

    pub fn set_data(&mut self, data: Option<T>) {
        self.data = data;
    }

    pub fn data(&self) -> &Option<T> {
        &self.data
    }
}

impl<T> fmt::Display for CoordNode<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.coord.x, self.coord.y)
    }
}

impl<T> Default for CoordNode<T> {
    fn default() -> Self {
        Self {
            coord: Coord::default(),
            data: None,
        }
    }
}

impl<T> Coordinate for CoordNode<T> {
    fn x_y(&self) -> (f64, f64) {
        self.coord.x_y()
    }

    fn zero() -> Self {
        Self {
            coord: Coord::zero(),
            data: None,
        }
    }

    fn as_coord(&self) -> Coord<f64> {
        self.coord
    }
}
