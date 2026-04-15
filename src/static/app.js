const form = document.querySelector("#compare-form");
const tickerInput = document.querySelector("#ticker");
const message = document.querySelector("#message");
const dashboard = document.querySelector("#dashboard");
const companyLabel = document.querySelector("#company-label");
const healthSummary = document.querySelector("#health-summary");
const healthStatus = document.querySelector("#health-status");
const healthScore = document.querySelector("#health-score");
const healthScoreFill = document.querySelector("#health-score-fill");
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

form.addEventListener("submit", async (event) => {
  event.preventDefault();

  const ticker = tickerInput.value.trim().toUpperCase();
  if (!ticker) {
    showError("Enter a ticker first.");
    return;
  }

  tickerInput.value = ticker;
  await loadDashboard(ticker);
});

async function loadDashboard(ticker) {
  setLoading(true, `Checking the latest 10-Q for ${ticker}...`);

  try {
    const payload = await fetchDashboardPayload(ticker);
    const normalized = normalizePayload(payload);

    if (!normalized) {
      throw new Error("The server returned an unexpected response shape.");
    }

    renderDashboard(normalized);
    showMessage(normalized.message);
  } catch (error) {
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
  };
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
      const status = normalizeSectionStatus(item.status, changeScore);

      return {
        name: item.name || item.label || item.title || "Section",
        changeScore,
        status,
        summary: item.summary || item.note || item.description || "",
      };
    })
    .filter(Boolean)
    .sort((a, b) => b.changeScore - a.changeScore);
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

  latestDate.textContent = data.latestDate;
  previousDate.textContent = data.previousDate;
  latestLink.href = data.latestUrl || "#";
  previousLink.href = data.previousUrl || "#";
  latestLink.target = "_blank";
  latestLink.rel = "noreferrer";
  previousLink.target = "_blank";
  previousLink.rel = "noreferrer";

  trendSource.textContent = data.sourceLabel;
  renderTrendGrid(data.financialTrends);
  renderWarningList(data.warningSigns);
  renderNoteList(data.managementNotes, data.riskNotes);
  renderSectionChanges(data.sectionChanges);

  sectionCount.textContent = `${data.sectionChanges.length} sections`;
  dashboard.classList.remove("is-hidden");
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
  return card;
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

  const top = document.createElement("div");
  top.className = "section-topline";

  const title = document.createElement("h3");
  title.className = "section-title";
  title.textContent = item.name;

  const score = document.createElement("span");
  score.className = "section-score";
  score.textContent = `${Math.round(clampScore(item.changeScore) * 100)}%`;

  top.append(title, score);

  const track = document.createElement("div");
  track.className = "section-track";
  track.setAttribute("aria-hidden", "true");

  const fill = document.createElement("div");
  fill.className = "section-fill";
  fill.style.width = `${Math.round(clampScore(item.changeScore) * 100)}%`;
  track.append(fill);

  const status = document.createElement("span");
  status.className = `status-chip ${toneClass(item.status)}`;
  status.textContent = titleCase(item.status);

  const summary = document.createElement("p");
  summary.className = "section-summary";
  summary.textContent = item.summary || "This section changed compared with the prior filing.";

  card.append(top, track, status, summary);
  return card;
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
    return "good";
  }

  if (score >= 0.45) {
    return "neutral";
  }

  return "bad";
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
    return "good";
  }

  if (
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
    text.includes("weak") ||
    text.includes("declin") ||
    text.includes("deterior") ||
    text.includes("risk") ||
    text.includes("material")
  ) {
    return "bad";
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

  if (text.includes("changed") || text.includes("moderate")) {
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

  if (text === "good" || text === "improving") {
    return "good";
  }

  if (text === "watch" || text === "neutral" || text === "stable" || text === "fallback") {
    return "watch";
  }

  if (text === "bad" || text === "weakening" || text === "declining") {
    return "bad";
  }

  return "neutral";
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
