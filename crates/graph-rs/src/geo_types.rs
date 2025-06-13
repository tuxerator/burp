use geo::{Coord, CoordNum, Point};

use crate::Coordinate;

impl<C: CoordNum> Coordinate<C> for Point<C> {
    fn x_y(&self) -> (C, C) {
        self.as_coord().x_y()
    }

    fn as_coord(&self) -> geo_types::Coord<C> {
        self.0
    }

    fn zero() -> Self {
        Point::new(C::zero(), C::zero())
    }
}

impl<C: CoordNum> Coordinate<C> for Coord<C> {
    fn x_y(&self) -> (C, C) {
        self.x_y()
    }

    fn as_coord(&self) -> Coord<C> {
        *self
    }

    fn zero() -> Self {
        Coord::zero()
    }
}
