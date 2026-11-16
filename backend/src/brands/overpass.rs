//! Store locations from OpenStreetMap via the Overpass API.

use serde_json::Value;

/// Server-side Overpass timeout; a country-wide brand query takes a few seconds.
pub const QUERY_TIMEOUT_SECS: u32 = 90;
/// Even the largest Italian chains have a few thousand shops.
pub const MAX_LOCATIONS: usize = 10_000;
/// Overpass memory budget for the query (bytes); far above any real brand.
const QUERY_MAX_SIZE: u32 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct StoreLocation {
    pub osm_type: &'static str,
    pub osm_id: i64,
    pub lat: f64,
    pub lon: f64,
    pub name: Option<String>,
}

/// Every node, way and relation of the brand inside the country. Ways and
/// relations (shops drawn as buildings) are reduced to their centre.
///
/// `qid` and `region` must be validated by the caller (`Q123…`, `IT`).
pub fn query(qid: &str, region: &str) -> String {
    format!(
        "[out:json][timeout:{QUERY_TIMEOUT_SECS}][maxsize:{QUERY_MAX_SIZE}];\n\
         area[\"ISO3166-1\"=\"{region}\"][admin_level=2]->.region;\n\
         nwr[\"brand:wikidata\"=\"{qid}\"](area.region);\n\
         out center tags;"
    )
}

pub fn parse(body: &Value) -> Vec<StoreLocation> {
    body["elements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(parse_element)
        .take(MAX_LOCATIONS)
        .collect()
}

fn parse_element(element: &Value) -> Option<StoreLocation> {
    let osm_type = match element["type"].as_str()? {
        "node" => "node",
        "way" => "way",
        "relation" => "relation",
        _ => return None,
    };
    let point = if osm_type == "node" {
        element
    } else {
        &element["center"]
    };
    let lat = point["lat"]
        .as_f64()
        .filter(|lat| (-90.0..=90.0).contains(lat))?;
    let lon = point["lon"]
        .as_f64()
        .filter(|lon| (-180.0..=180.0).contains(lon))?;
    Some(StoreLocation {
        osm_type,
        osm_id: element["id"].as_i64()?,
        lat,
        lon,
        name: element["tags"]["name"]
            .as_str()
            .map(|name| name.chars().take(128).collect()),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn query_targets_the_brand_inside_the_country() {
        let q = query("Q1059636", "IT");
        assert!(q.contains(r#"area["ISO3166-1"="IT"][admin_level=2]"#));
        assert!(q.contains(r#"nwr["brand:wikidata"="Q1059636"](area.region)"#));
        assert!(q.contains("out center tags;"));
    }

    #[test]
    fn parses_nodes_ways_and_relations() {
        let body = json!({ "elements": [
            { "type": "node", "id": 1, "lat": 45.46, "lon": 9.19, "tags": { "name": "Esselunga Viale Piave" } },
            { "type": "way", "id": 2, "center": { "lat": 45.5, "lon": 9.2 }, "tags": {} },
            { "type": "relation", "id": 3, "center": { "lat": 45.6, "lon": 9.3 } },
        ]});

        let locations = parse(&body);

        assert_eq!(locations.len(), 3);
        assert_eq!(locations[0].name.as_deref(), Some("Esselunga Viale Piave"));
        assert_eq!((locations[1].osm_type, locations[1].lat), ("way", 45.5));
        assert_eq!(locations[2].osm_id, 3);
    }

    #[test]
    fn skips_elements_without_a_usable_position() {
        let body = json!({ "elements": [
            { "type": "way", "id": 2, "tags": {} },
            { "type": "node", "id": 3, "lat": 95.0, "lon": 9.0 },
            { "type": "area", "id": 4, "lat": 1.0, "lon": 1.0 },
            { "type": "node", "lat": 1.0, "lon": 1.0 },
        ]});
        assert!(parse(&body).is_empty());
        assert!(parse(&json!({ "remark": "runtime error" })).is_empty());
    }
}
