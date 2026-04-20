const form = document.querySelector("#compare-form");
const tickerInput = document.querySelector("#ticker");
const tickerResults = document.querySelector("#ticker-results");
const message = document.querySelector("#message");
const analysisStatus = document.querySelector("#analysis-status");
const analysisStatusTitle = document.querySelector("#analysis-status-title");
const analysisStatusPercent = document.querySelector("#analysis-status-percent");
const analysisStatusFill = document.querySelector("#analysis-status-fill");
const analysisStatusCopy = document.querySelector("#analysis-status-copy");
const analysisStatusSteps = document.querySelector("#analysis-status-steps");
const dashboard = document.querySelector("#dashboard");
const companyLabel = document.querySelector("#company-label");
const healthSummary = document.querySelector("#health-summary");
const healthStatus = document.querySelector("#health-status");
const healthScore = document.querySelector("#health-score");
const healthScoreFill = document.querySelector("#health-score-fill");
const healthMethodology = document.querySelector("#health-methodology");
const healthDrivers = document.querySelector("#health-drivers");
const latestDate = document.querySelector("#latest-date");
const previousDate = document.querySelector("#previous-date");
const latestLink = document.querySelector("#latest-link");
const previousLink = document.querySelector("#previous-link");
const trendSource = document.querySelector("#trend-source");
const trendGrid = document.querySelector("#trend-grid");
const warningList = document.querySelector("#warning-list");
const noteList = document.querySelector("#note-list");
const sectionCount = document.querySelector("#section-count");
const sections = document.querySelector("#sections");

const TREND_ORDER = [
  { key: "revenue", label: "Revenue" },
  { key: "profit", label: "Profit / loss" },
  { key: "cash_flow", label: "Cash flow" },
  { key: "debt", label: "Debt" },
  { key: "margins", label: "Margins" },
];

const ANALYSIS_STAGES = [
  {
    label: "Resolve company",
    copy: "Match the ticker, confirm the company, and prepare the SEC lookup.",
    percent: 14,
  },
  {
    label: "Pull filings",
    copy: "Fetch the latest 10-Q references and gather the filing sources.",
    percent: 36,
  },
  {
    label: "Build signals",
    copy: "Compute revenue, income, cash flow, debt, and margin trends from SEC company facts.",
    percent: 68,
  },
  {
    label: "Review narrative",
    copy: "Compare filing sections and, when enabled, let AI review the latest 10-Q language.",
    percent: 92,
  },
];

let tickerCompanies = [];
let tickerLookup = null;
let tickerRequest = null;
let selectedTicker = tickerInput.value.trim().toUpperCase();
let analysisProgressInterval = null;
let analysisProgressState = null;
let analysisProgressHideTimeout = null;

tickerInput.addEventListener("input", async () => {
  selectedTicker = "";
  await renderTickerMatches(tickerInput.value);
});

tickerInput.addEventListener("focus", async () => {
  await renderTickerMatches(tickerInput.value);
});

document.addEventListener("click", (event) => {
  if (!form.contains(event.target)) {
    hideTickerMatches();
  }
});

tickerResults.addEventListener("mousedown", (event) => {
  const option = event.target.closest("[data-ticker]");
  if (!option) {
    return;
  }

  event.preventDefault();
  chooseTicker(option.dataset.ticker);
});

form.addEventListener("submit", async (event) => {
  event.preventDefault();

  const ticker = await resolveSubmittedTicker();
  if (!ticker) {
    showError("Enter a ticker first.");
    return;
  }

  tickerInput.value = ticker;
  await loadDashboard(ticker);
});

async function resolveSubmittedTicker() {
  const value = tickerInput.value.trim();
  if (!value) {
    return "";
  }

  const normalized = value.toUpperCase();
  if (selectedTicker && selectedTicker === normalized) {
    return selectedTicker;
  }

  try {
    const lookup = await getTickerLookup();
    const exactTicker = lookup.byTicker.get(normalized);
    if (exactTicker) {
      return exactTicker.ticker;
    }

    const exactCompany = lookup.byTitle.get(normalizeSearchText(value));
    if (exactCompany) {
      chooseTicker(exactCompany.ticker);
      return exactCompany.ticker;
    }

    const firstMatch = searchTickerCompanies(value, 1)[0];
    if (firstMatch) {
      chooseTicker(firstMatch.ticker);
      return firstMatch.ticker;
    }
  } catch {
    return normalized;
  }

  return normalized;
}

async function renderTickerMatches(value) {
  const query = value.trim();
  if (!query) {
    hideTickerMatches();
    return;
  }

  try {
    await loadTickerCompanies();
  } catch {
    tickerResults.replaceChildren(createTickerStatus("Ticker search is unavailable right now."));
    tickerResults.classList.remove("is-hidden");
    return;
  }

  const matches = searchTickerCompanies(query, 8);
  if (!matches.length) {
    tickerResults.replaceChildren(createTickerStatus("No company matches found."));
    tickerResults.classList.remove("is-hidden");
    return;
  }

  tickerResults.replaceChildren(...matches.map(createTickerOption));
  tickerResults.classList.remove("is-hidden");
}

function searchTickerCompanies(query, limit) {
  const normalized = normalizeSearchText(query);
  if (!normalized) {
    return [];
  }

  return tickerCompanies
    .map((company) => {
      const ticker = company.ticker.toUpperCase();
      const title = normalizeSearchText(company.title);
      let rank = 0;

      if (ticker === normalized) {
        rank = 1;
      } else if (ticker.startsWith(normalized)) {
        rank = 2;
      } else if (title.startsWith(normalized)) {
        rank = 3;
      } else if (ticker.includes(normalized)) {
        rank = 4;
      } else if (title.includes(normalized)) {
        rank = 5;
      }

      return rank ? { company, rank } : null;
    })
    .filter(Boolean)
    .sort(
      (a, b) =>
        a.rank - b.rank ||
        a.company.title.length - b.company.title.length ||
        a.company.ticker.localeCompare(b.company.ticker),
    )
    .slice(0, limit)
    .map((match) => match.company);
}

