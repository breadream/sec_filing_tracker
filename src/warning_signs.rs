use crate::{
    models::{FinancialTrend, NarrativeNote, SectionDiff, WarningSign},
    summarizer,
};

pub fn warning_signs(
    financial_trends: &[FinancialTrend],
    section_diffs: &[SectionDiff],
) -> Vec<WarningSign> {
    let mut signs = Vec::new();

    if let Some(cash) = financial_trends
        .iter()
        .find(|trend| trend.name == "Cash and Cash Equivalents")
    {
        if cash.status == "weakening" {
            signs.push(WarningSign {
                kind: "shrinking_cash".to_string(),
                severity: severity_for_score(cash.change_percent.unwrap_or(0.35).abs()),
                summary: "Cash and cash equivalents are moving lower versus the prior quarter."
                    .to_string(),
            });
        }
    }

    if let Some(net_income) = financial_trends
        .iter()
        .find(|trend| trend.name == "Net Income")
    {
        if net_income.latest.unwrap_or(0.0) < 0.0 && net_income.status == "weakening" {
            signs.push(WarningSign {
                kind: "rising_losses".to_string(),
                severity: severity_for_score(net_income.change_percent.unwrap_or(0.35).abs()),
                summary: "Net income is still in loss territory and moved in the wrong direction."
                    .to_string(),
            });
        }
    }

    if let Some(debt) = financial_trends.iter().find(|trend| trend.name == "Debt") {
        if debt.status == "weakening" {
            signs.push(WarningSign {
                kind: "heavier_borrowing".to_string(),
                severity: severity_for_score(debt.change_percent.unwrap_or(0.35).abs()),
                summary: "Debt increased compared with the prior quarter.".to_string(),
            });
        }
    }

    if let Some(gross_profit) = financial_trends
        .iter()
        .find(|trend| trend.name == "Gross Profit")
    {
        if gross_profit.status == "weakening" {
            signs.push(WarningSign {
                kind: "margin_compression".to_string(),
                severity: "medium".to_string(),
                summary: "Gross profit moved in the wrong direction relative to revenue."
                    .to_string(),
            });
        }
    }

    if let Some(risk_factor) = section_diffs
        .iter()
        .find(|diff| diff.name == "Risk Factors" && diff.change_score >= 0.35)
    {
        signs.push(WarningSign {
            kind: "new_risk_language".to_string(),
            severity: severity_for_score(risk_factor.change_score),
            summary: "Risk factor language changed materially compared with the prior quarter."
                .to_string(),
        });
    }

    if let Some(legal) = section_diffs
        .iter()
        .find(|diff| diff.name == "Legal Proceedings" && diff.change_score >= 0.35)
    {
        signs.push(WarningSign {
            kind: "legal_issues".to_string(),
            severity: severity_for_score(legal.change_score),
            summary: "Legal proceedings language changed enough to merit attention.".to_string(),
        });
    }

    signs.sort_by(|left, right| severity_rank(&right.severity).cmp(&severity_rank(&left.severity)));
    signs
}

pub fn narrative_notes(section_diffs: &[SectionDiff]) -> Vec<NarrativeNote> {
    let mut notes = section_diffs
        .iter()
        .filter(|diff| diff.change_score >= 0.20)
        .map(|diff| NarrativeNote {
            topic: narrative_topic(&diff.name),
            summary: summarizer::section_summary(diff),
        })
        .collect::<Vec<_>>();

    notes.truncate(3);
    notes
}

fn narrative_topic(name: &str) -> String {
    match name {
        "Risk Factors" => "risk factors".to_string(),
        "Management's Discussion and Analysis" => "management discussion".to_string(),
        "Legal Proceedings" => "legal proceedings".to_string(),
        other => other.to_ascii_lowercase(),
    }
}

fn severity_for_score(score: f64) -> String {
    if score >= 0.60 {
        "high".to_string()
    } else if score >= 0.35 {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "high" => 3,
        "medium" => 2,
        _ => 1,
    }
}
