use core::fmt;
use std::fmt::Debug;

use crate::{graph::NodeTrait, serde::CoordDef};
use geo_types::Coord;
use graph_rs::Coordinate;
use num_traits::Num;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Poi {
    name: String,
    amenity: Amenity,
}

impl Poi {
    pub fn new(name: String, amenity: Amenity) -> Self {
        Self { name, amenity }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn amenity(&self) -> &Amenity {
        &self.amenity
    }
}

impl NodeTrait for Poi {}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Amenity {
    None,
    Bar,
    Biergarten,
    Cafe,
    FastFood,
    FoodCourt,
    IceCream,
    Pub,
    Restaurant,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CoordNode<T> {
    #[serde(with = "CoordDef")]
    coord: Coord,
    data: Vec<T>,
}

impl<T> CoordNode<T> {
    pub fn new(coord: Coord, data: Vec<T>) -> Self {
        Self { coord, data }
    }

    pub fn set_coord(&mut self, coord: Coord) {
        self.coord = coord;
    }

    pub fn get_coord(&self) -> &Coord {
        &self.coord
    }

    pub fn set_data(&mut self, data: Vec<T>) {
        self.data = data;
    }

    pub fn push_data(&mut self, data: T) {
        self.data.push(data);
    }

    pub fn append_data(&mut self, data: &mut Vec<T>) {
        self.data.append(data);
    }

    pub fn data(&self) -> &Vec<T> {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut Vec<T> {
        &mut self.data
    }

    pub fn has_data(&self) -> bool {
        !self.data.is_empty()
    }
}

impl<T: Debug> fmt::Display for CoordNode<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "coord: ({}, {}), data: {:?}",
            self.coord.x, self.coord.y, self.data
        )
    }
}

impl<T> Default for CoordNode<T> {
    fn default() -> Self {
        Self {
            coord: Coord::default(),
            data: Vec::default(),
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
            data: vec![],
        }
    }

    fn as_coord(&self) -> Coord {
        self.coord
    }
}