async function loadTickerCompanies() {
  if (tickerCompanies.length) {
    return tickerCompanies;
  }

  if (!tickerRequest) {
    tickerRequest = fetchJson("/tickers").then((response) => {
      if (!response.ok || !Array.isArray(response.payload)) {
        throw new Error(response.error || "Ticker search is unavailable right now.");
      }

      tickerCompanies = response.payload
        .filter((company) => company?.ticker && company?.title)
        .map((company) => ({
          cik: company.cik_str,
          ticker: String(company.ticker).toUpperCase(),
          title: String(company.title),
        }));
      tickerLookup = null;
      return tickerCompanies;
    });
  }

  return tickerRequest;
}

async function getTickerLookup() {
  await loadTickerCompanies();

  if (tickerLookup) {
    return tickerLookup;
  }

  tickerLookup = {
    byTicker: new Map(),
    byTitle: new Map(),
  };

  tickerCompanies.forEach((company) => {
    tickerLookup.byTicker.set(company.ticker.toUpperCase(), company);
    tickerLookup.byTitle.set(normalizeSearchText(company.title), company);
  });

  return tickerLookup;
}

function createTickerOption(company) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "ticker-option";
  button.dataset.ticker = company.ticker;
  button.setAttribute("role", "option");

  const ticker = document.createElement("strong");
  ticker.textContent = company.ticker;

  const title = document.createElement("span");
  title.textContent = company.title;

  button.append(ticker, title);
  return button;
}

function createTickerStatus(text) {
  const status = document.createElement("p");
  status.className = "ticker-status";
  status.textContent = text;
  return status;
}

function chooseTicker(ticker) {
  selectedTicker = ticker.toUpperCase();
  tickerInput.value = selectedTicker;
  hideTickerMatches();
}

function hideTickerMatches() {
  tickerResults.classList.add("is-hidden");
  tickerResults.replaceChildren();
}

async function loadDashboard(ticker) {
  setLoading(true, `Checking the latest 10-Q for ${ticker}...`);
  startAnalysisStatus(ticker);

  try {
    const payload = await fetchDashboardPayload(ticker);
    const normalized = normalizePayload(payload);

    if (!normalized) {
      throw new Error("The server returned an unexpected response shape.");
    }

    completeAnalysisStatus(normalized);
    renderDashboard(normalized);
    showMessage(normalized.message);
  } catch (error) {
    stopAnalysisStatus();
    hideDashboard();
    showError(error.message);
  } finally {
    setLoading(false);
  }
}

async function fetchDashboardPayload(ticker) {
  const analyzed = await fetchJson(`/analyze/${encodeURIComponent(ticker)}`);
  if (analyzed.ok) {
    const normalized = normalizeAnalyzePayload(analyzed.payload);
    if (normalized) {
      return analyzed.payload;
    }

    const compared = await fetchJson(`/compare/${encodeURIComponent(ticker)}?form=10-Q`);
    if (compared.ok) {
      return compared.payload;
    }

    throw new Error(compared.error || "The comparison request could not be completed.");
  }

  if (analyzed.status === 404 || analyzed.status >= 500 || analyzed.invalidShape) {
    const compared = await fetchJson(`/compare/${encodeURIComponent(ticker)}?form=10-Q`);
    if (!compared.ok) {
      throw new Error(compared.error || "The comparison request could not be completed.");
    }

    return compared.payload;
  }

  throw new Error(analyzed.error || "The analysis request could not be completed.");
}

async function fetchJson(url) {
  try {
    const response = await fetch(url);
    const text = await response.text();

    if (!text) {
      return {
        ok: response.ok,
        status: response.status,
        payload: null,
        error: response.ok ? "" : defaultHttpError(response.status),
        invalidShape: response.ok,
      };
    }

    let payload;
    try {
      payload = JSON.parse(text);
    } catch {
      return {
        ok: false,
        status: response.status,
        payload: null,
        error: "The server returned an unreadable response.",
        invalidShape: true,
      };
    }

    return {
      ok: response.ok,
      status: response.status,
      payload,
      error: response.ok ? "" : payload.error || defaultHttpError(response.status),
      invalidShape: false,
    };
  } catch {
    return {
      ok: false,
      status: 0,
      payload: null,
      error: "The request could not reach the server.",
      invalidShape: false,
    };
  }
}

function normalizePayload(payload) {
  return normalizeAnalyzePayload(payload) || normalizeComparePayload(payload);
}

