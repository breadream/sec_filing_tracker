use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::header::CONTENT_TYPE,
    response::{Html, IntoResponse},
    routing::get,
};
use tower_http::trace::TraceLayer;

use crate::{
    diff,
    error::AppError,
    filing_fetcher, filing_locator, financial_metrics,
    models::{
        AnalyzeResponse, CompareQuery, CompareResponse, EvidenceItem, HealthResponse,
        ParsedSection, SectionComparisonResponse, SectionDiff,
    },
    sec_client::SecClient,
    section_parser, summarizer, trend_analyzer, warning_signs,
};

pub fn router(sec_client: Arc<SecClient>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/static/styles.css", get(styles))
        .route("/static/app.js", get(app_js))
        .route("/health", get(health))
        .route("/tickers", get(tickers))
        .route("/compare/:ticker", get(compare))
        .route("/analyze/:ticker", get(analyze))
        .with_state(sec_client)
        .layer(TraceLayer::new_for_http())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("static/index.html"))
}

async fn styles() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("static/styles.css"),
    )
}

async fn app_js() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "application/javascript; charset=utf-8")],
        include_str!("static/app.js"),
    )
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

async fn tickers(
    State(sec_client): State<Arc<SecClient>>,
) -> Result<Json<Vec<crate::models::CompanyTicker>>, AppError> {
    Ok(Json(sec_client.fetch_company_tickers().await?))
}

async fn compare(
    State(sec_client): State<Arc<SecClient>>,
    Path(ticker): Path<String>,
    Query(query): Query<CompareQuery>,
) -> Result<Json<CompareResponse>, AppError> {
    let ticker = ticker.trim().to_ascii_uppercase();
    if ticker.is_empty() {
        return Err(AppError::TickerNotFound);
    }

    let form = filing_locator::validate_form(query.form.as_deref())?;
    tracing::info!(ticker, form, "starting filing comparison");

    let company = sec_client.resolve_ticker(&ticker).await?;
    let submissions = sec_client.fetch_submissions(company.cik_str).await?;
    let located = filing_locator::latest_and_previous(&submissions, &form)?;

    let latest_document =
        filing_fetcher::fetch_filing_document(&sec_client, company.cik_str, &located.latest)
            .await?;
    let previous_document =
        filing_fetcher::fetch_filing_document(&sec_client, company.cik_str, &located.previous)
            .await?;

    let latest_sections = section_parser::extract_sections(&latest_document.html, &form);
    let previous_sections = section_parser::extract_sections(&previous_document.html, &form);
    let section_diffs = diff::compare_sections(&latest_sections, &previous_sections);

    if section_diffs.is_empty() {
        return Err(AppError::NoComparableSections);
    }

    let sections = section_diffs
        .iter()
        .map(|diff| section_response(diff, &latest_sections))
        .collect::<Vec<_>>();

    Ok(Json(CompareResponse {
        ticker,
        company_name: submissions.name,
        form,
        latest_filing_date: located.latest.filing_date,
        previous_filing_date: located.previous.filing_date,
        latest_filing_url: latest_document.url,
        previous_filing_url: previous_document.url,
        sections,
        overall_summary: summarizer::overall_summary(&section_diffs),
    }))
}

