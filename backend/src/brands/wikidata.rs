//! Brand lookup on Wikidata (`wbsearchentities`, `wbgetentities`).
//!
//! OpenStreetMap shops carry `brand:wikidata=Q…`, so a Wikidata id is what
//! links a card to the shops of its chain.

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

/// Longest Wikidata item id we accept (`Q` + up to 12 digits).
const MAX_QID_LEN: usize = 13;
pub const SEARCH_LIMIT: u32 = 8;

/// A search hit, not yet linked to any card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrandCandidate {
    pub wikidata_id: String,
    pub name: String,
    pub description: Option<String>,
}

/// `Q` followed by digits, no leading zero.
pub fn is_qid(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('Q') else {
        return false;
    };
    value.len() <= MAX_QID_LEN
        && !digits.is_empty()
        && !digits.starts_with('0')
        && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Query string for a `wbsearchentities` request.
pub fn search_params(query: &str, language: &str) -> Vec<(&'static str, String)> {
    vec![
        ("action", "wbsearchentities".into()),
        ("search", query.into()),
        ("language", language.into()),
        ("uselang", language.into()),
        ("type", "item".into()),
        ("limit", SEARCH_LIMIT.to_string()),
        ("format", "json".into()),
    ]
}

/// Query string for fetching an item's labels.
pub fn label_params(qid: &str, language: &str) -> Vec<(&'static str, String)> {
    vec![
        ("action", "wbgetentities".into()),
        ("ids", qid.into()),
        ("props", "labels".into()),
        ("languages", format!("{language}|en")),
        ("format", "json".into()),
    ]
}

pub fn parse_search(body: &Value) -> Vec<BrandCandidate> {
    body["search"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|hit| {
            let wikidata_id = hit["id"].as_str().filter(|id| is_qid(id))?;
            let name = hit["label"]
                .as_str()
                .filter(|label| !label.trim().is_empty())?;
            Some(BrandCandidate {
                wikidata_id: wikidata_id.to_owned(),
                name: truncate(name),
                description: hit["description"].as_str().map(truncate),
            })
        })
        .collect()
}

/// The item's label in `language`, falling back to English.
pub fn parse_label(body: &Value, qid: &str, language: &str) -> Option<String> {
    let labels = &body["entities"][qid]["labels"];
    [language, "en"]
        .iter()
        .find_map(|lang| labels[lang]["value"].as_str())
        .filter(|label| !label.trim().is_empty())
        .map(truncate)
}

/// Brand names are stored with at most 128 characters.
fn truncate(text: &str) -> String {
    text.trim().chars().take(128).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn recognises_wikidata_item_ids() {
        for valid in ["Q1", "Q1059636", "Q123456789012"] {
            assert!(is_qid(valid), "{valid}");
        }
        for invalid in [
            "",
            "Q",
            "Q0",
            "Q01",
            "P31",
            "q42",
            "Q42a",
            "Q1234567890123",
            "Q-1",
        ] {
            assert!(!is_qid(invalid), "{invalid}");
        }
    }

    #[test]
    fn parses_search_hits_and_skips_malformed_ones() {
        let body = json!({ "search": [
            { "id": "Q1059636", "label": "Esselunga", "description": "Italian supermarket chain" },
            { "id": "Q1129493", "label": "Coop" },
            { "id": "L123", "label": "lexeme" },
            { "id": "Q5", "label": "  " },
        ]});

        let hits = parse_search(&body);

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].name, "Esselunga");
        assert_eq!(
            hits[0].description.as_deref(),
            Some("Italian supermarket chain")
        );
        assert_eq!(hits[1].description, None);
        assert!(parse_search(&json!({ "error": {} })).is_empty());
    }

    #[test]
    fn picks_the_label_in_the_users_language_or_english() {
        let body = json!({ "entities": { "Q1": { "labels": {
            "it": { "value": "Conad" }, "en": { "value": "Conad (en)" }
        }}}});
        assert_eq!(parse_label(&body, "Q1", "it").as_deref(), Some("Conad"));
        assert_eq!(
            parse_label(&body, "Q1", "de").as_deref(),
            Some("Conad (en)")
        );
        assert_eq!(parse_label(&body, "Q2", "it"), None);
    }

    #[test]
    fn builds_requests() {
        let search = search_params("ess", "it");
        assert!(search.contains(&("action", "wbsearchentities".into())));
        assert!(search.contains(&("search", "ess".into())));
        assert!(label_params("Q1", "it").contains(&("languages", "it|en".into())));
    }
}