function normalizeAnalyzePayload(payload) {
  if (!isObject(payload)) {
    return null;
  }

  const hasStructuredFields =
    Boolean(payload.overall_health || payload.financial_trends || payload.warning_signs) ||
    Boolean(payload.management_explanation || payload.management_notes || payload.section_changes);

  if (!hasStructuredFields) {
    return null;
  }

  const overallHealth = normalizeOverallHealth(payload.overall_health || payload.health, payload);
  const financialTrends = normalizeFinancialTrends(payload.financial_trends || payload.trends || payload.metrics);
  const warningSigns = normalizeWarnings(payload.warning_signs || payload.warnings || payload.alerts);
  const managementNotes = normalizeNarrativeNotes(
    payload.management_explanation || payload.management_notes || payload.management_summary || payload.management_commentary,
    "Management",
  );
  const riskNotes = normalizeNarrativeNotes(
    payload.risk_notes || payload.risk_summary || payload.risk_factors || payload.risk_commentary,
    "Risk",
  );
  const sectionChanges = normalizeSectionChanges(payload.section_changes || payload.sections);

  return {
    sourceLabel: "Structured analysis from /analyze.",
    message: payload.overall_summary || overallHealth.summary || "The latest filing is ready.",
    ticker: String(payload.ticker || "").toUpperCase(),
    companyName: payload.company_name || payload.company || "Company",
    form: payload.form || "10-Q",
    latestDate: payload.latest_filing_date || payload.latest_date || "—",
    previousDate: payload.previous_filing_date || payload.previous_date || "—",
    latestUrl: payload.latest_filing_url || payload.latest_url || "",
    previousUrl: payload.previous_filing_url || payload.previous_url || "",
    overallHealth,
    aiAnalysis: normalizeAiAnalysis(payload.ai_analysis),
    financialTrends,
    warningSigns,
    managementNotes,
    riskNotes,
    sectionChanges,
  };
}

function normalizeComparePayload(payload) {
  if (!isObject(payload) || !Array.isArray(payload.sections)) {
    return null;
  }

  const sectionChanges = normalizeSectionChanges(payload.sections);
  const topScore = sectionChanges.reduce((max, section) => Math.max(max, section.changeScore), 0);
  const overallHealth = {
    status: deriveComparisonStatus(topScore),
    score: topScore,
    summary:
      payload.overall_summary ||
      "This endpoint currently returns section comparison details only. Structured financial trend data is not available yet.",
    methodology: "Fallback view from section comparison only.",
    drivers: [],
  };

  return {
    sourceLabel: "Fallback compare from /compare.",
    message: "Structured analysis was not available, so the section comparison view was used instead.",
    ticker: String(payload.ticker || "").toUpperCase(),
    companyName: payload.company_name || "Company",
    form: payload.form || "10-Q",
    latestDate: payload.latest_filing_date || "—",
    previousDate: payload.previous_filing_date || "—",
    latestUrl: payload.latest_filing_url || "",
    previousUrl: payload.previous_filing_url || "",
    overallHealth,
    aiAnalysis: {
      enabled: false,
      used: false,
      message: "AI evidence is not available in compare fallback.",
    },
    financialTrends: buildFallbackTrends(),
    warningSigns: deriveWarningsFromSections(sectionChanges),
    managementNotes: deriveNarrativeFromSections(sectionChanges, "management"),
    riskNotes: deriveNarrativeFromSections(sectionChanges, "risk"),
    sectionChanges,
  };
}

function normalizeOverallHealth(value, payload) {
  if (!isObject(value)) {
    const score = fallbackHealthScore(payload);
    return {
      status: deriveHealthStatus(score),
      score,
      summary:
        payload.overall_summary ||
        "The filing is available, but the backend did not return a structured health summary yet.",
      methodology: "Fallback score inferred from available filing comparison data.",
      drivers: [],
    };
  }

  const score = clampScore(value.score ?? value.value ?? fallbackHealthScore(payload));
  return {
    status: normalizeStatus(value.status || value.trend || value.label, score),
    score,
    summary:
      value.summary ||
      value.message ||
      payload.overall_summary ||
      "The filing is available, but the backend did not return a structured health summary yet.",
    methodology: value.methodology || "Score is based on available operating and filing signals.",
    drivers: normalizeHealthDrivers(value.drivers || value.contributions || value.evidence),
  };
}

function normalizeAiAnalysis(value) {
  if (!isObject(value)) {
    return {
      enabled: false,
      used: false,
      message: "",
    };
  }

  return {
    enabled: Boolean(value.enabled),
    used: Boolean(value.used),
    model: value.model || "",
    message: value.message || "",
  };
}

function normalizeHealthDrivers(value) {
  return coerceArray(value)
    .map((item) => {
      if (typeof item === "string") {
        return {
          label: "Evidence",
          impact: "neutral",
          summary: item,
          evidence: "",
        };
      }

      if (!isObject(item)) {
        return null;
      }

      return {
        label: item.label || item.name || item.title || "Signal",
        impact: normalizeImpact(item.impact || item.direction || item.status),
        summary: item.summary || item.message || item.note || "",
        evidence: item.evidence || item.snippet || item.detail || "",
      };
    })
    .filter(Boolean);
}

function normalizeFinancialTrends(value) {
  if (Array.isArray(value)) {
    return value.map((item, index) => normalizeTrendItem(item, TREND_ORDER[index]?.label || `Metric ${index + 1}`));
  }

  if (!isObject(value)) {
    return buildFallbackTrends();
  }

  return TREND_ORDER.map((entry) => normalizeTrendItem(value[entry.key], entry.label));
}

