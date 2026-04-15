use crate::{
    error::AppError,
    models::{FilingDocument, FilingMetadata},
    sec_client::SecClient,
};

pub async fn fetch_filing_document(
    sec_client: &SecClient,
    cik: u64,
    filing: &FilingMetadata,
) -> Result<FilingDocument, AppError> {
    let url = crate::filing_locator::build_filing_url(cik, filing);
    let html = sec_client.fetch_text(&url).await?;

    Ok(FilingDocument { url, html })
}
