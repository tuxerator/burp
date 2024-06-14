pub use geozero;
use log::info;
use ordered_float::OrderedFloat;

use std::{cmp::Ordering, collections::HashMap, io::Read, mem};

use geo::{
    coord,
    geometry::{Coord, Point},
    point, EuclideanDistance, GeodesicDistance, HaversineDistance, LineString,
};
use geozero::{
    error::GeozeroError, geojson::GeoJson, ColumnValue, FeatureProcessor, GeomProcessor,
    PropertyProcessor,
};

use crate::{graph::csr::DirectedCsrGraph, DirectedGraph, Graph};

use super::edgelist::EdgeList;

#[derive(Debug, Default)]
pub struct GraphWriter {
    node_map: HashMap<Coord<OrderedFloat<f64>>, usize>,
    nodes: Vec<Coord<f64>>,
    edges: Vec<(usize, usize, f64)>,
    line: Vec<(usize, usize, f64)>,
    coords: Option<Vec<Coord>>,
    index: usize,
    filter_features: bool,
    is_way: bool,
    is_sidewalk: bool,
}

pub fn read_geojson<R, P>(reader: R, processor: &mut P) -> Result<(), GeozeroError>
where
    R: Read,
    P: FeatureProcessor,
{
    geozero::geojson::read_geojson(reader, processor)
}

impl GraphWriter {
    pub fn new_from(graph_writer: GraphWriter) -> GraphWriter {
        graph_writer
    }

    pub fn filter_features(&mut self) {
        self.filter_features = true;
    }

    pub fn get_graph(&mut self) -> DirectedCsrGraph<f64, Coord> {
        let edge_list = EdgeList::new(mem::take(&mut self.edges));

        let graph = DirectedCsrGraph::from(edge_list);

        DirectedCsrGraph::new(
            mem::take(&mut self.nodes).into_boxed_slice(),
            graph.csr_out,
            graph.csr_inc,
        )
    }
}

impl GeomProcessor for GraphWriter {
    fn xy(&mut self, x: f64, y: f64, idx: usize) -> geozero::error::Result<()> {
        if (!self.is_way || self.is_sidewalk) && self.filter_features {
            let _ = self.coords.take();
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
            self.nodes.push(coord);
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
        if (!self.is_way || self.is_sidewalk) && self.filter_features {
            let _ = self.coords.take();
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

            if self.filter_features {
                self.line.push((*node_a, *node_b, d));
            } else {
                self.edges.push((*node_a, *node_b, d))
            }

            coord_a = coord_b;
        }

        Ok(())
    }
}

impl FeatureProcessor for GraphWriter {
    fn feature_begin(&mut self, idx: u64) -> geozero::error::Result<()> {
        self.is_way = false;
        self.is_sidewalk = false;
        self.line = Vec::new();

        Ok(())
    }
    fn feature_end(&mut self, idx: u64) -> geozero::error::Result<()> {
        if self.is_way && !self.is_sidewalk {
            self.edges.append(&mut self.line);
        }
        Ok(())
    }
}

impl PropertyProcessor for GraphWriter {
    fn property(&mut self, i: usize, n: &str, v: &ColumnValue) -> geozero::error::Result<bool> {
        match n {
            "footway" => {
                if let ColumnValue::String(s) = v {
                    if s != &"null" {
                        self.is_sidewalk = true;
                    }
                }
            }
            "highway" => {
                self.is_way = true;
            }
            _ => (),
        };
        Ok(false) // don't abort
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
    use std::error::Error;

    use geo::{Coord, CoordsIter, Geometry, Point};
    use geozero::{geo_types::GeoWriter, geojson::read_geojson};
    use ordered_float::OrderedFloat;

    use crate::{input::geo_zero::GraphWriter, DirectedGraph, Graph};

    #[test]
    fn line_string() {
        let geojson = r#"{
            "type": "LineString",
            "coordinates": [
                [1875038.447610231,-3269648.6879248763],[1874359.641504197,-3270196.812984864],[1874141.0428635243,-3270953.7840121365],[1874440.1778162003,-3271619.4315206874],[1876396.0598222911,-3274138.747656357],[1876442.0805243007,-3275052.60551469],[1874739.312657555,-3275457.333765534]
            ]
        }"#;
        let mut graph_writer = GraphWriter::default();
        assert!(read_geojson(geojson.as_bytes(), &mut graph_writer).is_ok());
        let graph = graph_writer.get_graph();

        assert_eq!(
            graph.neighbors(0).map(|x| x.target()).collect::<Vec<_>>(),
            vec![1]
        );
        assert_eq!(
            graph.node_value(3).unwrap().x_y(),
            (1874440.1778162003, -3271619.4315206874)
        );
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
        let mut graph_writer = GraphWriter::default();
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

        let mut graph_writer = GraphWriter::default();
        graph_writer.filter_features();
        assert!(read_geojson(geojson.as_bytes(), &mut graph_writer).is_ok());
        let graph = graph_writer.get_graph();

        assert_eq!(
            graph.neighbors(0).map(|x| x.target()).collect::<Vec<_>>(),
            vec![1]
        );
    }
}