function normalizeTrendItem(item, fallbackLabel) {
  if (!isObject(item)) {
    return {
      label: fallbackLabel,
      latest: null,
      previous: null,
      delta: null,
      unit: "",
      status: "unknown",
      note: "Structured metric data is not available yet.",
      available: false,
    };
  }

  const latest = pickFirst(item, ["latest", "current", "value", "this_period", "period_current"]);
  const previous = pickFirst(item, ["previous", "prior", "last", "previous_period", "period_previous"]);
  const delta =
    pickFirst(item, ["change_percent", "change", "delta", "percent_change", "growth"]) ??
    computeDelta(latest, previous);
  const status = normalizeTrendStatus(item.status || item.trend || item.direction || item.signal);

  return {
    label: item.label || item.name || item.title || fallbackLabel,
    latest,
    previous,
    delta,
    unit: item.unit || item.units || "",
    status,
    note: item.summary || item.note || item.explanation || item.description || "",
    latestPeriodEnd: item.latest_period_end || item.latestPeriodEnd || "",
    previousPeriodEnd: item.previous_period_end || item.previousPeriodEnd || "",
    sourceLabel: item.source_label || item.sourceLabel || "",
    sourceNamespace: item.source_namespace || item.sourceNamespace || "",
    sourceConcept: item.source_concept || item.sourceConcept || "",
    sourceEndpointFamily: item.source_endpoint_family || item.sourceEndpointFamily || "",
    sourceUrl: item.source_url || item.sourceUrl || "",
    sourceForm: item.source_form || item.sourceForm || "",
    sourceFiled: item.source_filed || item.sourceFiled || "",
    available: latest !== null || previous !== null || delta !== null,
  };
}

function normalizeWarnings(value) {
  const items = coerceArray(value);
  return items
    .map((item) => {
      if (typeof item === "string") {
        return {
          title: item,
          summary: "",
          severity: "medium",
        };
      }

      if (!isObject(item)) {
        return null;
      }

      return {
        title: item.title || item.name || item.type || "Warning",
        summary: item.summary || item.message || item.note || "",
        severity: normalizeSeverity(item.severity || item.level || item.status || "medium"),
      };
    })
    .filter(Boolean);
}

function normalizeNarrativeNotes(value, titleFallback) {
  const items = coerceArray(value);
  return items
    .map((item) => {
      if (typeof item === "string") {
        return {
          title: titleFallback,
          summary: item,
          severity: "neutral",
        };
      }

      if (!isObject(item)) {
        return null;
      }

      return {
        title: item.topic || item.title || item.name || titleFallback,
        summary: item.summary || item.text || item.note || item.message || "",
        severity: normalizeSeverity(item.severity || item.status || item.level || "neutral"),
      };
    })
    .filter(Boolean);
}

function normalizeSectionChanges(value) {
  return coerceArray(value)
    .map((item) => {
      if (!isObject(item)) {
        return null;
      }

      const changeScore = clampScore(
        pickFirst(item, ["change_score", "score", "change", "delta", "importance"]) ?? 0,
      );
      const attentionScore = clampScore(
        pickFirst(item, ["attention_score", "attention", "risk_score"]) ?? changeScore,
      );
      const status = normalizeSectionStatus(item.status, attentionScore);

      return {
        name: item.name || item.label || item.title || "Section",
        changeScore,
        attentionScore,
        status,
        summary: item.summary || item.note || item.description || "",
        analysisBasis: item.analysis_basis || item.methodology || "",
        similarity: pickFirst(item, ["similarity"]),
        paragraphOverlap: pickFirst(item, ["paragraph_overlap"]),
        lengthDelta: pickFirst(item, ["length_delta"]),
        evidence: normalizeEvidenceItems(item.evidence || item.excerpts || item.supporting_evidence),
      };
    })
    .filter(Boolean)
    .sort((a, b) => b.changeScore - a.changeScore);
}

function normalizeEvidenceItems(value) {
  return coerceArray(value)
    .map((item) => {
      if (typeof item === "string") {
        return {
          label: "Evidence",
          snippet: item,
        };
      }

      if (!isObject(item)) {
        return null;
      }

      return {
        label: item.label || item.title || item.kind || "Evidence",
        snippet: item.snippet || item.text || item.evidence || item.summary || "",
      };
    })
    .filter((item) => item && item.snippet);
}

function buildFallbackTrends() {
  return TREND_ORDER.map((entry) => ({
    label: entry.label,
    latest: null,
    previous: null,
    delta: null,
    status: "unknown",
    note: "Structured financial trend data is not available in the compare fallback yet.",
    available: false,
  }));
}

function deriveWarningsFromSections(sectionChanges) {
  const keywords = [
    "risk",
    "legal",
    "proceeding",
    "liquidity",
    "cash",
    "debt",
    "demand",
    "layoff",
    "restructuring",
    "headwind",
    "customer",
    "concentration",
  ];

  const items = sectionChanges
    .filter((section) => {
      const text = `${section.name} ${section.summary}`.toLowerCase();
      return keywords.some((keyword) => text.includes(keyword));
    })
    .slice(0, 4)
    .map((section) => ({
      title: section.name,
      summary: section.summary || "This section changed enough to warrant a closer look.",
      severity: severityFromScore(section.changeScore),
    }));

  if (items.length) {
    return items;
  }

  if (!sectionChanges.length) {
    return [];
  }

  return sectionChanges.slice(0, 3).map((section) => ({
    title: section.name,
    summary: section.summary || "This section moved compared with the previous filing.",
    severity: severityFromScore(section.changeScore),
  }));
}

function deriveNarrativeFromSections(sectionChanges, keyword) {
  const items = sectionChanges.filter((section) => section.name.toLowerCase().includes(keyword));
  if (items.length) {
    return items.slice(0, 3).map((section) => ({
      title: section.name,
      summary: section.summary || "The filing includes this section change, but no summary text was returned.",
      severity: severityFromScore(section.changeScore),
    }));
  }

  return [];
}

