//! Brand logos: Wikidata's "logo image" (P154) file, rendered by Wikimedia
//! Commons as a small raster thumbnail (SVG logos come back as PNG).

use serde_json::Value;
use url::Url;

/// Wikidata property "logo image".
pub const LOGO_PROPERTY: &str = "P154";
/// Wikidata qualifier "end time": set on logos no longer in use.
const END_TIME_QUALIFIER: &str = "P582";
/// Thumbnail width requested from Commons: sharp on a 3x screen at ~80 CSS px.
pub const THUMB_WIDTH: u32 = 256;
/// A 256 px PNG logo is a few tens of KB; matches the database CHECK.
pub const MAX_LOGO_BYTES: usize = 256 * 1024;
/// Commons file names are page titles: at most 255 bytes.
const MAX_FILE_NAME_BYTES: usize = 255;
/// Where Commons serves thumbnails from.
const MEDIA_ORIGIN: &str = "https://upload.wikimedia.org";

/// Query string for the item's logo claims (`wbgetclaims`).
pub fn claim_params(qid: &str) -> Vec<(&'static str, String)> {
    vec![
        ("action", "wbgetclaims".into()),
        ("entity", qid.into()),
        ("property", LOGO_PROPERTY.into()),
        ("props", "".into()),
        ("format", "json".into()),
    ]
}

/// The current logo's Commons file name: a preferred-rank claim first, else
/// the first normal-rank one without an end date. Deprecated claims never count.
pub fn parse_logo_file(body: &Value) -> Option<String> {
    let claims: Vec<&Value> = body["claims"][LOGO_PROPERTY]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|claim| claim["rank"].as_str() != Some("deprecated"))
        .collect();
    let is_current = |claim: &&&Value| claim["qualifiers"].get(END_TIME_QUALIFIER).is_none();
    let chosen = claims
        .iter()
        .find(|claim| claim["rank"].as_str() == Some("preferred"))
        .or_else(|| claims.iter().find(is_current))?;
    chosen["mainsnak"]["datavalue"]["value"]
        .as_str()
        .filter(|name| is_file_name(name))
        .map(str::to_owned)
}

/// A plausible Commons file name; `|` would let it name several files at once.
fn is_file_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= MAX_FILE_NAME_BYTES
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '|' | '#'))
}

/// Query string asking Commons for a raster thumbnail of `file`.
pub fn thumb_params(file: &str) -> Vec<(&'static str, String)> {
    vec![
        ("action", "query".into()),
        ("titles", format!("File:{file}")),
        ("prop", "imageinfo".into()),
        ("iiprop", "url".into()),
        ("iiurlwidth", THUMB_WIDTH.to_string()),
        ("format", "json".into()),
        ("formatversion", "2".into()),
    ]
}

/// The thumbnail URL, if it points at Commons' media host or at `commons_api`'s
/// own origin (a local test double). Anything else is not fetched.
pub fn parse_thumb_url(body: &Value, commons_api: &Url) -> Option<Url> {
    let raw = body["query"]["pages"][0]["imageinfo"][0]["thumburl"].as_str()?;
    let url = Url::parse(raw).ok()?;
    let origin = url.origin().ascii_serialization();
    (origin == MEDIA_ORIGIN || url.origin() == commons_api.origin()).then_some(url)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn claim(file: &str, rank: &str) -> Value {
        json!({ "rank": rank, "mainsnak": { "datavalue": { "value": file } } })
    }

    #[test]
    fn picks_the_preferred_logo() {
        let body = json!({ "claims": { "P154": [
            claim("Old.svg", "normal"),
            claim("New.svg", "preferred"),
        ]}});
        assert_eq!(parse_logo_file(&body).as_deref(), Some("New.svg"));
    }

    #[test]
    fn skips_deprecated_and_ended_logos() {
        let mut ended = claim("1990.svg", "normal");
        ended["qualifiers"] = json!({ "P582": [{}] });
        let body = json!({ "claims": { "P154": [
            claim("Wrong.svg", "deprecated"),
            ended,
            claim("Current.svg", "normal"),
        ]}});
        assert_eq!(parse_logo_file(&body).as_deref(), Some("Current.svg"));
    }

    #[test]
    fn no_logo_when_missing_or_malformed() {
        assert_eq!(parse_logo_file(&json!({ "claims": {} })), None);
        assert_eq!(parse_logo_file(&json!({ "error": {} })), None);
        for bad in ["", "  ", "A|B.svg", "A#b.svg", "a\nb.svg"] {
            let body = json!({ "claims": { "P154": [claim(bad, "normal")] } });
            assert_eq!(parse_logo_file(&body), None, "{bad:?}");
        }
        let long = "a".repeat(MAX_FILE_NAME_BYTES + 1);
        let body = json!({ "claims": { "P154": [claim(&long, "normal")] } });
        assert_eq!(parse_logo_file(&body), None);
    }

    #[test]
    fn accepts_thumbnails_only_from_commons_media() {
        let api = Url::parse("https://commons.wikimedia.org/w/api.php").unwrap();
        let body =
            |url: &str| json!({ "query": { "pages": [{ "imageinfo": [{ "thumburl": url }] }] } });
        let thumb =
            "https://upload.wikimedia.org/wikipedia/commons/thumb/a/ab/X.svg/256px-X.svg.png";
        assert_eq!(
            parse_thumb_url(&body(thumb), &api)
                .map(String::from)
                .as_deref(),
            Some(thumb)
        );
        for bad in [
            "http://upload.wikimedia.org/x.png",
            "https://evil.example/x.png",
            "https://upload.wikimedia.org.evil.example/x.png",
            "not a url",
        ] {
            assert_eq!(parse_thumb_url(&body(bad), &api), None, "{bad}");
        }
        assert_eq!(
            parse_thumb_url(
                &json!({ "query": { "pages": [{ "missing": true }] } }),
                &api
            ),
            None
        );
    }

    #[test]
    fn builds_requests() {
        assert!(claim_params("Q1").contains(&("property", "P154".into())));
        assert!(thumb_params("Logo.svg").contains(&("titles", "File:Logo.svg".into())));
    }
}
