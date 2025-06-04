use geo::{point, Coord, CoordNum, Point};

use crate::Coordinate;

impl<C: CoordNum> Coordinate<C> for Point<C> {
    fn x_y(&self) -> (C, C) {
        self.x_y()
    }

    fn as_coord(&self) -> geo_types::Coord<C> {
        self.0
    }

    fn zero() -> Self {
        Point::zero()
    }
}

impl<C: CoordNum> Coordinate<C> for Coord<C> {
    fn x_y(&self) -> (C, C) {
        self.x_y()
    }

    fn as_coord(&self) -> Coord<C> {
        self.clone()
    }

    fn zero() -> Self {
        Coord::zero()
    }
}
