pub use geozero;
use log::info;
use ordered_float::OrderedFloat;

use std::{
    cmp::Ordering,
    collections::{self, HashMap},
    io::Read,
    iter::Map,
    marker::PhantomData,
    mem,
    ops::Deref,
    sync::{Arc, RwLock},
};

use geo::{
    coord, line_string, point, Centroid, EuclideanDistance, GeodesicDistance, HaversineDistance,
};
use geo_types::{
    Coord, Geometry, GeometryCollection, LineString, MultiLineString, MultiPoint, MultiPolygon,
    Point, Polygon,
};
use geozero::{
    error::GeozeroError, geojson::GeoJson, ColumnValue, FeatureProcessor, GeomProcessor,
    PropertyProcessor,
};

use graph_rs::{graph::csr::DirectedCsrGraph, CoordGraph, Coordinate, DirectedGraph, Graph};

use graph_rs::input::edgelist::EdgeList;

use crate::{
    galileo::GalileoMap,
    oracle::Oracle,
    types::{Amenity, CoordNode, Poi},
};

use super::NodeValue;

pub struct GraphWriter<F>
where
    F: Fn(&HashMap<String, ColumnValueClonable>) -> bool,
{
    node_map: HashMap<Coord<OrderedFloat<f64>>, usize>,
    nodes: Vec<CoordNode<Poi>>,
    edges: Vec<(usize, usize, f64)>,
    line: Vec<(usize, usize, f64)>,
    coords: Option<Vec<Coord>>,
    index: usize,
    property_filter: F,
    properties: HashMap<String, ColumnValueClonable>,
    include_feature: bool,
    map: Option<GalileoMap>,
}

pub fn read_geojson<R, P>(reader: R, processor: &mut P) -> Result<(), GeozeroError>
where
    R: Read,
    P: FeatureProcessor,
{
    geozero::geojson::read_geojson(reader, processor)
}

impl<F> GraphWriter<F>
where
    F: Fn(&HashMap<String, ColumnValueClonable>) -> bool,
{
    pub fn new(property_filter: F, map: Option<GalileoMap>) -> Self {
        GraphWriter {
            node_map: HashMap::default(),
            nodes: Vec::default(),
            edges: Vec::default(),
            line: Vec::default(),
            coords: None,
            index: usize::default(),
            property_filter,
            properties: HashMap::default(),
            include_feature: true,
            map,
        }
    }

    pub fn new_from_filter(property_filter: F) -> Self {
        GraphWriter::new(property_filter, None)
    }

    pub fn new_from(graph_writer: Self) -> Self {
        graph_writer
    }

    pub fn get_graph(&mut self) -> DirectedCsrGraph<f64, CoordNode<Poi>> {
        let edge_list = EdgeList::new(mem::take(&mut self.edges));

        let graph = DirectedCsrGraph::from(edge_list);

        DirectedCsrGraph::new(
            mem::take(&mut self.nodes).into_boxed_slice(),
            graph.csr_out,
            graph.csr_inc,
        )
    }
}

