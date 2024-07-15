use geo_types::{Coord, CoordNum};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(remote = "Coord")]
pub struct CoordDef<T: CoordNum> {
    x: T,
    y: T,
}
