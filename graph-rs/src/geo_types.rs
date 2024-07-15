use geo::{point, Point};

use crate::Coordinate;

impl Coordinate for Point {
    fn x_y(&self) -> (f64, f64) {
        Point::x_y(*self)
    }

    fn as_coord(&self) -> geo_types::Coord<f64> {
        self.0
    }

    fn zero() -> Self {
        point!(x: 0.0, y: 0.0)
    }
}