impl<F> GeomProcessor for GraphWriter<F>
where
    F: Fn(&HashMap<String, ColumnValueClonable>) -> bool,
{
    fn xy(&mut self, x: f64, y: f64, idx: usize) -> geozero::error::Result<()> {
        if !self.include_feature {
            return Ok(());
        }
        let coords = self
            .coords
            .as_mut()
            .ok_or(GeozeroError::Geometry("Not ready for coords".to_string()))?;

        let coord = coord! {x: x, y: y};
        let ord_coord = coord! {x: OrderedFloat(x), y: OrderedFloat(y)};
        coords.push(coord);

        if let std::collections::hash_map::Entry::Vacant(e) = self.node_map.entry(ord_coord) {
            e.insert(self.index);
            self.nodes.push(CoordNode::new(coord, vec![]));
            self.index += 1;
        }
        Ok(())
    }

    fn point_begin(&mut self, idx: usize) -> geozero::error::Result<()> {
        self.coords = Some(Vec::with_capacity(1));
        Ok(())
    }

    fn point_end(&mut self, idx: usize) -> geozero::error::Result<()> {
        let coords = self
            .coords
            .take()
            .ok_or(GeozeroError::Geometry("No coords for Point".to_string()))?;

        Ok(())
    }

    fn linestring_begin(
        &mut self,
        tagged: bool,
        size: usize,
        idx: usize,
    ) -> geozero::error::Result<()> {
        debug_assert!(self.coords.is_none());
        self.coords = Some(Vec::with_capacity(size));
        Ok(())
    }

    fn linestring_end(&mut self, tagged: bool, idx: usize) -> geozero::error::Result<()> {
        if !self.include_feature {
            self.coords.take();
            return Ok(());
        }
        let mut coords = self
            .coords
            .take()
            .ok_or(GeozeroError::Geometry("No coords in LineSting".to_string()))?
            .into_iter();

        let mut coord_a = coords.next().unwrap();
        for coord_b in coords {
            let node_a =
                self.node_map
                    .get(&to_ord_coord(&coord_a))
                    .ok_or(GeozeroError::Geometry(
                        "Coord not processed yet".to_string(),
                    ))?;
            let node_b =
                self.node_map
                    .get(&to_ord_coord(&coord_b))
                    .ok_or(GeozeroError::Geometry(
                        "Coord not processed yet".to_string(),
                    ))?;

            let p_a: Point = coord_a.into();
            let p_b: Point = coord_b.into();

            let d = p_a.haversine_distance(&p_b);

            self.edges.push((*node_a, *node_b, d));

            if let Some(ref map) = self.map {
                map.draw_line(line_string![p_a.into(), p_b.into()]);
            }

            coord_a = coord_b;
        }

        Ok(())
    }
}

impl<F> FeatureProcessor for GraphWriter<F>
where
    F: Fn(&HashMap<String, ColumnValueClonable>) -> bool,
{
    fn feature_begin(&mut self, idx: u64) -> geozero::error::Result<()> {
        self.include_feature = true;
        self.properties = HashMap::default();

        Ok(())
    }

    fn properties_end(&mut self) -> geozero::error::Result<()> {
        self.include_feature = (self.property_filter)(&self.properties);
        Ok(())
    }
}

impl<F> PropertyProcessor for GraphWriter<F>
where
    F: Fn(&HashMap<String, ColumnValueClonable>) -> bool,
{
    fn property(&mut self, i: usize, n: &str, v: &ColumnValue) -> geozero::error::Result<bool> {
        let value = ColumnValueClonable::from(v);

        self.properties.insert(n.to_string(), value);
        Ok(false) // don't abort
    }
}

#[derive(Debug)]
pub enum ColumnValueClonable {
    Bool(bool),
    Binary(Vec<u8>),
    Byte(i8),
    UByte(u8),
    Short(i16),
    UShort(u16),
    Int(i32),
    UInt(u32),
    Long(i64),
    ULong(u64),
    Float(f32),
    Double(f64),
    String(String),
    DateTime(String),
    Json(String),
}

impl<'a> From<&ColumnValue<'a>> for ColumnValueClonable {
    fn from(value: &ColumnValue<'a>) -> Self {
        match value {
            ColumnValue::Bool(i) => ColumnValueClonable::Bool(*i),
            ColumnValue::Binary(i) => ColumnValueClonable::Binary(i.to_vec()),
            ColumnValue::Byte(i) => ColumnValueClonable::Byte(*i),
            ColumnValue::UByte(i) => ColumnValueClonable::UByte(*i),
            ColumnValue::Short(i) => ColumnValueClonable::Short(*i),
            ColumnValue::UShort(i) => ColumnValueClonable::UShort(*i),
            ColumnValue::Int(i) => ColumnValueClonable::Int(*i),
            ColumnValue::UInt(i) => ColumnValueClonable::UInt(*i),
            ColumnValue::Long(i) => ColumnValueClonable::Long(*i),
            ColumnValue::ULong(i) => ColumnValueClonable::ULong(*i),
            ColumnValue::Float(i) => ColumnValueClonable::Float(*i),
            ColumnValue::Double(i) => ColumnValueClonable::Double(*i),
            ColumnValue::String(i) => ColumnValueClonable::String(i.to_string()),
            ColumnValue::DateTime(i) => ColumnValueClonable::DateTime(i.to_string()),
            ColumnValue::Json(i) => ColumnValueClonable::Json(i.to_string()),
        }
    }
}

