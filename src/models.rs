use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub ok: bool,
}

#[derive(Debug, Deserialize)]
pub struct CompareQuery {
    pub form: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompareResponse {
    pub ticker: String,
    pub company_name: String,
    pub form: String,
    pub latest_filing_date: String,
    pub previous_filing_date: String,
    pub latest_filing_url: String,
    pub previous_filing_url: String,
    pub sections: Vec<SectionComparisonResponse>,
    pub overall_summary: String,
}

#[derive(Debug, Serialize)]
pub struct AnalyzeResponse {
    pub ticker: String,
    pub company_name: String,
    pub form: String,
    pub latest_filing_date: String,
    pub previous_filing_date: String,
    pub latest_filing_url: String,
    pub previous_filing_url: String,
    pub overall_health: OverallHealth,
    pub financial_trends: Vec<FinancialTrend>,
    pub warning_signs: Vec<WarningSign>,
    pub management_explanation: Vec<NarrativeNote>,
    pub section_changes: Vec<SectionComparisonResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverallHealth {
    pub status: String,
    pub score: f64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FinancialTrend {
    pub name: String,
    pub unit: String,
    pub latest: Option<f64>,
    pub previous: Option<f64>,
    pub latest_period_end: Option<String>,
    pub previous_period_end: Option<String>,
    pub change_percent: Option<f64>,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WarningSign {
    pub kind: String,
    pub severity: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NarrativeNote {
    pub topic: String,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct SectionComparisonResponse {
    pub name: String,
    pub change_score: f64,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompanyTicker {
    pub cik_str: u64,
    pub ticker: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Submissions {
    pub name: String,
    pub filings: SubmissionFilings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmissionFilings {
    pub recent: RecentFilings,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFilings {
    pub accession_number: Vec<String>,
    pub filing_date: Vec<String>,
    pub form: Vec<String>,
    pub primary_document: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FilingMetadata {
    pub accession_number: String,
    pub filing_date: String,
    pub primary_document: String,
}

#[derive(Debug, Clone)]
pub struct LocatedFilings {
    pub latest: FilingMetadata,
    pub previous: FilingMetadata,
}

#[derive(Debug, Clone)]
pub struct FilingDocument {
    pub url: String,
    pub html: String,
}

#[derive(Debug, Clone)]
pub struct ParsedSection {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct SectionDiff {
    pub name: String,
    pub change_score: f64,
    pub length_delta: f64,
    pub paragraph_overlap: f64,
    pub similarity: f64,
}
