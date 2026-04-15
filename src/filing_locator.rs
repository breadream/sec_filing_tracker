use crate::{
    error::AppError,
    models::{FilingMetadata, LocatedFilings, Submissions},
};

pub fn validate_form(form: Option<&str>) -> Result<String, AppError> {
    let normalized = form.unwrap_or("10-Q").trim().to_ascii_uppercase();
    match normalized.as_str() {
        "10-Q" | "10-K" => Ok(normalized),
        _ => Err(AppError::InvalidForm),
    }
}

pub fn latest_and_previous(
    submissions: &Submissions,
    requested_form: &str,
) -> Result<LocatedFilings, AppError> {
    let recent = &submissions.filings.recent;
    let count = [
        recent.accession_number.len(),
        recent.filing_date.len(),
        recent.form.len(),
        recent.primary_document.len(),
    ]
    .into_iter()
    .min()
    .unwrap_or(0);

    let mut filings = Vec::new();
    for index in 0..count {
        if recent.form[index] == requested_form {
            filings.push(FilingMetadata {
                accession_number: recent.accession_number[index].clone(),
                filing_date: recent.filing_date[index].clone(),
                primary_document: recent.primary_document[index].clone(),
            });
        }
    }

    if filings.len() < 2 {
        return Err(AppError::NotEnoughFilings);
    }

    filings.sort_by(|a, b| b.filing_date.cmp(&a.filing_date));

    Ok(LocatedFilings {
        latest: filings[0].clone(),
        previous: filings[1].clone(),
    })
}

pub fn build_filing_url(cik: u64, filing: &FilingMetadata) -> String {
    let accession_no_dashes = filing.accession_number.replace('-', "");
    format!(
        "https://www.sec.gov/Archives/edgar/data/{}/{}/{}",
        cik, accession_no_dashes, filing.primary_document
    )
}
