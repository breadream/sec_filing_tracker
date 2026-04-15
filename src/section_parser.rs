use std::collections::HashSet;

use regex::Regex;
use scraper::{Html, Selector};

use crate::models::ParsedSection;

#[derive(Debug, Clone)]
struct HeadingMatch {
    index: usize,
    name: String,
}

pub fn extract_sections(html: &str, form: &str) -> Vec<ParsedSection> {
    let text = html_to_text(html);
    extract_sections_from_text(&text, form)
}

fn html_to_text(html: &str) -> String {
    let document = Html::parse_document(html);
    let selector = Selector::parse("body").expect("body selector should parse");
    let raw = document
        .select(&selector)
        .next()
        .map(|body| body.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| {
            Html::parse_fragment(html)
                .root_element()
                .text()
                .collect::<Vec<_>>()
                .join(" ")
        });

    normalize_text(&raw)
}

fn extract_sections_from_text(text: &str, form: &str) -> Vec<ParsedSection> {
    let definitions = section_definitions(form);
    let heading_regex = Regex::new(
        r"(?ix)
        \b
        (?:part\s+[ivx]+\s*[-:.]?\s*)?
        item\s+
        (?P<item>1a|1|3|2|7)
        \.?
        \s+
        (?P<title>
            risk\s+factors|
            business|
            legal\s+proceedings|
            management['’]s\s+discussion\s+and\s+analysis(?:\s+of\s+financial\s+condition\s+and\s+results\s+of\s+operations)?
        )
        \b",
    )
    .expect("section heading regex should compile");

    let allowed_names: HashSet<&str> = definitions.iter().map(|(name, _)| *name).collect();
    let mut headings = Vec::new();

    for capture in heading_regex.captures_iter(text) {
        let Some(full_match) = capture.get(0) else {
            continue;
        };
        let item = capture
            .name("item")
            .map(|matched| matched.as_str().to_ascii_lowercase())
            .unwrap_or_default();
        let title = capture
            .name("title")
            .map(|matched| canonical_title(matched.as_str()))
            .unwrap_or_default();

        if let Some(name) = definitions
            .iter()
            .find(|(_, expected)| {
                expected
                    .iter()
                    .any(|candidate| candidate == &item || candidate == &title)
            })
            .map(|(name, _)| *name)
        {
            if allowed_names.contains(name) {
                headings.push(HeadingMatch {
                    index: full_match.start(),
                    name: name.to_string(),
                });
            }
        }
    }

    headings.sort_by_key(|heading| heading.index);
    headings.dedup_by(|a, b| a.name == b.name && a.index.abs_diff(b.index) < 500);

    let mut sections = Vec::new();
    for index in 0..headings.len() {
        let heading = &headings[index];
        if sections
            .iter()
            .any(|section: &ParsedSection| section.name == heading.name)
        {
            continue;
        }

        let next_index = headings
            .iter()
            .skip(index + 1)
            .find(|candidate| candidate.index > heading.index)
            .map(|candidate| candidate.index)
            .unwrap_or(text.len());

        if next_index <= heading.index {
            continue;
        }

        let section_text = normalize_text(&text[heading.index..next_index]);
        if section_text.split_whitespace().count() >= 50 {
            sections.push(ParsedSection {
                name: heading.name.clone(),
                text: section_text,
            });
        }
    }

    sections
}

fn section_definitions(form: &str) -> Vec<(&'static str, Vec<String>)> {
    match form {
        "10-K" => vec![
            ("Business", vec!["1".into(), "business".into()]),
            ("Risk Factors", vec!["1a".into(), "risk factors".into()]),
            (
                "Legal Proceedings",
                vec!["3".into(), "legal proceedings".into()],
            ),
            (
                "Management's Discussion and Analysis",
                vec![
                    "7".into(),
                    "management's discussion and analysis".into(),
                    "management’s discussion and analysis".into(),
                ],
            ),
        ],
        _ => vec![
            ("Risk Factors", vec!["1a".into(), "risk factors".into()]),
            (
                "Legal Proceedings",
                vec!["1".into(), "legal proceedings".into()],
            ),
            (
                "Management's Discussion and Analysis",
                vec![
                    "2".into(),
                    "management's discussion and analysis".into(),
                    "management’s discussion and analysis".into(),
                ],
            ),
        ],
    }
}

fn canonical_title(title: &str) -> String {
    let lower = title.to_ascii_lowercase();
    let lower = lower.replace('’', "'");
    if lower.starts_with("management's discussion and analysis") {
        "management's discussion and analysis".to_string()
    } else {
        lower
    }
}

fn normalize_text(input: &str) -> String {
    let whitespace = Regex::new(r"\s+").expect("whitespace regex should compile");
    whitespace.replace_all(input, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_10q_sections() {
        let text = "<body>Item 1. Legal Proceedings ".to_string()
            + &"legal text ".repeat(60)
            + " Item 1A. Risk Factors "
            + &"risk text ".repeat(60)
            + " Item 2. Management's Discussion and Analysis "
            + &"mda text ".repeat(60);

        let sections = extract_sections(&text, "10-Q");

        assert!(
            sections
                .iter()
                .any(|section| section.name == "Legal Proceedings")
        );
        assert!(
            sections
                .iter()
                .any(|section| section.name == "Risk Factors")
        );
        assert!(
            sections
                .iter()
                .any(|section| section.name == "Management's Discussion and Analysis")
        );
    }
}
