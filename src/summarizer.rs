use crate::models::SectionDiff;

pub fn status_for(change_score: f64) -> String {
    if change_score < 0.20 {
        "unchanged"
    } else if change_score < 0.55 {
        "moderate"
    } else {
        "changed"
    }
    .to_string()
}

pub fn section_summary(diff: &SectionDiff) -> String {
    if diff.change_score < 0.20 {
        return "This section appears largely unchanged.".to_string();
    }

    if diff.paragraph_overlap < 0.35 && diff.similarity < 0.45 {
        return "This section appears to have significant wording and content changes.".to_string();
    }

    if diff.length_delta >= 0.35 && diff.change_score >= 0.55 {
        return "This section appears materially expanded or reduced compared with the previous filing."
            .to_string();
    }

    if diff.change_score >= 0.55 {
        return "This section appears to have significant wording and content changes.".to_string();
    }

    "This section has moderate wording and content updates.".to_string()
}

pub fn overall_summary(diffs: &[SectionDiff]) -> String {
    if diffs.is_empty() {
        return "No comparable narrative sections were found.".to_string();
    }

    let changed: Vec<&str> = diffs
        .iter()
        .filter(|diff| diff.change_score >= 0.20)
        .take(2)
        .map(|diff| diff.name.as_str())
        .collect();

    match changed.as_slice() {
        [] => {
            "The latest filing appears largely unchanged across the comparable narrative sections."
                .to_string()
        }
        [one] => format!("The latest filing mainly updates {one}."),
        [one, two, ..] => format!("The latest filing mainly updates {one} and {two}."),
    }
}