async fn analyze(
    State(sec_client): State<Arc<SecClient>>,
    Path(ticker): Path<String>,
) -> Result<Json<AnalyzeResponse>, AppError> {
    let ticker = ticker.trim().to_ascii_uppercase();
    if ticker.is_empty() {
        return Err(AppError::TickerNotFound);
    }

    let form = "10-Q".to_string();
    tracing::info!(ticker, "starting company health analysis");

    let company = sec_client.resolve_ticker(&ticker).await?;
    let submissions = sec_client.fetch_submissions(company.cik_str).await?;
    let company_facts = sec_client.fetch_company_facts(company.cik_str).await?;
    let located = filing_locator::latest_and_previous(&submissions, &form)?;

    let latest_document =
        filing_fetcher::fetch_filing_document(&sec_client, company.cik_str, &located.latest)
            .await?;
    let previous_document =
        filing_fetcher::fetch_filing_document(&sec_client, company.cik_str, &located.previous)
            .await?;

    let latest_sections = section_parser::extract_sections(&latest_document.html, &form);
    let previous_sections = section_parser::extract_sections(&previous_document.html, &form);
    let section_diffs = diff::compare_sections(&latest_sections, &previous_sections);

    if section_diffs.is_empty() {
        return Err(AppError::NoComparableSections);
    }

    let financial_trends = financial_metrics::financial_trends(&company_facts, company.cik_str);
    let mut overall_health = trend_analyzer::overall_health(&financial_trends, &section_diffs);
    let warning_signs = warning_signs::warning_signs(&financial_trends, &section_diffs);
    let management_explanation = warning_signs::narrative_notes(&section_diffs);

    let mut section_changes = section_diffs
        .iter()
        .map(|diff| section_response(diff, &latest_sections))
        .collect::<Vec<_>>();

    let (ai_analysis, ai_analysis_state) = crate::ai_analyzer::analyze_filing(
        &ticker,
        &submissions.name,
        &form,
        &located.latest.filing_date,
        &financial_trends,
        &latest_sections,
    )
    .await;
    crate::ai_analyzer::apply_ai_analysis(ai_analysis, &mut overall_health, &mut section_changes);

    Ok(Json(AnalyzeResponse {
        ticker,
        company_name: submissions.name,
        form,
        latest_filing_date: located.latest.filing_date,
        previous_filing_date: located.previous.filing_date,
        latest_filing_url: latest_document.url,
        previous_filing_url: previous_document.url,
        overall_health,
        financial_trends,
        warning_signs,
        management_explanation,
        section_changes,
        ai_analysis: ai_analysis_state,
    }))
}

fn section_response(
    diff: &SectionDiff,
    latest_sections: &[ParsedSection],
) -> SectionComparisonResponse {
    SectionComparisonResponse {
        name: diff.name.clone(),
        change_score: diff.change_score,
        attention_score: diff.change_score,
        status: summarizer::status_for(diff.change_score),
        summary: summarizer::section_summary(diff),
        analysis_basis:
            "Deterministic change intensity from wording similarity, paragraph overlap, and length movement versus the prior filing."
                .to_string(),
        similarity: diff.similarity,
        paragraph_overlap: diff.paragraph_overlap,
        length_delta: diff.length_delta,
        evidence: deterministic_section_evidence(diff, latest_sections),
    }
}

fn deterministic_section_evidence(
    diff: &SectionDiff,
    latest_sections: &[ParsedSection],
) -> Vec<EvidenceItem> {
    let mut evidence = vec![EvidenceItem {
        label: "Change basis".to_string(),
        snippet: format!(
            "Similarity {:.0}%, paragraph overlap {:.0}%, length movement {:.0}%.",
            diff.similarity * 100.0,
            diff.paragraph_overlap * 100.0,
            diff.length_delta * 100.0
        ),
    }];

    if let Some(section) = latest_sections
        .iter()
        .find(|section| section.name == diff.name)
        .and_then(|section| evidence_sentence(&diff.name, &section.text))
    {
        evidence.push(EvidenceItem {
            label: "Latest filing excerpt".to_string(),
            snippet: section,
        });
    }

    evidence
}

fn evidence_sentence(section_name: &str, text: &str) -> Option<String> {
    let keywords = match section_name {
        "Risk Factors" => &[
            "risk",
            "adverse",
            "material",
            "liquidity",
            "substantial doubt",
            "uncertain",
        ][..],
        "Management's Discussion and Analysis" => &[
            "revenue",
            "net loss",
            "liquidity",
            "cash",
            "margin",
            "operating",
        ][..],
        "Legal Proceedings" => &[
            "legal",
            "proceeding",
            "litigation",
            "claim",
            "contingencies",
        ][..],
        _ => &["risk", "revenue", "cash", "operating", "legal"][..],
    };

    let sentences = text
        .split('.')
        .map(str::trim)
        .filter(|sentence| sentence.split_whitespace().count() >= 8)
        .filter(|sentence| !sentence.starts_with("Item "))
        .collect::<Vec<_>>();

    sentences
        .iter()
        .find(|sentence| {
            let lower = sentence.to_ascii_lowercase();
            keywords.iter().any(|keyword| lower.contains(keyword))
        })
        .or_else(|| sentences.first())
        .map(|sentence| {
            let mut snippet = sentence.chars().take(260).collect::<String>();
            snippet.push('.');
            snippet
        })
}