impl Clone for ColumnValueClonable {
    fn clone(&self) -> Self {
        match self {
            ColumnValueClonable::Bool(i) => ColumnValueClonable::Bool(*i),
            ColumnValueClonable::Binary(i) => ColumnValueClonable::Binary(i.clone()),
            ColumnValueClonable::Byte(i) => ColumnValueClonable::Byte(*i),
            ColumnValueClonable::UByte(i) => ColumnValueClonable::UByte(*i),
            ColumnValueClonable::Short(i) => ColumnValueClonable::Short(*i),
            ColumnValueClonable::UShort(i) => ColumnValueClonable::UShort(*i),
            ColumnValueClonable::Int(i) => ColumnValueClonable::Int(*i),
            ColumnValueClonable::UInt(i) => ColumnValueClonable::UInt(*i),
            ColumnValueClonable::Long(i) => ColumnValueClonable::Long(*i),
            ColumnValueClonable::ULong(i) => ColumnValueClonable::ULong(*i),
            ColumnValueClonable::Float(i) => ColumnValueClonable::Float(*i),
            ColumnValueClonable::Double(i) => ColumnValueClonable::Double(*i),
            ColumnValueClonable::String(i) => ColumnValueClonable::String(i.clone()),
            ColumnValueClonable::DateTime(i) => ColumnValueClonable::DateTime(i.clone()),
            ColumnValueClonable::Json(i) => ColumnValueClonable::Json(i.clone()),
        }
    }
}

pub struct PoiWriter<F>
where
    F: Fn(&HashMap<String, ColumnValueClonable>) -> bool,
{
    coord: Option<Coord>,
    coords: Option<Vec<Coord>>,
    line_strings: Option<Vec<LineString>>,
    geom: Option<GeometryCollection>,
    geoms: Vec<Geometry<f64>>,
    collections: Vec<Vec<Geometry<f64>>>,
    polygons: Option<Vec<Polygon>>,
    property_filter: F,
    properties: Option<HashMap<String, ColumnValueClonable>>,
    pois: Vec<CoordNode<Poi>>,
    include_feature: bool,
}

impl<F> PoiWriter<F>
where
    F: Fn(&HashMap<String, ColumnValueClonable>) -> bool,
{
    pub fn new(property_filter: F) -> Self {
        PoiWriter {
            coord: None,
            coords: None,
            line_strings: None,
            geom: None,
            geoms: Vec::default(),
            collections: Vec::default(),
            polygons: None,
            property_filter,
            properties: None,
            pois: Vec::default(),
            include_feature: true,
        }
    }
    fn finish_geometry(&mut self, geometry: Geometry<f64>) -> geozero::error::Result<()> {
        // Add the geometry to a collection if we're in the middle of processing
        // a (potentially nested) collection
        if let Some(most_recent_collection) = self.collections.last_mut() {
            most_recent_collection.push(geometry);
        } else {
            self.geoms.push(geometry);
        }
        Ok(())
    }

    pub fn pois(&self) -> &[CoordNode<Poi>] {
        &self.pois
    }
}

impl<F> FeatureProcessor for PoiWriter<F>
where
    F: Fn(&HashMap<String, ColumnValueClonable>) -> bool,
{
    fn feature_begin(&mut self, idx: u64) -> geozero::error::Result<()> {
        self.include_feature = true;

        Ok(())
    }

    fn feature_end(&mut self, idx: u64) -> geozero::error::Result<()> {
        let geom = GeometryCollection(self.geoms.to_vec());
        self.geoms = Vec::default();
        let center_coord = geom.centroid().ok_or(GeozeroError::FeatureGeometry(
            "Could not compute centroid for this Feature".to_string(),
        ))?;

        let properties = self
            .properties
            .take()
            .ok_or(GeozeroError::Properties("No properties found".to_string()))?;

        if let Some(ColumnValueClonable::String(poi_name)) = properties.get("name") {
            let amenity =
                if let Some(ColumnValueClonable::String(amenity)) = properties.get("amenity") {
                    match amenity.as_str() {
                        "bar" => Amenity::Bar,
                        "biergarten" => Amenity::Biergarten,
                        "cafe" => Amenity::Cafe,
                        "fast_food" => Amenity::FastFood,
                        "food_court" => Amenity::FoodCourt,
                        "pub" => Amenity::Pub,
                        "ice_cream" => Amenity::IceCream,
                        "restaurant" => Amenity::Restaurant,
                        _ => Amenity::None,
                    }
                } else {
                    Amenity::None
                };
            let poi = CoordNode::new(
                center_coord.into(),
                vec![Poi::new(poi_name.to_string(), amenity)],
            );
            self.pois.push(poi);
        }

        Ok(())
    }

    fn properties_begin(&mut self) -> geozero::error::Result<()> {
        debug_assert!(self.properties.is_none());
        self.properties = Some(HashMap::default());
        Ok(())
    }

    fn properties_end(&mut self) -> geozero::error::Result<()> {
        if let Some(ref properties) = self.properties {
            self.include_feature = (self.property_filter)(properties);
        }
        Ok(())
    }
}

