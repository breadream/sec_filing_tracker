use std::{collections::HashMap, time::Duration};

use reqwest::{
    Client, StatusCode,
    header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT},
};
use tokio::{sync::Mutex, time::Instant};

use crate::{
    company_facts::CompanyFacts,
    error::AppError,
    models::{CompanyTicker, Submissions},
};

pub struct SecClient {
    http: Client,
    min_delay: Duration,
    last_request_at: Mutex<Option<Instant>>,
}

impl SecClient {
    pub fn new(user_agent: &str, timeout: Duration, min_delay: Duration) -> Result<Self, AppError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(user_agent)
                .map_err(|err| AppError::Internal(format!("invalid SEC_USER_AGENT: {err}")))?,
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json,text/html,*/*"),
        );

        let http = Client::builder()
            .default_headers(headers)
            .timeout(timeout)
            .build()
            .map_err(|err| AppError::Internal(err.to_string()))?;

        Ok(Self {
            http,
            min_delay,
            last_request_at: Mutex::new(None),
        })
    }

    pub async fn resolve_ticker(&self, ticker: &str) -> Result<CompanyTicker, AppError> {
        self.fetch_company_tickers()
            .await?
            .into_iter()
            .find(|company| company.ticker.eq_ignore_ascii_case(ticker))
            .ok_or(AppError::TickerNotFound)
    }

    pub async fn fetch_company_tickers(&self) -> Result<Vec<CompanyTicker>, AppError> {
        // TODO: Cache this ticker map locally and load it from disk with a refresh TTL.
        let companies: HashMap<String, CompanyTicker> = self
            .get_json("https://www.sec.gov/files/company_tickers.json")
            .await?;
        let mut companies = companies.into_values().collect::<Vec<_>>();
        companies.sort_by(|a, b| a.ticker.cmp(&b.ticker));

        Ok(companies)
    }

    pub async fn fetch_submissions(&self, cik: u64) -> Result<Submissions, AppError> {
        let url = format!("https://data.sec.gov/submissions/CIK{cik:010}.json");
        self.get_json(&url).await
    }

    pub async fn fetch_company_facts(&self, cik: u64) -> Result<CompanyFacts, AppError> {
        let url = crate::company_facts::company_facts_url(cik);
        self.get_json(&url).await
    }

    pub async fn fetch_text(&self, url: &str) -> Result<String, AppError> {
        self.wait_for_turn().await;

        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Err(sec_status_error(url, response.status()));
        }

        response.text().await.map_err(AppError::from)
    }

    async fn get_json<T>(&self, url: &str) -> Result<T, AppError>
    where
        T: serde::de::DeserializeOwned,
    {
        self.wait_for_turn().await;

        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Err(sec_status_error(url, response.status()));
        }

        response.json::<T>().await.map_err(AppError::from)
    }

    async fn wait_for_turn(&self) {
        let mut last_request_at = self.last_request_at.lock().await;

        if let Some(last) = *last_request_at {
            let elapsed = last.elapsed();
            if elapsed < self.min_delay {
                tokio::time::sleep(self.min_delay - elapsed).await;
            }
        }

        *last_request_at = Some(Instant::now());
    }
}

fn sec_status_error(url: &str, status: StatusCode) -> AppError {
    AppError::SecFailure(format!("SEC returned {status} for {url}"))
}
