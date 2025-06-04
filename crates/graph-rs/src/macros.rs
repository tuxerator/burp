#[macro_export]
macro_rules! coord {
    (x: $x:expr, y: $y:expr $(,)* ) => {
        $crate::input::geo_zero::Coord { x: $x, y: $y }
    };
}
