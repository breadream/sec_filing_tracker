use crate::models::{FinancialTrend, HealthDriver, OverallHealth, SectionDiff};

pub fn overall_health(
    financial_trends: &[FinancialTrend],
    section_diffs: &[SectionDiff],
) -> OverallHealth {
    let mut score = 0.5;
    let mut drivers = Vec::new();

    for trend in financial_trends {
        let weight = match trend.name.as_str() {
            "Revenue" | "Gross profit" | "Operating income" | "Net income" => 0.09,
            "Operating cash flow" => 0.08,
            "Cash and equivalents" => 0.06,
            "Debt" => 0.10,
            _ => 0.05,
        };

        let mut impact = "neutral";
        match trend.status.as_str() {
            "improving" => {
                score += weight;
                impact = "positive";
            }
            "weakening" => {
                score -= weight;
                impact = "negative";
            }
            _ => {}
        }

        if trend.status != "unknown" {
            drivers.push(HealthDriver {
                label: trend.name.clone(),
                impact: impact.to_string(),
                summary: format!(
                    "{} moved the score {} by {:.0} points.",
                    trend.name,
                    if impact == "positive" {
                        "up"
                    } else if impact == "negative" {
                        "down"
                    } else {
                        "only slightly"
                    },
                    weight * 100.0
                ),
                evidence: Some(trend.summary.clone()),
            });
        }
    }

    if !section_diffs.is_empty() {
        let avg_change = section_diffs
            .iter()
            .map(|diff| diff.change_score)
            .sum::<f64>()
            / section_diffs.len() as f64;
        score -= avg_change * 0.20;
        drivers.push(HealthDriver {
            label: "Narrative disclosure changes".to_string(),
            impact: if avg_change >= 0.20 {
                "negative".to_string()
            } else {
                "neutral".to_string()
            },
            summary: format!(
                "Average section-change intensity was {:.0}%, applying a {:.0}-point caution penalty.",
                avg_change * 100.0,
                avg_change * 20.0
            ),
            evidence: Some(
                "This reflects wording similarity, paragraph overlap, and length movement across comparable sections."
                    .to_string(),
            ),
        });
    }

    score = score.clamp(0.0, 1.0);

    let status = if score >= 0.72 {
        "strong"
    } else if score >= 0.45 {
        "watch"
    } else {
        "stressed"
    }
    .to_string();

    OverallHealth {
        status,
        score: (score * 100.0).round() / 100.0,
        summary: build_summary(financial_trends, section_diffs, score),
        methodology:
            "Starts at 50, adjusts for operating trends, then applies a narrative disclosure caution penalty. Optional AI can replace the summary with filing-specific evidence when OPENAI_API_KEY is set."
                .to_string(),
        drivers,
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