function renderDashboard(data) {
  companyLabel.textContent = [data.ticker, data.companyName, data.form].filter(Boolean).join(" · ");
  healthSummary.textContent = data.overallHealth.summary;
  healthStatus.textContent = titleCase(data.overallHealth.status);
  healthStatus.className = `status-chip ${toneClass(data.overallHealth.status)}`;

  const healthPercent = Math.round(clampScore(data.overallHealth.score) * 100);
  healthScore.textContent = `${healthPercent}%`;
  healthScoreFill.style.width = `${healthPercent}%`;
  healthMethodology.textContent = data.overallHealth.methodology;
  renderHealthDrivers(data.overallHealth.drivers);

  latestDate.textContent = data.latestDate;
  previousDate.textContent = data.previousDate;
  latestLink.href = data.latestUrl || "#";
  previousLink.href = data.previousUrl || "#";
  latestLink.target = "_blank";
  latestLink.rel = "noreferrer";
  previousLink.target = "_blank";
  previousLink.rel = "noreferrer";

  trendSource.textContent = data.aiAnalysis?.message || data.sourceLabel;
  renderTrendGrid(data.financialTrends);
  renderWarningList(data.warningSigns);
  renderNoteList(data.managementNotes, data.riskNotes);
  renderSectionChanges(data.sectionChanges);

  sectionCount.textContent = `${data.sectionChanges.length} sections`;
  dashboard.classList.remove("is-hidden");
}

function renderHealthDrivers(items) {
  healthDrivers.replaceChildren();

  if (!items.length) {
    healthDrivers.append(createEmptyState("No score drivers were returned yet.", "The score will show its inputs here when the backend supplies them."));
    return;
  }

  items.forEach((item) => healthDrivers.append(createHealthDriver(item)));
}

function createHealthDriver(item) {
  const card = document.createElement("article");
  card.className = `driver-item ${item.impact}`;

  const top = document.createElement("div");
  top.className = "driver-topline";

  const title = document.createElement("h3");
  title.className = "warning-title";
  title.textContent = item.label;

  const impact = document.createElement("span");
  impact.className = `warning-severity ${severityFromImpact(item.impact)}`;
  impact.textContent = titleCase(item.impact);

  top.append(title, impact);

  const summary = document.createElement("p");
  summary.className = "warning-copy";
  summary.textContent = item.summary || "This signal contributed to the health score.";

  card.append(top, summary);

  if (item.evidence) {
    const evidence = document.createElement("p");
    evidence.className = "evidence-snippet";
    evidence.textContent = item.evidence;
    card.append(evidence);
  }

  return card;
}

function renderTrendGrid(items) {
  trendGrid.replaceChildren(...items.map(createTrendCard));
}

function createTrendCard(item) {
  const card = document.createElement("article");
  card.className = `metric-card ${item.available ? "" : "is-empty"}`.trim();
  const chart = metricBars(item);

  const top = document.createElement("div");
  top.className = "metric-topline";

  const label = document.createElement("h3");
  label.className = "metric-label";
  label.textContent = item.label;

  const delta = document.createElement("span");
  delta.className = `metric-delta ${metricDeltaTone(item)}`;
  delta.textContent = formatDelta(item.delta, item.available);

  top.append(label, delta);

  const value = document.createElement("strong");
  value.className = "metric-value";
  value.textContent = formatMetricValue(item.latest, item.available, item.unit);

  const chartBlock = document.createElement("div");
  chartBlock.className = "metric-chart";
  chartBlock.innerHTML = `
    <div class="bar-pair">
      <div class="bar-row">
        <span>Previous</span>
        <div class="bar-track"><div class="bar-fill previous"></div></div>
        <strong></strong>
      </div>
      <div class="bar-row">
        <span>Latest</span>
        <div class="bar-track"><div class="bar-fill latest"></div></div>
        <strong></strong>
      </div>
    </div>
  `;

  chartBlock.querySelector(".bar-fill.previous").style.width = `${chart.previousWidth}%`;
  chartBlock.querySelector(".bar-fill.latest").style.width = `${chart.latestWidth}%`;
  chartBlock.querySelector(".bar-row:first-child strong").textContent = formatMetricValue(
    item.previous,
    item.available,
    item.unit,
  );
  chartBlock.querySelector(".bar-row:last-child strong").textContent = formatMetricValue(
    item.latest,
    item.available,
    item.unit,
  );

  const note = document.createElement("p");
  note.className = "metric-note";
  note.textContent = item.note || (item.available ? "" : "Structured metric data is not available yet.");

  card.append(top, value, chartBlock, note);

  const provenance = buildTrendProvenance(item);
  if (provenance) {
    const source = document.createElement(item.sourceUrl ? "a" : "p");
    source.className = "metric-source";
    source.textContent = provenance;

    if (item.sourceUrl) {
      source.href = item.sourceUrl;
      source.target = "_blank";
      source.rel = "noreferrer";
    }

    card.append(source);
  }

  return card;
}

function buildTrendProvenance(item) {
  const parts = [];

  if (item.sourceLabel) {
    parts.push(item.sourceLabel);
  }

  if (item.sourceNamespace && item.sourceConcept) {
    parts.push(`${item.sourceNamespace}.${item.sourceConcept}`);
  } else if (item.sourceConcept) {
    parts.push(item.sourceConcept);
  }

  if (item.latestPeriodEnd) {
    parts.push(`latest period end ${item.latestPeriodEnd}`);
  }

  if (item.sourceFiled) {
    parts.push(`filed ${item.sourceFiled}`);
  }

  if (item.sourceForm) {
    parts.push(item.sourceForm);
  }

  if (item.sourceEndpointFamily && item.sourceEndpointFamily !== "derived") {
    parts.push(item.sourceEndpointFamily);
  }

  return parts.filter(Boolean).join(" · ");
}