impl<F> PropertyProcessor for PoiWriter<F>
where
    F: Fn(&HashMap<String, ColumnValueClonable>) -> bool,
{
    fn property(&mut self, i: usize, n: &str, v: &ColumnValue) -> geozero::error::Result<bool> {
        let value = ColumnValueClonable::from(v);
        if let Some(ref mut properties) = self.properties {
            properties.insert(n.to_string(), value);
        }

        Ok(false) // don't abort
    }
}

impl<F> GeomProcessor for PoiWriter<F>
where
    F: Fn(&HashMap<String, ColumnValueClonable>) -> bool,
{
    fn xy(&mut self, x: f64, y: f64, idx: usize) -> geozero::error::Result<()> {
        info!("Processing coord");
        if !self.include_feature {
            return Ok(());
        }
        let coords = self
            .coords
            .as_mut()
            .ok_or(GeozeroError::Geometry("Not ready for coords".to_string()))?;

        let coord = coord! {x: x, y: y};
        coords.push(coord);

        Ok(())
    }

    fn point_begin(&mut self, idx: usize) -> geozero::error::Result<()> {
        self.coords = Some(Vec::with_capacity(1));
        Ok(())
    }

    fn point_end(&mut self, idx: usize) -> geozero::error::Result<()> {
        let coords = self
            .coords
            .take()
            .ok_or(GeozeroError::Geometry("No coords for Point".to_string()))?;

        debug_assert!(coords.len() == 1);

        self.finish_geometry(Point(coords[0]).into())
    }

    fn multipoint_begin(&mut self, size: usize, _idx: usize) -> geozero::error::Result<()> {
        debug_assert!(self.coords.is_none());
        self.coords = Some(Vec::with_capacity(size));
        Ok(())
    }

    fn multipoint_end(&mut self, _idx: usize) -> geozero::error::Result<()> {
        let coords = self.coords.take().ok_or(GeozeroError::Geometry(
            "No coords for MultiPoint".to_string(),
        ))?;
        let points: Vec<Point<_>> = coords.into_iter().map(From::from).collect();
        self.finish_geometry(MultiPoint(points).into())
    }

    fn linestring_begin(
        &mut self,
        tagged: bool,
        size: usize,
        idx: usize,
    ) -> geozero::error::Result<()> {
        debug_assert!(self.coords.is_none());
        self.coords = Some(Vec::with_capacity(size));
        Ok(())
    }

    fn linestring_end(&mut self, tagged: bool, idx: usize) -> geozero::error::Result<()> {
        if !self.include_feature {
            self.coords.take();
            return Ok(());
        }
        let mut coords = self
            .coords
            .take()
            .ok_or(GeozeroError::Geometry("No coords in LineSting".to_string()))?;

        let line_string = LineString(coords);

        if tagged {
            self.finish_geometry(line_string.into())?;
        } else {
            let line_strings = self.line_strings.as_mut().ok_or(GeozeroError::Geometry(
                "Missing container for LineString".to_string(),
            ))?;

            line_strings.push(line_string);
        }

        Ok(())
    }

    fn multilinestring_begin(&mut self, size: usize, idx: usize) -> geozero::error::Result<()> {
        debug_assert!(self.line_strings.is_none());
        self.line_strings = Some(Vec::with_capacity(size));
        Ok(())
    }

    fn multilinestring_end(&mut self, idx: usize) -> geozero::error::Result<()> {
        let line_strings = self.line_strings.take().ok_or(GeozeroError::Geometry(
            "No LineStrings for MultiLineString".to_string(),
        ))?;
        self.finish_geometry(MultiLineString(line_strings).into())
    }

    fn polygon_begin(
        &mut self,
        tagged: bool,
        size: usize,
        idx: usize,
    ) -> geozero::error::Result<()> {
        debug_assert!(self.line_strings.is_none());
        self.line_strings = Some(Vec::with_capacity(size));

        Ok(())
    }

    fn polygon_end(&mut self, tagged: bool, idx: usize) -> geozero::error::Result<()> {
        if !self.include_feature {
            self.line_strings.take();
            return Ok(());
        }

        let mut line_strings = self.line_strings.take().ok_or(GeozeroError::Geometry(
            "Missing LineStrings for Polygon".to_string(),
        ))?;

        let polygon = if line_strings.is_empty() {
            Polygon::new(LineString(vec![]), vec![])
        } else {
            let exterior = line_strings.remove(0);
            Polygon::new(exterior, mem::take(&mut line_strings))
        };

        if tagged {
            self.finish_geometry(polygon.into())?;
        } else {
            let polygons = self.polygons.as_mut().ok_or(GeozeroError::Geometry(
                "Missing container for Polygon".to_string(),
            ))?;
            polygons.push(polygon);
        }

        Ok(())
    }

    fn multipolygon_begin(&mut self, size: usize, idx: usize) -> geozero::error::Result<()> {
        debug_assert!(self.polygons.is_none());
        self.polygons = Some(Vec::with_capacity(size));
        Ok(())
    }

    fn multipolygon_end(&mut self, idx: usize) -> geozero::error::Result<()> {
        let polygons = self.polygons.take().ok_or(GeozeroError::Geometry(
            "Missing polygons for MultiPolygon".to_string(),
        ))?;
        self.finish_geometry(MultiPolygon(polygons).into())
    }

    fn geometrycollection_begin(&mut self, size: usize, idx: usize) -> geozero::error::Result<()> {
        self.collections.push(Vec::with_capacity(size));
        Ok(())
    }

    fn geometrycollection_end(&mut self, idx: usize) -> geozero::error::Result<()> {
        let geometries = self.collections.pop().ok_or(GeozeroError::Geometry(
            "Unexpected geometry type".to_string(),
        ))?;

        self.finish_geometry(Geometry::GeometryCollection(GeometryCollection(geometries)))
    }
}

