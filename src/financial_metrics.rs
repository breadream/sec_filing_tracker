use crate::{
    company_facts::{
        CompanyFacts, FactPoint, latest_previous_by_period_end, select_recent_usd_facts,
    },
    models::FinancialTrend,
};

const USD_MILLIONS: f64 = 1_000_000.0;

pub fn financial_trends(facts: &CompanyFacts) -> Vec<FinancialTrend> {
    let revenue = concept_trend(
        facts,
        "Revenue",
        &[
            "RevenueFromContractWithCustomerExcludingAssessedTax",
            "Revenues",
            "SalesRevenueNet",
        ],
        Direction::HigherIsBetter,
    );
    let net_income = concept_trend(
        facts,
        "Net income",
        &["NetIncomeLoss"],
        Direction::HigherIsBetter,
    );
    let operating_income = concept_trend(
        facts,
        "Operating income",
        &["OperatingIncomeLoss"],
        Direction::HigherIsBetter,
    );
    let gross_profit = concept_trend(
        facts,
        "Gross profit",
        &["GrossProfit"],
        Direction::HigherIsBetter,
    );
    let operating_cash_flow = concept_trend(
        facts,
        "Operating cash flow",
        &["NetCashProvidedByUsedInOperatingActivities"],
        Direction::HigherIsBetter,
    );
    let cash = concept_trend(
        facts,
        "Cash and equivalents",
        &[
            "CashAndCashEquivalentsAtCarryingValue",
            "CashAndCashEquivalentsAndShortTermInvestments",
        ],
        Direction::HigherIsBetter,
    );
    let debt = concept_trend(
        facts,
        "Debt",
        &[
            "LongTermDebtAndFinanceLeaseObligations",
            "LongTermDebt",
            "LongTermDebtAndFinanceLeaseObligationsCurrent",
            "LongTermDebtAndFinanceLeaseObligationsNoncurrent",
            "LongTermDebtCurrent",
            "LongTermDebtNoncurrent",
        ],
        Direction::LowerIsBetter,
    );

    let mut trends = vec![
        revenue.clone(),
        net_income.clone(),
        operating_income.clone(),
        gross_profit.clone(),
        operating_cash_flow,
        cash,
        debt,
    ];

    if let Some(margin) = margin_trend("Net margin", &revenue, &net_income) {
        trends.push(margin);
    }
    if let Some(margin) = margin_trend("Operating margin", &revenue, &operating_income) {
        trends.push(margin);
    }
    if let Some(margin) = margin_trend("Gross margin", &revenue, &gross_profit) {
        trends.push(margin);
    }

    trends
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    HigherIsBetter,
    LowerIsBetter,
}

fn concept_trend(
    facts: &CompanyFacts,
    name: &str,
    concept_aliases: &[&str],
    direction: Direction,
) -> FinancialTrend {
    let pair = select_recent_usd_facts(facts, concept_aliases, "10-Q")
        .and_then(|series| latest_previous_by_period_end(&prefer_quarterly_points(&series.facts)));

    trend_from_pair(
        name,
        pair.as_ref().map(|pair| (&pair.latest, &pair.previous)),
        direction,
    )
}

fn trend_from_pair(
    name: &str,
    pair: Option<(&FactPoint, &FactPoint)>,
    direction: Direction,
) -> FinancialTrend {
    let latest = pair.map(|(latest, _)| latest);
    let previous = pair.map(|(_, previous)| previous);
    let change_percent = latest
        .zip(previous)
        .and_then(|(latest, previous)| percent_change(latest.value, previous.value));
    let status = trend_status(change_percent, direction);

    FinancialTrend {
        name: name.to_string(),
        unit: "USD millions".to_string(),
        latest: latest.map(|point| round_millions(point.value / USD_MILLIONS)),
        previous: previous.map(|point| round_millions(point.value / USD_MILLIONS)),
        latest_period_end: latest.map(|point| point.period_end.clone()),
        previous_period_end: previous.map(|point| point.period_end.clone()),
        change_percent,
        status: status.clone(),
        summary: trend_summary(name, change_percent, &status),
    }
}

fn margin_trend(
    name: &str,
    revenue: &FinancialTrend,
    numerator: &FinancialTrend,
) -> Option<FinancialTrend> {
    let latest = numerator.latest.zip(revenue.latest).and_then(ratio_percent);
    let previous = numerator
        .previous
        .zip(revenue.previous)
        .and_then(ratio_percent);
    let change_percent = latest.zip(previous).map(|(latest, previous)| {
        if previous == 0.0 {
            0.0
        } else {
            round_percent(((latest - previous) / previous.abs()) * 100.0)
        }
    });
    let status = trend_status(change_percent, Direction::HigherIsBetter);

    Some(FinancialTrend {
        name: name.to_string(),
        unit: "percent".to_string(),
        latest,
        previous,
        latest_period_end: revenue.latest_period_end.clone(),
        previous_period_end: revenue.previous_period_end.clone(),
        change_percent,
        status: status.clone(),
        summary: trend_summary(name, change_percent, &status),
    })
}

fn prefer_quarterly_points(points: &[FactPoint]) -> Vec<FactPoint> {
    let quarterly = points
        .iter()
        .filter(|point| {
            point
                .duration_days
                .is_some_and(|days| (70..=110).contains(&days))
        })
        .cloned()
        .collect::<Vec<_>>();

    if quarterly.len() >= 2 {
        quarterly
    } else {
        points.to_vec()
    }
}

fn trend_status(change_percent: Option<f64>, direction: Direction) -> String {
    let Some(change_percent) = change_percent else {
        return "unknown".to_string();
    };

    let improving = match direction {
        Direction::HigherIsBetter => change_percent >= 3.0,
        Direction::LowerIsBetter => change_percent <= -3.0,
    };
    let weakening = match direction {
        Direction::HigherIsBetter => change_percent <= -3.0,
        Direction::LowerIsBetter => change_percent >= 3.0,
    };

    if improving {
        "improving"
    } else if weakening {
        "weakening"
    } else {
        "stable"
    }
    .to_string()
}

fn trend_summary(name: &str, change_percent: Option<f64>, status: &str) -> String {
    match change_percent {
        Some(change) => {
            format!("{name} is {status}, changing {change:.1}% versus the prior 10-Q period.")
        }
        None => format!("{name} could not be compared from available SEC company facts."),
    }
}

fn percent_change(latest: f64, previous: f64) -> Option<f64> {
    if previous == 0.0 {
        None
    } else {
        Some(round_percent(
            ((latest - previous) / previous.abs()) * 100.0,
        ))
    }
}

fn ratio_percent((numerator, denominator): (f64, f64)) -> Option<f64> {
    if denominator == 0.0 {
        None
    } else {
        Some(round_percent((numerator / denominator) * 100.0))
    }
}

fn round_percent(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn round_millions(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}