function metricBars(item) {
  const latest = Number(item.latest);
  const previous = Number(item.previous);

  if (!item.available || !Number.isFinite(latest) || !Number.isFinite(previous)) {
    return {
      previousWidth: 18,
      latestWidth: 18,
    };
  }

  const max = Math.max(Math.abs(latest), Math.abs(previous), 1);
  const previousWidth = Math.max(8, Math.round((Math.abs(previous) / max) * 100));
  const latestWidth = Math.max(8, Math.round((Math.abs(latest) / max) * 100));

  return {
    previousWidth,
    latestWidth,
  };
}

function metricDeltaTone(item) {
  const delta = Number(item.delta);
  if (!Number.isFinite(delta)) {
    return toneClass(item.status);
  }

  const lowerIsBetter = String(item.label || "").toLowerCase().includes("debt");
  if (lowerIsBetter) {
    if (delta < 0) {
      return "good";
    }
    if (delta > 0) {
      return "bad";
    }
  }

  return toneClass(item.status);
}

function renderWarningList(items) {
  warningList.replaceChildren();
  if (!items.length) {
    warningList.append(createEmptyState("No warning flags were returned yet.", "Warnings will appear here when the backend supplies them."));
    return;
  }

  items.forEach((item) => warningList.append(createWarningItem(item)));
}

function createWarningItem(item) {
  const card = document.createElement("article");
  card.className = "warning-item";

  const main = document.createElement("div");

  const title = document.createElement("h3");
  title.className = "warning-title";
  title.textContent = item.title;

  const copy = document.createElement("p");
  copy.className = "warning-copy";
  copy.textContent = item.summary || "This warning sign is visible in the filing change set.";

  main.append(title, copy);

  const meta = document.createElement("div");
  meta.className = "warning-meta";

  const severity = document.createElement("span");
  severity.className = `warning-severity ${item.severity}`;
  severity.textContent = titleCase(item.severity);

  meta.append(severity);
  card.append(main, meta);
  return card;
}

function renderNoteList(managementNotes, riskNotes) {
  noteList.replaceChildren();

  const groups = [
    { title: "Management", items: managementNotes },
    { title: "Risk", items: riskNotes },
  ];

  const hasItems = groups.some((group) => group.items.length);
  if (!hasItems) {
    noteList.append(
      createEmptyState(
        "No narrative notes were returned yet.",
        "This area will show management explanation and risk-factor notes when the analysis endpoint is available.",
      ),
    );
    return;
  }

  groups.forEach((group) => {
    if (!group.items.length) {
      return;
    }

    const wrapper = document.createElement("div");
    wrapper.className = "note-list-group";

    const title = document.createElement("p");
    title.className = "note-group-title";
    title.textContent = group.title;

    wrapper.append(title, ...group.items.map(createNoteItem));
    noteList.append(wrapper);
  });
}

function createNoteItem(item) {
  const card = document.createElement("article");
  card.className = "note-item";

  const title = document.createElement("h3");
  title.className = "note-title";
  title.textContent = item.title;

  const copy = document.createElement("p");
  copy.className = "note-copy";
  copy.textContent = item.summary || "No additional narrative was returned.";

  const severity = document.createElement("span");
  severity.className = `warning-severity ${item.severity}`;
  severity.textContent = titleCase(item.severity);

  card.append(title, severity, copy);
  return card;
}

function renderSectionChanges(items) {
  sections.replaceChildren();

  if (!items.length) {
    sections.append(
      createEmptyState(
        "No section changes were returned yet.",
        "This area will fill with the largest narrative shifts from the latest 10-Q.",
      ),
    );
    return;
  }

  items.forEach((item) => sections.append(createSectionItem(item)));
}

function createSectionItem(item) {
  const card = document.createElement("article");
  card.className = "section-item";
  const attentionScore = clampScore(item.attentionScore ?? item.changeScore);

  const top = document.createElement("div");
  top.className = "section-topline";

  const title = document.createElement("h3");
  title.className = "section-title";
  title.textContent = item.name;

  const score = document.createElement("span");
  score.className = "section-score";
  score.textContent = `Attention ${Math.round(attentionScore * 100)}`;

  top.append(title, score);

  const track = document.createElement("div");
  track.className = "section-track";
  track.setAttribute("aria-hidden", "true");

  const fill = document.createElement("div");
  fill.className = "section-fill";
  fill.style.width = `${Math.round(attentionScore * 100)}%`;
  track.append(fill);

  const status = document.createElement("span");
  status.className = `status-chip ${toneClass(item.status)}`;
  status.textContent = titleCase(item.status);

  const summary = document.createElement("p");
  summary.className = "section-summary";
  summary.textContent = item.summary || "This section changed compared with the prior filing.";

  const basis = document.createElement("p");
  basis.className = "section-basis";
  basis.textContent = item.analysisBasis || buildSectionBasis(item);

  card.append(top, track, status, summary, basis);

  if (item.evidence.length) {
    const evidenceList = document.createElement("div");
    evidenceList.className = "evidence-list";
    item.evidence.slice(0, 3).forEach((evidence) => evidenceList.append(createEvidenceItem(evidence)));
    card.append(evidenceList);
  }

  return card;
}

function createEvidenceItem(item) {
  const wrapper = document.createElement("p");
  wrapper.className = "evidence-snippet";

  const label = document.createElement("strong");
  label.textContent = `${item.label}: `;

  const snippet = document.createElement("span");
  snippet.textContent = item.snippet;

  wrapper.append(label, snippet);
  return wrapper;
}

function buildSectionBasis(item) {
  const similarity = Number(item.similarity);
  const overlap = Number(item.paragraphOverlap);
  const lengthDelta = Number(item.lengthDelta);

  if ([similarity, overlap, lengthDelta].every(Number.isFinite)) {
    return `Change basis: ${Math.round(similarity * 100)}% wording similarity, ${Math.round(overlap * 100)}% paragraph overlap, ${Math.round(lengthDelta * 100)}% length movement.`;
  }

  return "Attention combines disclosure movement with the section's current filing context.";
}

