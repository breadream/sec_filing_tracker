use std::{cmp::Ordering, collections::HashMap};

use serde::{Deserialize, Serialize, de::Error as _};

#[derive(Debug, Clone, Deserialize)]
pub struct CompanyFacts {
    pub facts: CompanyFactsNamespaces,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompanyFactsNamespaces {
    #[serde(flatten)]
    pub namespaces: HashMap<String, HashMap<String, ConceptFacts>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConceptFacts {
    pub label: Option<String>,
    pub units: HashMap<String, Vec<RawFactRecord>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawFactRecord {
    #[serde(rename = "accn")]
    pub accession_number: Option<String>,
    #[serde(deserialize_with = "deserialize_number")]
    pub val: f64,
    pub fy: Option<i32>,
    pub fp: Option<String>,
    pub form: Option<String>,
    pub filed: Option<String>,
    pub frame: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FactPoint {
    pub namespace: String,
    pub concept: String,
    pub label: Option<String>,
    pub unit: String,
    pub value: f64,
    pub accession_number: Option<String>,
    pub fy: Option<i32>,
    pub fp: Option<String>,
    pub form: String,
    pub filed: Option<String>,
    pub frame: Option<String>,
    pub period_start: Option<String>,
    pub period_end: String,
    pub duration_days: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecentFactSeries {
    pub concept: String,
    pub label: Option<String>,
    pub unit: String,
    pub facts: Vec<FactPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FactPair {
    pub latest: FactPoint,
    pub previous: FactPoint,
}

pub fn company_facts_url(cik: u64) -> String {
    format!("https://data.sec.gov/api/xbrl/companyfacts/CIK{cik:010}.json")
}

pub fn select_recent_usd_facts(
    company_facts: &CompanyFacts,
    concept_aliases: &[&str],
    form: &str,
) -> Option<RecentFactSeries> {
    for alias in concept_aliases {
        if let Some(series) = select_concept_usd_facts(company_facts, alias, form) {
            return Some(series);
        }
    }

    None
}

pub fn latest_previous_by_period_end(facts: &[FactPoint]) -> Option<FactPair> {
    let mut ordered = facts.to_vec();
    ordered.sort_by(compare_recent_fact_points);
    ordered.dedup_by(|left, right| left.period_end == right.period_end);

    let latest = ordered.first()?.clone();
    let previous = ordered.get(1)?.clone();

    Some(FactPair { latest, previous })
}

fn select_concept_usd_facts(
    company_facts: &CompanyFacts,
    concept: &str,
    form: &str,
) -> Option<RecentFactSeries> {
    let mut namespace_names: Vec<&str> = company_facts
        .facts
        .namespaces
        .keys()
        .map(|name| name.as_str())
        .collect();
    namespace_names.sort();

    if let Some(index) = namespace_names
        .iter()
        .position(|namespace| *namespace == "us-gaap")
    {
        let namespace = namespace_names.remove(index);
        namespace_names.insert(0, namespace);
    }

    for namespace in namespace_names {
        let concept_map = company_facts.facts.namespaces.get(namespace)?;
        let concept_facts = concept_map.get(concept)?;
        let raw_facts = concept_facts.units.get("USD")?;

        let mut facts: Vec<FactPoint> = raw_facts
            .iter()
            .filter(|fact| fact.form.as_deref() == Some(form))
            .map(|fact| {
                raw_fact_to_point(
                    namespace,
                    concept,
                    concept_facts.label.as_deref(),
                    "USD",
                    fact,
                )
            })
            .collect();

        if facts.is_empty() {
            continue;
        }

        facts.sort_by(compare_recent_fact_points);

        return Some(RecentFactSeries {
            concept: concept.to_string(),
            label: concept_facts.label.clone(),
            unit: "USD".to_string(),
            facts,
        });
    }

    None
}

fn raw_fact_to_point(
    namespace: &str,
    concept: &str,
    label: Option<&str>,
    unit: &str,
    fact: &RawFactRecord,
) -> FactPoint {
    FactPoint {
        namespace: namespace.to_string(),
        concept: concept.to_string(),
        label: label.map(|value| value.to_string()),
        unit: unit.to_string(),
        value: fact.val,
        accession_number: fact.accession_number.clone(),
        fy: fact.fy,
        fp: fact.fp.clone(),
        form: fact.form.clone().unwrap_or_default(),
        filed: fact.filed.clone(),
        frame: fact.frame.clone(),
        period_start: fact.start.clone(),
        period_end: fact.end.clone().unwrap_or_default(),
        duration_days: period_duration_days(fact.start.as_deref(), fact.end.as_deref()),
    }
}

fn compare_recent_fact_points(left: &FactPoint, right: &FactPoint) -> Ordering {
    right
        .period_end
        .cmp(&left.period_end)
        .then_with(|| right.filed.cmp(&left.filed))
        .then_with(|| left.duration_days.cmp(&right.duration_days))
        .then_with(|| right.accession_number.cmp(&left.accession_number))
}

fn period_duration_days(start: Option<&str>, end: Option<&str>) -> Option<i64> {
    let start = parse_ymd(start?)?;
    let end = parse_ymd(end?)?;
    Some(days_from_civil(end.0, end.1, end.2) - days_from_civil(start.0, start.1, start.2))
}

fn parse_ymd(value: &str) -> Option<(i64, i64, i64)> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<i64>().ok()?;
    let day = parts.next()?.parse::<i64>().ok()?;
    Some((year, month, day))
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn deserialize_number<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(number) => number
            .as_f64()
            .ok_or_else(|| D::Error::custom("numeric value could not be represented as f64")),
        serde_json::Value::String(text) => text
            .parse::<f64>()
            .map_err(|err| D::Error::custom(format!("invalid numeric string: {err}"))),
        other => Err(D::Error::custom(format!(
            "unexpected numeric value: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
    {
      "cik": 320193,
      "entityName": "Apple Inc.",
      "facts": {
        "us-gaap": {
          "Revenues": {
            "label": "Revenues",
            "units": {
              "USD": [
                {
                  "val": 150.0,
                  "form": "10-Q",
                  "filed": "2025-08-01",
                  "start": "2025-04-01",
                  "end": "2025-06-30"
                },
                {
                  "val": 300.0,
                  "form": "10-Q",
                  "filed": "2025-08-01",
                  "start": "2025-01-01",
                  "end": "2025-06-30"
                },
                {
                  "val": 120.0,
                  "form": "10-Q",
                  "filed": "2025-05-01",
                  "start": "2025-01-01",
                  "end": "2025-03-31"
                }
              ]
            }
          }
        }
      }
    }
    "#;

    #[test]
    fn parses_company_facts_and_selects_recent_usd_series() {
        let company_facts: CompanyFacts = serde_json::from_str(FIXTURE).expect("fixture parses");
        let series =
            select_recent_usd_facts(&company_facts, &["Revenues"], "10-Q").expect("series");

        assert_eq!(series.concept, "Revenues");
        assert_eq!(series.unit, "USD");
        assert_eq!(series.facts.len(), 3);
        assert_eq!(series.facts[0].period_end, "2025-06-30");
        assert!(series.facts.iter().any(|fact| fact.value == 150.0));
        assert!(series.facts.iter().any(|fact| fact.value == 300.0));
        assert_eq!(series.facts[0].duration_days, Some(90));
    }

    #[test]
    fn latest_previous_uses_period_end_order() {
        let company_facts: CompanyFacts = serde_json::from_str(FIXTURE).expect("fixture parses");
        let series =
            select_recent_usd_facts(&company_facts, &["Revenues"], "10-Q").expect("series");
        let pair = latest_previous_by_period_end(&series.facts).expect("pair");

        assert_eq!(pair.latest.period_end, "2025-06-30");
        assert_eq!(pair.previous.period_end, "2025-03-31");
    }
}
