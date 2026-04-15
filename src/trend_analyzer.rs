use crate::models::{FinancialTrend, OverallHealth, SectionDiff};

pub fn overall_health(
    financial_trends: &[FinancialTrend],
    section_diffs: &[SectionDiff],
) -> OverallHealth {
    let mut score = 0.5;

    for trend in financial_trends {
        let weight = match trend.name.as_str() {
            "Revenue" | "Gross Profit" | "Operating Income" | "Net Income" => 0.09,
            "Operating Cash Flow" => 0.08,
            "Cash and Cash Equivalents" => 0.06,
            "Debt" => 0.10,
            _ => 0.05,
        };

        match trend.status.as_str() {
            "improving" => {
                if trend.name == "Debt" {
                    score -= weight;
                } else {
                    score += weight;
                }
            }
            "weakening" => {
                if trend.name == "Debt" {
                    score += weight;
                } else {
                    score -= weight;
                }
            }
            _ => {}
        }
    }

    if !section_diffs.is_empty() {
        let avg_change = section_diffs
            .iter()
            .map(|diff| diff.change_score)
            .sum::<f64>()
            / section_diffs.len() as f64;
        score -= avg_change * 0.20;
    }

    score = score.clamp(0.0, 1.0);

    let status = if score >= 0.72 {
        "healthy"
    } else if score >= 0.45 {
        "watch"
    } else {
        "weak"
    }
    .to_string();

    OverallHealth {
        status,
        score: (score * 100.0).round() / 100.0,
        summary: build_summary(financial_trends, section_diffs, score),
    }
}

fn build_summary(
    financial_trends: &[FinancialTrend],
    section_diffs: &[SectionDiff],
    score: f64,
) -> String {
    let mut weak_points = financial_trends
        .iter()
        .filter(|trend| trend.status == "weakening")
        .map(|trend| trend.name.as_str())
        .take(2)
        .collect::<Vec<_>>();

    if weak_points.is_empty() {
        weak_points = section_diffs
            .iter()
            .filter(|diff| diff.change_score >= 0.35)
            .map(|diff| diff.name.as_str())
            .take(2)
            .collect();
    }

    if score >= 0.72 {
        if weak_points.is_empty() {
            "Core operating trends look healthy and the filing text is broadly steady.".to_string()
        } else {
            format!(
                "Core operating trends look healthy, with changes concentrated in {}.",
                join_names(&weak_points)
            )
        }
    } else if score >= 0.45 {
        if weak_points.is_empty() {
            "The latest filing shows mixed operating trends and moderate disclosure changes."
                .to_string()
        } else {
            format!(
                "The company shows mixed operating trends, with the most notable pressure in {}.",
                join_names(&weak_points)
            )
        }
    } else if weak_points.is_empty() {
        "The latest filing shows broad weakness across the trend and narrative signals.".to_string()
    } else {
        format!(
            "The latest filing shows broad weakness, especially in {}.",
            join_names(&weak_points)
        )
    }
}

fn join_names(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_string(),
        [one, two, ..] => format!("{one} and {two}"),
    }
}
