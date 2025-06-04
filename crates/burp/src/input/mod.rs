pub mod geo_zero;

use geo_types::Coord;
use graph_rs::Coordinate;

#[derive(Clone, Debug)]
pub enum NodeValue {
    Coord(Coord),
    Poi { coord: Coord, name: String },
}

impl Coordinate for NodeValue {
    fn x_y(&self) -> (f64, f64) {
        match self {
            Self::Coord(coord) => coord.x_y(),
            Self::Poi { coord: c, name: _ } => c.x_y(),
        }
    }

    fn zero() -> Self {
        Self::Coord(Coord::zero())
    }

    fn as_coord(&self) -> Coord<f64> {
        Coord::from(self.x_y())
    }
}
