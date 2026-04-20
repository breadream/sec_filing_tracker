use std::{collections::HashMap, env, time::Duration};

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::models::{
    AiAnalysisState, EvidenceItem, FinancialTrend, OverallHealth, ParsedSection,
    SectionComparisonResponse,
};

const DEFAULT_MODEL: &str = "gpt-4.1-mini";
const MAX_SECTION_CHARS: usize = 7_500;

#[derive(Debug, Deserialize)]
pub(crate) struct AiFilingAnalysis {
    health_score: f64,
    health_status: String,
    health_summary: String,
    health_evidence: Vec<AiEvidence>,
    sections: Vec<AiSectionAnalysis>,
}

#[derive(Debug, Deserialize)]
struct AiEvidence {
    label: String,
    impact: String,
    summary: String,
    evidence: String,
}

#[derive(Debug, Deserialize)]
struct AiSectionAnalysis {
    name: String,
    attention_score: f64,
    status: String,
    summary: String,
    evidence: Vec<EvidenceItem>,
}

pub(crate) async fn analyze_filing(
    ticker: &str,
    company_name: &str,
    form: &str,
    latest_filing_date: &str,
    financial_trends: &[FinancialTrend],
    latest_sections: &[ParsedSection],
) -> (Option<AiFilingAnalysis>, AiAnalysisState) {
    let Ok(api_key) = env::var("OPENAI_API_KEY") else {
        return (
            None,
            AiAnalysisState {
                enabled: false,
                used: false,
                model: None,
                message: "AI evidence is off. Set OPENAI_API_KEY to analyze the latest 10-Q language with an AI reviewer.".to_string(),
            },
        );
    };

    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .build();
    let Ok(client) = client else {
        return ai_failure_state(model, "The AI HTTP client could not be created.");
    };

    let request = json!({
        "model": model,
        "instructions": "You are a cautious public-company 10-Q analyst. Score company health from the latest filing evidence, not stock price. Use financial trends, latest disclosure language, risk severity, liquidity, profitability, legal exposure, and management commentary. Do not treat unchanged boilerplate as good or bad by itself. Return only JSON matching the requested schema. Keep every evidence snippet short and copied from the provided filing text or metric summaries.",
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": build_prompt(ticker, company_name, form, latest_filing_date, financial_trends, latest_sections),
            }]
        }],
        "text": {
            "format": {
                "type": "json_schema",
                "name": "filing_health_analysis",
                "strict": true,
                "schema": response_schema()
            }
        }
    });

    let response = client
        .post("https://api.openai.com/v1/responses")
        .header(AUTHORIZATION, format!("Bearer {api_key}"))
        .header(CONTENT_TYPE, "application/json")
        .json(&request)
        .send()
        .await;

    let Ok(response) = response else {
        return ai_failure_state(model, "The AI request could not reach OpenAI.");
    };

    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "OpenAI analysis request failed");
        return ai_failure_state(
            model,
            "OpenAI returned an error, so deterministic analysis was used.",
        );
    }

    let body = response.json::<Value>().await;
    let Ok(body) = body else {
        return ai_failure_state(model, "OpenAI returned an unreadable response.");
    };

    let Some(text) = extract_output_text(&body) else {
        return ai_failure_state(model, "OpenAI did not return structured analysis text.");
    };

    match serde_json::from_str::<AiFilingAnalysis>(&text) {
        Ok(analysis) => (
            Some(analysis),
            AiAnalysisState {
                enabled: true,
                used: true,
                model: Some(model),
                message: "AI evidence used the latest 10-Q sections plus SEC company-facts trends."
                    .to_string(),
            },
        ),
        Err(err) => {
            tracing::warn!(%err, "OpenAI analysis JSON did not match expected shape");
            ai_failure_state(
                model,
                "OpenAI returned analysis in an unexpected shape, so deterministic analysis was used.",
            )
        }
    }
}

pub(crate) fn apply_ai_analysis(
    analysis: Option<AiFilingAnalysis>,
    overall_health: &mut OverallHealth,
    sections: &mut [SectionComparisonResponse],
) {
    let Some(analysis) = analysis else {
        return;
    };

    overall_health.score = normalize_ai_score(analysis.health_score);
    overall_health.status = normalize_health_status(&analysis.health_status, overall_health.score);
    overall_health.summary = analysis.health_summary;
    overall_health.methodology =
        "AI reviewed the latest 10-Q narrative sections together with SEC company-facts trends, then assigned an evidence-backed attention score.".to_string();
    overall_health.drivers = analysis
        .health_evidence
        .into_iter()
        .map(|item| crate::models::HealthDriver {
            label: item.label,
            impact: normalize_impact(&item.impact),
            summary: item.summary,
            evidence: Some(item.evidence),
        })
        .collect();

    let section_map = analysis
        .sections
        .into_iter()
        .map(|section| (section.name.to_ascii_lowercase(), section))
        .collect::<HashMap<_, _>>();

    for section in sections {
        let Some(ai_section) = section_map.get(&section.name.to_ascii_lowercase()) else {
            continue;
        };

        section.attention_score = normalize_ai_score(ai_section.attention_score);
        section.status = normalize_section_status(&ai_section.status, section.attention_score);
        section.summary = ai_section.summary.clone();
        section.analysis_basis =
            "AI attention score from latest-filing risk severity, management language, legal exposure, and supporting text.".to_string();
        section.evidence = ai_section.evidence.clone();
    }
}

