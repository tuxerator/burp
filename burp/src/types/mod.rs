use core::fmt;
use std::fmt::{Debug, Display};

use crate::{graph::NodeTrait, serde::CoordDef};
use galileo::galileo_types::cartesian::NewCartesianPoint2d;
use geo::{coord, CoordNum};
use geo_types::Coord;
use graph_rs::Coordinate;
use num_traits::Num;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

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
pub struct CoordNode<C, T>
where
    C: CoordNum,
{
    coord: Coord<C>,
    data: Vec<T>,
}

impl<C, T> CoordNode<C, T>
where
    C: CoordNum,
{
    pub fn new(coord: Coord<C>, data: Vec<T>) -> Self {
        Self { coord, data }
    }

    pub fn set_coord(&mut self, coord: Coord<C>) {
        self.coord = coord;
    }

    pub fn get_coord(&self) -> &Coord<C> {
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

    pub fn map_coords<F, D>(self, mut f: F) -> CoordNode<D, T>
    where
        F: FnMut(C) -> D,
        D: CoordNum,
    {
        CoordNode::new(coord! {x: f(self.coord.x), y: f(self.coord.y)}, self.data)
    }
}

impl<C, T> fmt::Display for CoordNode<C, T>
where
    C: CoordNum + Display,
    T: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "coord: ({}, {}), data: {:?}",
            self.coord.x, self.coord.y, self.data
        )
    }
}

impl<C, T> Default for CoordNode<C, T>
where
    C: CoordNum + Default,
{
    fn default() -> Self {
        Self {
            coord: Coord::default(),
            data: Vec::default(),
        }
    }
}

impl<T, C> Coordinate<C> for CoordNode<C, T>
where
    C: qutee::Coordinate + Num,
{
    fn x_y(&self) -> (C, C) {
        self.coord.x_y()
    }

    fn zero() -> Self {
        Self {
            coord: Coord::zero(),
            data: vec![],
        }
    }

    fn as_coord(&self) -> Coord<C> {
        self.coord
    }
}
