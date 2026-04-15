use std::collections::HashSet;

use regex::Regex;

use crate::models::{ParsedSection, SectionDiff};

pub fn compare_sections(
    latest_sections: &[ParsedSection],
    previous_sections: &[ParsedSection],
) -> Vec<SectionDiff> {
    let mut diffs = Vec::new();

    for latest in latest_sections {
        let Some(previous) = previous_sections
            .iter()
            .find(|section| section.name == latest.name)
        else {
            continue;
        };

        diffs.push(compare_section(latest, previous));
    }

    diffs.sort_by(|a, b| {
        b.change_score
            .partial_cmp(&a.change_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    diffs
}

fn compare_section(latest: &ParsedSection, previous: &ParsedSection) -> SectionDiff {
    let latest_text = normalize_for_compare(&latest.text);
    let previous_text = normalize_for_compare(&previous.text);
    let similarity =
        jaccard_similarity(&word_shingles(&latest_text), &word_shingles(&previous_text));
    let paragraph_overlap = paragraph_overlap(&latest_text, &previous_text);
    let length_delta = length_delta(&latest_text, &previous_text);

    let change_score =
        ((1.0 - similarity) * 0.55 + (1.0 - paragraph_overlap) * 0.25 + length_delta * 0.20)
            .clamp(0.0, 1.0);

    SectionDiff {
        name: latest.name.clone(),
        change_score: round_score(change_score),
        length_delta: round_score(length_delta),
        paragraph_overlap: round_score(paragraph_overlap),
        similarity: round_score(similarity),
    }
}

fn normalize_for_compare(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let punctuation = Regex::new(r"[^a-z0-9.\s]").expect("punctuation regex should compile");
    let whitespace = Regex::new(r"\s+").expect("whitespace regex should compile");
    whitespace
        .replace_all(&punctuation.replace_all(&lower, " "), " ")
        .trim()
        .to_string()
}

fn word_shingles(text: &str) -> HashSet<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 5 {
        return words.into_iter().map(ToOwned::to_owned).collect();
    }

    words
        .windows(5)
        .map(|window| window.join(" "))
        .collect::<HashSet<_>>()
}

fn jaccard_similarity(left: &HashSet<String>, right: &HashSet<String>) -> f64 {
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }

    let intersection = left.intersection(right).count() as f64;
    let union = left.union(right).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn paragraph_overlap(latest: &str, previous: &str) -> f64 {
    let latest_paragraphs = paragraph_set(latest);
    let previous_paragraphs = paragraph_set(previous);

    if latest_paragraphs.is_empty() && previous_paragraphs.is_empty() {
        return 1.0;
    }

    jaccard_similarity(&latest_paragraphs, &previous_paragraphs)
}

fn paragraph_set(text: &str) -> HashSet<String> {
    text.split('.')
        .map(str::trim)
        .filter(|paragraph| paragraph.split_whitespace().count() >= 8)
        .map(ToOwned::to_owned)
        .collect()
}

fn length_delta(latest: &str, previous: &str) -> f64 {
    let latest_words = latest.split_whitespace().count() as f64;
    let previous_words = previous.split_whitespace().count() as f64;
    let max_words = latest_words.max(previous_words);

    if max_words == 0.0 {
        0.0
    } else {
        ((latest_words - previous_words).abs() / max_words).clamp(0.0, 1.0)
    }
}

fn round_score(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_sections_have_low_change() {
        let latest = ParsedSection {
            name: "Risk Factors".to_string(),
            text: "Risk factors remain substantially similar for this reporting period.".repeat(20),
        };
        let previous = latest.clone();

        let diff = compare_section(&latest, &previous);

        assert!(diff.change_score <= 0.05);
    }
}