fn build_prompt(
    ticker: &str,
    company_name: &str,
    form: &str,
    latest_filing_date: &str,
    financial_trends: &[FinancialTrend],
    latest_sections: &[ParsedSection],
) -> String {
    let metrics = financial_trends
        .iter()
        .map(|trend| {
            format!(
                "- {}: latest {:?} {}, previous {:?}, change {:?}%, status {}, summary: {}",
                trend.name,
                trend.latest,
                trend.unit,
                trend.previous,
                trend.change_percent,
                trend.status,
                trend.summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let sections = latest_sections
        .iter()
        .map(|section| {
            format!(
                "## {}\n{}",
                section.name,
                truncate_chars(&section.text, MAX_SECTION_CHARS)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "Company: {company_name} ({ticker})\nForm: {form}\nLatest filing date: {latest_filing_date}\n\nFinancial trend signals:\n{metrics}\n\nLatest filing sections:\n{sections}\n\nReturn a health_score from 0 to 100, where 100 is healthiest. Use health_status strong, steady, watch, or stressed. For section status use good, watch, or bad. Scores should be grounded in current operating risk and disclosure evidence, not only quarter-over-quarter wording change."
    )
}

fn response_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["health_score", "health_status", "health_summary", "health_evidence", "sections"],
        "properties": {
            "health_score": { "type": "number" },
            "health_status": { "type": "string" },
            "health_summary": { "type": "string" },
            "health_evidence": {
                "type": "array",
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["label", "impact", "summary", "evidence"],
                    "properties": {
                        "label": { "type": "string" },
                        "impact": { "type": "string" },
                        "summary": { "type": "string" },
                        "evidence": { "type": "string" }
                    }
                }
            },
            "sections": {
                "type": "array",
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["name", "attention_score", "status", "summary", "evidence"],
                    "properties": {
                        "name": { "type": "string" },
                        "attention_score": { "type": "number" },
                        "status": { "type": "string" },
                        "summary": { "type": "string" },
                        "evidence": {
                            "type": "array",
                            "maxItems": 3,
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "required": ["label", "snippet"],
                                "properties": {
                                    "label": { "type": "string" },
                                    "snippet": { "type": "string" }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

fn extract_output_text(body: &Value) -> Option<String> {
    if let Some(text) = body.get("output_text").and_then(Value::as_str) {
        return Some(text.to_string());
    }

    body.get("output")?
        .as_array()?
        .iter()
        .flat_map(|item| {
            item.get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .find_map(|content| {
            content
                .get("text")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

fn ai_failure_state(model: String, message: &str) -> (Option<AiFilingAnalysis>, AiAnalysisState) {
    (
        None,
        AiAnalysisState {
            enabled: true,
            used: false,
            model: Some(model),
            message: message.to_string(),
        },
    )
}

fn normalize_ai_score(score: f64) -> f64 {
    if score > 1.0 {
        (score / 100.0).clamp(0.0, 1.0)
    } else {
        score.clamp(0.0, 1.0)
    }
}

fn normalize_health_status(status: &str, score: f64) -> String {
    let text = status.to_ascii_lowercase();
    if text.contains("strong") || text.contains("healthy") {
        "strong".to_string()
    } else if text.contains("steady") {
        "steady".to_string()
    } else if text.contains("watch") || text.contains("mixed") {
        "watch".to_string()
    } else if text.contains("stress") || text.contains("weak") || text.contains("bad") {
        "stressed".to_string()
    } else if score >= 0.72 {
        "strong".to_string()
    } else if score >= 0.58 {
        "steady".to_string()
    } else if score >= 0.45 {
        "watch".to_string()
    } else {
        "stressed".to_string()
    }
}

fn normalize_section_status(status: &str, score: f64) -> String {
    let text = status.to_ascii_lowercase();
    if text.contains("good") || text.contains("low") || text.contains("stable") {
        "good".to_string()
    } else if text.contains("bad") || text.contains("high") || text.contains("material") {
        "bad".to_string()
    } else if text.contains("watch") || text.contains("medium") || text.contains("moderate") {
        "watch".to_string()
    } else if score >= 0.72 {
        "bad".to_string()
    } else if score >= 0.35 {
        "watch".to_string()
    } else {
        "good".to_string()
    }
}

fn normalize_impact(impact: &str) -> String {
    let text = impact.to_ascii_lowercase();
    if text.contains("positive") || text.contains("up") || text.contains("good") {
        "positive".to_string()
    } else if text.contains("negative") || text.contains("down") || text.contains("bad") {
        "negative".to_string()
    } else {
        "neutral".to_string()
    }
}

fn truncate_chars(text: &str, limit: usize) -> String {
    let mut output = String::new();
    for character in text.chars().take(limit) {
        output.push(character);
    }
    output
}
