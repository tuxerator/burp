use std::rc::Rc;

use geo_types::{Coord, CoordNum};
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(remote = "Coord")]
pub struct CoordDef<T: CoordNum> {
    x: T,
    y: T,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "OrderedFloat")]
pub struct OrderedFloatDef<T>(T);

#[derive(Serialize, Deserialize)]
struct Test {
    #[serde(with = "OrderedFloatDef")]
    test: OrderedFloat<i32>,
}