function createEmptyState(titleText, copyText) {
  const card = document.createElement("article");
  card.className = "empty-state";

  const title = document.createElement("h3");
  title.className = "warning-title";
  title.textContent = titleText;

  const copy = document.createElement("p");
  copy.className = "warning-copy";
  copy.textContent = copyText;

  card.append(title, copy);
  return card;
}

function setLoading(isLoading, text = "") {
  const button = form.querySelector("button");
  button.disabled = isLoading;
  button.textContent = isLoading ? "Analyzing..." : "Analyze 10-Q";

  if (text) {
    showMessage(text);
  }
}

function startAnalysisStatus(ticker) {
  stopAnalysisStatus(false);

  analysisProgressState = {
    ticker,
    progress: 4,
    stageIndex: 0,
  };

  renderAnalysisStatus({
    title: `Analyzing ${ticker}`,
    copy: ANALYSIS_STAGES[0].copy,
    progress: analysisProgressState.progress,
    activeStage: 0,
  });

  analysisStatus.classList.remove("is-hidden");

  analysisProgressInterval = window.setInterval(() => {
    if (!analysisProgressState) {
      return;
    }

    const nextStageIndex = ANALYSIS_STAGES.findIndex((stage) => analysisProgressState.progress < stage.percent);
    analysisProgressState.stageIndex =
      nextStageIndex === -1 ? ANALYSIS_STAGES.length - 1 : Math.max(0, nextStageIndex);

    const currentStage = ANALYSIS_STAGES[analysisProgressState.stageIndex];
    const maxProgress = currentStage.percent;
    const increment = analysisProgressState.progress < 36 ? 5 : analysisProgressState.progress < 68 ? 3 : 1.5;

    analysisProgressState.progress = Math.min(
      maxProgress,
      Math.max(analysisProgressState.progress + increment, currentStage.percent - 8),
    );

    renderAnalysisStatus({
      title: `Analyzing ${analysisProgressState.ticker}`,
      copy: currentStage.copy,
      progress: analysisProgressState.progress,
      activeStage: analysisProgressState.stageIndex,
    });
  }, 650);
}

function completeAnalysisStatus(data) {
  const aiEnabled = Boolean(data?.aiAnalysis?.enabled);
  const aiUsed = Boolean(data?.aiAnalysis?.used);
  const finalCopy = aiUsed
    ? "SEC data is loaded and the AI review finished."
    : aiEnabled
      ? "SEC data is loaded. AI review was unavailable, so the deterministic analysis is shown."
      : "SEC data is loaded. The dashboard is ready with deterministic filing analysis.";

  renderAnalysisStatus({
    title: "Analysis ready",
    copy: finalCopy,
    progress: 100,
    activeStage: ANALYSIS_STAGES.length - 1,
  });

  analysisProgressHideTimeout = window.setTimeout(() => {
    stopAnalysisStatus();
  }, 900);
}

function stopAnalysisStatus(hide = true) {
  if (analysisProgressInterval) {
    window.clearInterval(analysisProgressInterval);
    analysisProgressInterval = null;
  }

  if (analysisProgressHideTimeout) {
    window.clearTimeout(analysisProgressHideTimeout);
    analysisProgressHideTimeout = null;
  }

  analysisProgressState = null;

  if (hide) {
    analysisStatus.classList.add("is-hidden");
  }
}

function renderAnalysisStatus({ title, copy, progress, activeStage }) {
  analysisStatusTitle.textContent = title;
  analysisStatusCopy.textContent = copy;
  analysisStatusPercent.textContent = `${Math.round(progress)}%`;
  analysisStatusFill.style.width = `${Math.round(progress)}%`;

  analysisStatusSteps.replaceChildren(
    ...ANALYSIS_STAGES.map((stage, index) => createAnalysisStep(stage, index, activeStage, progress)),
  );
}

function createAnalysisStep(stage, index, activeStage, progress) {
  const item = document.createElement("div");
  item.className = "analysis-step";

  if (progress >= stage.percent) {
    item.classList.add("is-complete");
  } else if (index === activeStage) {
    item.classList.add("is-active");
  }

  const label = document.createElement("p");
  label.className = "analysis-step-label";
  label.textContent = stage.label;

  const copy = document.createElement("p");
  copy.className = "analysis-step-copy";
  copy.textContent = stage.copy;

  item.append(label, copy);
  return item;
}

function showMessage(text) {
  message.className = "message";
  message.textContent = text;
}

function showError(text) {
  message.className = "message error";
  message.textContent = text;
}

function defaultHttpError(status) {
  if (status === 404) {
    return "The endpoint was not found.";
  }

  if (status >= 500) {
    return "The server returned an error.";
  }

  return "The request could not be completed.";
}

function hideDashboard() {
  dashboard.classList.add("is-hidden");
}

function normalizeSearchText(value) {
  return String(value || "")
    .trim()
    .toUpperCase()
    .replace(/\s+/g, " ");
}