fn cmp_points(a: &Point, b: &Point) -> Ordering {
    let d = a.euclidean_distance(b);

    match d {
        x if x < 0.0_f64 => Ordering::Less,
        x if x == 0.0_f64 => Ordering::Equal,
        x if x > 0.0_f64 => Ordering::Greater,
        _ => unreachable!(),
    }
}

fn to_ord_coord(&coord: &Coord) -> Coord<OrderedFloat<f64>> {
    coord! {x: OrderedFloat(coord.x), y: OrderedFloat(coord.y)}
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, error::Error};

    use geo::{Coord, CoordsIter, Geometry, Point};
    use geozero::{geo_types::GeoWriter, geojson::read_geojson};
    use graph_rs::Graph;
    use ordered_float::OrderedFloat;

    use crate::input::{
        geo_zero::{ColumnValueClonable, GraphWriter},
        NodeValue,
    };

    #[test]
    fn line_string() {
        let geojson = r#"{
            "type": "LineString",
            "coordinates": [
                [1875038.447610231,-3269648.6879248763],[1874359.641504197,-3270196.812984864],[1874141.0428635243,-3270953.7840121365],[1874440.1778162003,-3271619.4315206874],[1876396.0598222911,-3274138.747656357],[1876442.0805243007,-3275052.60551469],[1874739.312657555,-3275457.333765534]
            ]
        }"#;
        let mut graph_writer = GraphWriter::new_from_filter(|_| true);
        assert!(read_geojson(geojson.as_bytes(), &mut graph_writer).is_ok());
        let graph = graph_writer.get_graph();

        assert_eq!(
            graph.neighbors(0).map(|x| x.target()).collect::<Vec<_>>(),
            vec![1]
        );
        if let coord = graph.node_value(3).unwrap() {
            assert_eq!(
                coord.get_coord().x_y(),
                (1874440.1778162003, -3271619.4315206874)
            );
        }
    }

    #[test]
    fn multi_polygon() {
        let geojson = r#"{
            "type": "MultiPolygon",
            "coordinates": [[[
                [73.020375,-40.919052],[70.247234,-41.331999],[173.958405,-40.926701],[174.247587,-41.349155],[174.248517,-41.770008],[173.876447,-42.233184],[173.22274,-42.970038],[172.711246,-43.372288],[173.080113,-43.853344],[172.308584,-43.865694],[171.452925,-44.242519],[171.185138,-44.897104],[170.616697,-45.908929],[169.831422,-46.355775],[169.332331,-46.641235],[168.411354,-46.619945],[167.763745,-46.290197],[166.676886,-46.219917],[166.509144,-45.852705],[167.046424,-45.110941],[168.303763,-44.123973],[168.949409,-43.935819],[169.667815,-43.555326],[170.52492,-43.031688],[171.12509,-42.512754],[171.569714,-41.767424],[171.948709,-41.514417],[172.097227,-40.956104],[172.79858,-40.493962],[173.020375,-40.919052],[73.020375,-40.919052]
            ]],[[
                [174.612009,-36.156397],[175.336616,-37.209098],[175.357596,-36.526194],[175.808887,-36.798942],[175.95849,-37.555382],[176.763195,-37.881253],[177.438813,-37.961248],[178.010354,-37.579825],[178.517094,-37.695373],[178.274731,-38.582813],[177.97046,-39.166343],[177.206993,-39.145776],[176.939981,-39.449736],[177.032946,-39.879943],[176.885824,-40.065978],[176.508017,-40.604808],[176.01244,-41.289624],[175.239567,-41.688308],[175.067898,-41.425895],[174.650973,-41.281821],[175.22763,-40.459236],[174.900157,-39.908933],[173.824047,-39.508854],[173.852262,-39.146602],[174.574802,-38.797683],[174.743474,-38.027808],[174.697017,-37.381129],[174.292028,-36.711092],[174.319004,-36.534824],[173.840997,-36.121981],[173.054171,-35.237125],[172.636005,-34.529107],[173.007042,-34.450662],[173.551298,-35.006183],[174.32939,-35.265496],[174.612009,-36.156397]
            ]]]
        }"#;
        let mut graph_writer = GraphWriter::new_from_filter(|_| true);
        assert!(read_geojson(geojson.as_bytes(), &mut graph_writer).is_ok());
        let graph = graph_writer.get_graph();

        assert_eq!(
            graph.neighbors(0).map(|x| x.target()).collect::<Vec<_>>(),
            vec![1, 29]
        );
    }

    #[test]
    fn way_filter() {
        let geojson = r#" {
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "geometry": {
                "type": "LineString",
                "coordinates": [
                [
                    13.3530166,
                    52.5365623
                ],
                [
                    13.3531553,
                    52.5364245
                ],
                [
                    13.3538338,
                    52.5364855
                ],
                [
                    13.3542415,
                    52.536498
                ],
                [
                    13.3546724,
                    52.5364904
                ],
                [
                    13.355102,
                    52.5364593
                ]
                ]
            },
            "properties": {
                "osm_id": 54111470,
                "osm_type": "ways_line",
                "tunnel": null,
                "surface": "paving_stones",
                "name": null,
                "width": null,
                "highway": "service",
                "oneway": null,
                "layer": null,
                "bridge": null,
                "smoothness": null
            }
        }]
        }"#;

        let filter =
            |p: &HashMap<String, ColumnValueClonable>| p.contains_key(&"highway".to_string());
        let mut graph_writer = GraphWriter::new_from_filter(filter);
        assert!(read_geojson(geojson.as_bytes(), &mut graph_writer).is_ok());
        let graph = graph_writer.get_graph();

        assert_eq!(
            graph.neighbors(0).map(|x| x.target()).collect::<Vec<_>>(),
            vec![1]
        );
    }
}