function isObject(value) {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function coerceArray(value) {
  if (Array.isArray(value)) {
    return value;
  }

  if (value === undefined || value === null) {
    return [];
  }

  return [value];
}

function pickFirst(object, keys) {
  for (const key of keys) {
    const value = object[key];
    if (value !== undefined && value !== null && value !== "") {
      return value;
    }
  }

  return null;
}

function formatMetricValue(value, available, unit = "") {
  if (!available) {
    return "—";
  }

  if (value === null || value === undefined || value === "") {
    return "—";
  }

  if (typeof value === "number") {
    const formatted = Number.isInteger(value)
      ? value.toLocaleString()
      : value.toLocaleString(undefined, { maximumFractionDigits: 1 });

    if (unit === "USD millions") {
      return `$${formatted}M`;
    }

    if (unit === "percent") {
      return `${formatted}%`;
    }

    return formatted;
  }

  if (isObject(value)) {
    if (value.formatted) {
      return String(value.formatted);
    }

    if (value.label) {
      return String(value.label);
    }

    if (value.value !== undefined && value.value !== null) {
      return formatMetricValue(value.value, true, value.unit || unit);
    }
  }

  return String(value);
}

function formatDelta(value, available) {
  if (!available) {
    return "Unavailable";
  }

  if (value === null || value === undefined || value === "") {
    return "No delta";
  }

  if (typeof value === "number") {
    const magnitude = Math.abs(value).toLocaleString(undefined, { maximumFractionDigits: 2 });
    const sign = value > 0 ? "+" : value < 0 ? "-" : "";
    return `${sign}${magnitude}%`;
  }

  return String(value);
}

function clampScore(value) {
  const number = Number(value);
  if (!Number.isFinite(number)) {
    return 0;
  }

  return Math.min(1, Math.max(0, number));
}

function fallbackHealthScore(payload) {
  const sections = Array.isArray(payload?.sections) ? payload.sections : [];
  const topScore = sections.reduce((max, section) => {
    const score = Number(section?.change_score ?? section?.score ?? 0);
    return Number.isFinite(score) ? Math.max(max, score) : max;
  }, 0);

  return clampScore(topScore || 0.42);
}

function deriveHealthStatus(score) {
  if (score >= 0.72) {
    return "strong";
  }

  if (score >= 0.45) {
    return "watch";
  }

  return "stressed";
}

function deriveComparisonStatus(score) {
  if (score >= 0.72) {
    return "bad";
  }

  if (score >= 0.45) {
    return "watch";
  }

  return "good";
}

function normalizeStatus(status, score) {
  const text = String(status || "").toLowerCase();

  if (text.includes("good") || text.includes("improv") || text.includes("healthy") || text.includes("strong")) {
    return "strong";
  }

  if (
    text.includes("steady") ||
    text.includes("warn") ||
    text.includes("watch") ||
    text.includes("moderate") ||
    text.includes("mixed") ||
    text.includes("trend")
  ) {
    return "watch";
  }

  if (
    text.includes("bad") ||
    text.includes("stress") ||
    text.includes("weak") ||
    text.includes("declin") ||
    text.includes("deterior") ||
    text.includes("risk") ||
    text.includes("material")
  ) {
    return "stressed";
  }

  if (typeof score === "number") {
    return deriveHealthStatus(score);
  }

  return "neutral";
}

function normalizeSectionStatus(status, score) {
  const text = String(status || "").toLowerCase();

  if (text.includes("unchanged") || text.includes("stable")) {
    return "good";
  }

  if (text.includes("changed")) {
    return score >= 0.55 ? "bad" : "watch";
  }

  if (text.includes("moderate")) {
    return "watch";
  }

  if (text.includes("material") || text.includes("large") || text.includes("significant")) {
    return "bad";
  }

  return deriveComparisonStatus(score);
}

function normalizeTrendStatus(status) {
  const text = String(status || "").toLowerCase();

  if (text.includes("good") || text.includes("improv") || text.includes("up") || text.includes("better")) {
    return "good";
  }

  if (text.includes("bad") || text.includes("weak") || text.includes("down") || text.includes("worse")) {
    return "bad";
  }

  if (text.includes("watch") || text.includes("mixed") || text.includes("steady") || text.includes("stable")) {
    return "watch";
  }

  return "neutral";
}

function normalizeSeverity(value) {
  const text = String(value || "").toLowerCase();

  if (text.includes("high") || text.includes("critical") || text.includes("severe")) {
    return "high";
  }

  if (text.includes("low") || text.includes("minor")) {
    return "low";
  }

  if (text.includes("medium") || text.includes("moderate") || text.includes("watch")) {
    return "medium";
  }

  return "unknown";
}

function severityFromScore(score) {
  if (score >= 0.72) {
    return "high";
  }

  if (score >= 0.45) {
    return "medium";
  }

  return "low";
}

function toneClass(status) {
  const text = String(status || "").toLowerCase();

  if (text === "good" || text === "strong" || text === "healthy" || text === "improving") {
    return "good";
  }

  if (text === "watch" || text === "steady" || text === "neutral" || text === "stable" || text === "fallback") {
    return "watch";
  }

  if (text === "bad" || text === "stressed" || text === "weak" || text === "weakening" || text === "declining") {
    return "bad";
  }

  return "neutral";
}

function normalizeImpact(value) {
  const text = String(value || "").toLowerCase();

  if (text.includes("positive") || text.includes("up") || text.includes("good")) {
    return "positive";
  }

  if (text.includes("negative") || text.includes("down") || text.includes("bad")) {
    return "negative";
  }

  return "neutral";
}

function severityFromImpact(impact) {
  if (impact === "positive") {
    return "low";
  }

  if (impact === "negative") {
    return "high";
  }

  return "unknown";
}

function titleCase(value) {
  return String(value || "")
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/\b\w/g, (character) => character.toUpperCase()) || "Unknown";
}

function computeDelta(latest, previous) {
  const current = Number(latest);
  const prior = Number(previous);

  if (!Number.isFinite(current) || !Number.isFinite(prior) || prior === 0) {
    return null;
  }

  return ((current - prior) / Math.abs(prior)) * 100;
}
