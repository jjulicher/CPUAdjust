/// Embedded CoolerControl-style dark dashboard UI served at `http://127.0.0.1:11988/`
pub const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>CPUAdjust — Ryzen 7000 SMU & Curve Optimizer Control</title>
<link rel="icon" type="image/png" href="/icon.png">
<style>
  :root {
    --bg-base: #0f1217;
    --bg-sidebar: #151921;
    --bg-card: #1a1f29;
    --bg-card-hover: #202634;
    --bg-input: #11151c;
    --border: #2a3446;
    --text-main: #f0f6fc;
    --text-muted: #9fb0c7;
    --accent-cyan: #38bdf8;
    --accent-blue: #3b82f6;
    --accent-emerald: #10b981;
    --accent-amber: #f59e0b;
    --accent-rose: #f43f5e;
    --accent-purple: #c084fc;
    --gold: #fbbf24;
    --silver: #cbd5e1;
  }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  html {
    font-size: 16px;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    text-rendering: optimizeLegibility;
  }
  body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Inter, Ubuntu, "DejaVu Sans", sans-serif;
    font-size: 1rem;
    line-height: 1.45;
    background: var(--bg-base);
    color: var(--text-main);
    display: flex;
    height: 100vh;
    overflow: hidden;
    user-select: none;
  }
  /* CoolerControl-style Left Sidebar */
  .sidebar {
    width: 310px;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }
  .brand {
    padding: 20px 22px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .brand-icon {
    width: 46px;
    height: 46px;
    border-radius: 10px;
    object-fit: contain;
    display: block;
    filter: drop-shadow(0 4px 10px rgba(0, 0, 0, 0.45));
  }
  .brand-title { font-size: 20px; font-weight: 700; letter-spacing: 0.3px; }
  .brand-sub { font-size: 13px; color: var(--text-muted); margin-top: 2px; }
  .nav-group-label {
    padding: 16px 22px 6px;
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--text-muted);
  }
  .nav-item {
    margin: 2px 12px;
    padding: 10px 14px;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    transition: all 0.15s;
  }
  .nav-item:hover { background: var(--bg-card); color: var(--text-main); }
  .nav-item.active {
    background: rgba(56, 189, 248, 0.14);
    color: var(--accent-cyan);
    border-left: 4px solid var(--accent-cyan);
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-emerald);
    display: inline-block;
  }
  .status-dot.inactive {
    background: var(--accent-rose);
  }
  .sensor-tree {
    flex: 1;
    overflow-y: auto;
    padding: 8px 14px 16px;
  }
  .tree-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 10px;
    border-radius: 6px;
    font-size: 13.5px;
    color: var(--text-muted);
  }
  .tree-row:hover { background: rgba(255,255,255,0.04); }
  .tree-val { font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace; font-size: 13.5px; font-weight: 700; color: var(--text-main); }
  .sidebar-footer {
    padding: 14px 18px;
    border-top: 1px solid var(--border);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-muted);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .dot {
    display: inline-block;
    width: 10px; height: 10px;
    border-radius: 50%;
    background: var(--accent-emerald);
    margin-right: 8px;
    box-shadow: 0 0 8px var(--accent-emerald);
  }
  .dot.offline { background: var(--accent-rose); box-shadow: 0 0 8px var(--accent-rose); }

  /* Main Area */
  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .topbar {
    height: 70px;
    background: var(--bg-sidebar);
    border-bottom: 1px solid var(--border);
    padding: 0 26px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
  }
  .topbar-left { display: flex; align-items: center; gap: 16px; }
  .cpu-title { font-size: 19px; font-weight: 700; }
  .pill {
    font-size: 13px;
    font-weight: 700;
    padding: 5px 12px;
    border-radius: 999px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    color: var(--text-muted);
  }
  .pill.synced {
    background: rgba(16, 185, 129, 0.14);
    border-color: rgba(16, 185, 129, 0.4);
    color: var(--accent-emerald);
  }
  .pill.staged {
    background: rgba(245, 158, 11, 0.16);
    border-color: rgba(245, 158, 11, 0.5);
    color: var(--accent-amber);
  }
  .topbar-actions { display: flex; align-items: center; gap: 10px; }
  .zoom-group {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-right: 6px;
    background: var(--bg-input);
    padding: 3px;
    border-radius: 8px;
    border: 1px solid var(--border);
  }
  .zoom-btn {
    padding: 6px 10px;
    font-size: 13px;
    font-weight: 700;
    border-radius: 5px;
    border: none;
  }
  button {
    background: var(--bg-card);
    color: var(--text-main);
    border: 1px solid var(--border);
    padding: 9px 15px;
    border-radius: 7px;
    font-size: 13.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s;
  }
  button:hover { background: var(--bg-card-hover); border-color: #3b465e; }
  button.primary {
    background: linear-gradient(135deg, #0284c7, #2563eb);
    border-color: #38bdf8;
    color: #fff;
  }
  button.primary:hover { filter: brightness(1.1); }
  button.emerald {
    background: rgba(16, 185, 129, 0.18);
    border-color: var(--accent-emerald);
    color: #34d399;
  }
  button.emerald:hover { background: rgba(16, 185, 129, 0.28); }
  button.active-choice {
    background: rgba(56, 189, 248, 0.20);
    border-color: var(--accent-cyan);
    color: var(--accent-cyan);
  }

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 22px 26px 44px;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }

  /* KPI Gauge Cards */
  .kpi-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
    gap: 16px;
  }
  .kpi-card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }
  .kpi-header {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    color: var(--text-muted);
  }
  .kpi-main {
    font-size: 30px;
    font-weight: 800;
    font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace;
    margin: 8px 0 6px;
  }
  .kpi-sub { font-size: 13.5px; color: var(--text-muted); display: flex; justify-content: space-between; }
  .bar-track {
    height: 7px;
    background: #11151c;
    border-radius: 999px;
    overflow: hidden;
    margin-top: 10px;
  }
  .bar-fill {
    height: 100%;
    border-radius: 999px;
    transition: width 0.3s ease;
  }

  /* Section Card */
  .panel {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 20px 22px;
  }
  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    flex-wrap: wrap;
    gap: 12px;
  }
  .panel-title { font-size: 18px; font-weight: 700; display: flex; align-items: center; gap: 8px; }
  .panel-subtitle { font-size: 14px; color: var(--text-muted); margin-top: 2px; }

  /* Chart */
  .chart-wrap {
    position: relative;
    height: 240px;
    width: 100%;
    background: #12161f;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px;
  }
  canvas#telemetryChart { width: 100%; height: 100%; display: block; }
  .chart-legend {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
    font-size: 14px;
    font-weight: 600;
  }
  .legend-item {
    display: flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
    padding: 5px 10px;
    border-radius: 6px;
    background: rgba(255,255,255,0.04);
  }
  .legend-item.off { opacity: 0.35; }
  .legend-swatch { width: 12px; height: 12px; border-radius: 3px; }

  /* Per-Core Curve Optimizer Grid */
  .bulk-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 14px;
    padding: 14px 16px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 8px;
    margin-bottom: 16px;
  }
  .cores-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
    gap: 14px;
  }
  .core-card {
    background: #141922;
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    transition: border-color 0.15s;
  }
  .core-card.gold { border-left: 4px solid var(--gold); }
  .core-card.silver { border-left: 4px solid var(--silver); }
  .core-card.modified { border-color: var(--accent-amber); }
  .core-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .core-name { font-size: 17px; font-weight: 700; }
  .core-threads { font-size: 13px; color: var(--text-muted); margin-left: 6px; }
  .tier-badge {
    font-size: 12px;
    font-weight: 700;
    padding: 4px 10px;
    border-radius: 999px;
    background: rgba(255,255,255,0.06);
    color: var(--text-muted);
  }
  .tier-badge.gold { background: rgba(251, 191, 36, 0.16); color: var(--gold); border: 1px solid rgba(251, 191, 36, 0.4); }
  .tier-badge.silver { background: rgba(203, 213, 225, 0.16); color: var(--silver); border: 1px solid rgba(203, 213, 225, 0.4); }
  .core-metrics {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    background: var(--bg-input);
    padding: 10px;
    border-radius: 7px;
    font-size: 13px;
  }
  .cm-label { color: var(--text-muted); font-size: 12px; font-weight: 600; }
  .cm-val { font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace; font-size: 14px; font-weight: 700; margin-top: 3px; }
  .co-control-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .step-btn {
    width: 34px;
    height: 34px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 18px;
    font-weight: 700;
  }
  input[type="range"] {
    flex: 1;
    height: 8px;
    accent-color: var(--accent-cyan);
    cursor: pointer;
  }
  .co-readout {
    min-width: 66px;
    text-align: right;
    font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace;
    font-size: 18px;
    font-weight: 800;
    color: var(--accent-cyan);
  }
  .core-foot {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
    color: var(--text-muted);
  }

  /* Limits & Presets */
  .limits-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(290px, 1fr));
    gap: 16px;
  }
  .limit-box {
    background: #141922;
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .preset-card {
    background: #141922;
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 16px;
    cursor: pointer;
    transition: all 0.15s;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 12px;
  }
  .preset-card:hover {
    border-color: var(--accent-cyan);
    background: var(--bg-card-hover);
  }
  .preset-name { font-size: 16px; font-weight: 700; color: var(--accent-cyan); }
  .preset-desc { font-size: 14px; color: var(--text-muted); line-height: 1.45; }

  /* Setup Guide / Instruction Box */
  .guide-box {
    background: #131822;
    border: 1px solid var(--accent-amber);
    border-radius: 10px;
    padding: 20px;
    margin-bottom: 16px;
  }
  .guide-step {
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 14px 16px;
    margin-top: 12px;
  }
  .guide-code {
    display: block;
    font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace;
    font-size: 13.5px;
    color: var(--accent-cyan);
    background: #0b0e13;
    padding: 10px 12px;
    border-radius: 6px;
    margin-top: 8px;
    user-select: text;
  }

  .toast {
    position: fixed;
    bottom: 24px;
    right: 26px;
    padding: 14px 20px;
    border-radius: 8px;
    background: #1e293b;
    border: 1px solid var(--accent-cyan);
    color: #fff;
    font-size: 15px;
    font-weight: 600;
    box-shadow: 0 10px 25px rgba(0,0,0,0.5);
    opacity: 0;
    transform: translateY(10px);
    transition: all 0.2s;
    pointer-events: none;
    z-index: 1000;
  }
  .toast.show { opacity: 1; transform: translateY(0); }
  .toast.error { border-color: var(--accent-rose); background: #31101c; }
</style>
</head>
<body>
  <aside class="sidebar">
    <div class="brand">
      <img class="brand-icon" src="/icon.png" alt="CPUAdjust">
      <div>
        <div class="brand-title">CPUAdjust</div>
        <div class="brand-sub">Ryzen SMU &amp; PBO Control</div>
      </div>
    </div>

    <div class="nav-group-label">Control Views</div>
    <div class="nav-item active" onclick="setView('dashboard', this)">
      <span>1. System Dashboard</span>
      <span class="status-dot" id="dotDashboard"></span>
    </div>
    <div class="nav-item" onclick="setView('limits', this)">
      <span>2. Limits &amp; Presets</span>
      <span class="status-dot" id="dotLimits"></span>
    </div>
    <div class="nav-item" onclick="setView('pstate', this)">
      <span>3. AMD P-State &amp; CPPC</span>
      <span class="status-dot" id="dotPstate"></span>
    </div>
    <div class="nav-item" onclick="setView('smu_adv', this)">
      <span>4. SMU Advanced &amp; SoC</span>
      <span class="status-dot" id="dotSmuAdv"></span>
    </div>
    <div class="nav-item" onclick="setView('igpu', this)">
      <span>5. Raphael iGPU (DPM)</span>
      <span class="status-dot" id="dotIgpu"></span>
    </div>

    <div class="nav-group-label">Live Hardware Tree</div>
    <div class="sensor-tree" id="sensorTree">
      <div class="tree-row"><span>Tctl / Limit</span><span class="tree-val" id="stTctl">-- °C</span></div>
      <div class="tree-row"><span>Tccd1 Die</span><span class="tree-val" id="stTccd1">-- °C</span></div>
      <div class="tree-row"><span>Package PPT</span><span class="tree-val" id="stPpt">-- W</span></div>
      <div class="tree-row"><span>TDC / EDC</span><span class="tree-val" id="stCurrents">-- A</span></div>
      <div class="tree-row"><span>CPU Vcore (SVI3)</span><span class="tree-val" id="stVcore">-- V</span></div>
      <div class="tree-row"><span>SoC VDDCR_SOC</span><span class="tree-val" id="stVsoc">-- V</span></div>
      <div class="tree-row"><span>FCLK / UCLK / MCLK</span><span class="tree-val" id="stClocks">--</span></div>
      <div class="tree-row"><span>AMD P-State EPP</span><span class="tree-val" id="stEpp">--</span></div>
      <div class="tree-row"><span>Raphael iGPU</span><span class="tree-val" id="stIgpu">--</span></div>
      <div class="tree-row"><span>NVMe / DDR5</span><span class="tree-val" id="stStorage">--</span></div>
      <div id="stFans"></div>
    </div>

    <div class="sidebar-footer">
      <div><span class="dot" id="drvDot"></span><span id="drvText">ryzen_smu</span></div>
      <span id="smuVerText">SMU --</span>
    </div>
  </aside>

  <main class="main">
    <header class="topbar">
      <div class="topbar-left">
        <div class="cpu-title" id="cpuHeaderTitle">AMD Ryzen 7 7700X <span style="color:var(--text-muted);font-weight:500;font-size:15px;" id="boardHeaderSub">(Raphael AM5)</span></div>
        <span class="pill synced" id="syncBadge">SMU Synced</span>
      </div>
      <div class="topbar-actions">
        <div class="zoom-group" title="Adjust UI &amp; Font Scale (or press +/- keys)">
          <button class="zoom-btn" onclick="adjustUiZoom(-0.1)">A-</button>
          <button class="zoom-btn" id="zoomPctBtn" onclick="resetUiZoom()" style="color:var(--accent-cyan);min-width:56px;">100%</button>
          <button class="zoom-btn" onclick="adjustUiZoom(0.1)">A+</button>
        </div>
        <button id="btnToggleGuide" onclick="toggleSetupGuide()" title="Show/hide subsystem enablement instructions">? Setup Guide</button>
        <button onclick="reloadFromSmu()" title="Discard staged edits and reload live SMU state">Reload Live</button>
        <button class="emerald" onclick="applyAll(false)" title="Write staged Curve Optimizer and Thermal/Power limits to SMU registers immediately">Apply Live</button>
        <button class="primary" onclick="applyAll(true)" title="Apply to SMU and persist to config.toml for boot restoration">Apply &amp; Save Config</button>
      </div>
    </header>

    <div class="content">
      <!-- KPI Row -->
      <section class="kpi-grid" id="secKpi">
        <div class="kpi-card">
          <div class="kpi-header"><span>CPU Temp (Tctl)</span><span id="kpiTctlLim">Max 90°C</span></div>
          <div class="kpi-main" id="kpiTctl" style="color:var(--accent-amber)">--.- °C</div>
          <div class="kpi-sub"><span>CCD1: <b id="kpiTccd1">--.- °C</b></span><span id="kpiHeadroom">--°C headroom</span></div>
          <div class="bar-track"><div class="bar-fill" id="barTctl" style="width:0%;background:var(--accent-amber)"></div></div>
        </div>

        <div class="kpi-card">
          <div class="kpi-header"><span>Package Power (PPT)</span><span id="kpiPptLim">Limit -- W</span></div>
          <div class="kpi-main" id="kpiPpt" style="color:var(--accent-cyan)">--.- W</div>
          <div class="kpi-sub"><span>TDC: <b id="kpiTdc">-- A</b></span><span>EDC: <b id="kpiEdc">-- A</b></span></div>
          <div class="bar-track"><div class="bar-fill" id="barPpt" style="width:0%;background:var(--accent-cyan)"></div></div>
        </div>

        <div class="kpi-card">
          <div class="kpi-header"><span>Peak Core Boost</span><span id="kpiAvgFreq">Avg -- MHz</span></div>
          <div class="kpi-main" id="kpiPeakFreq" style="color:var(--accent-emerald)">---- MHz</div>
          <div class="kpi-sub"><span>Vcore: <b id="kpiVcore">-.--- V</b></span><span>SoC: <b id="kpiVsoc">-.--- V</b></span></div>
          <div class="bar-track"><div class="bar-fill" id="barFreq" style="width:0%;background:var(--accent-emerald)"></div></div>
        </div>

        <div class="kpi-card">
          <div class="kpi-header"><span>Active Curve Optimizer</span><span id="kpiCoreCountBadge">DLDO PSM</span></div>
          <div class="kpi-main" id="kpiCoSummary" style="color:var(--accent-purple)">All -10</div>
          <div class="kpi-sub"><span id="kpiCoMv">~ -35 mV undervolt</span><span id="kpiPoll">Poll 60s</span></div>
          <div class="bar-track"><div class="bar-fill" id="barCo" style="width:33%;background:var(--accent-purple)"></div></div>
        </div>
      </section>

      <!-- Setup Guide / Instruction Overlay (when active or subsystem inactive) -->
      <section class="guide-box" id="secSetupGuide" style="display:none;"></section>

      <!-- Live Telemetry Chart -->
      <section class="panel" id="secChart">
        <div class="panel-header">
          <div>
            <div class="panel-title">Real-Time Telemetry Graph</div>
            <div class="panel-subtitle">1.5s rolling hardware history from SMU PM Table (0x540104) &amp; k10temp/nct6799</div>
          </div>
          <div class="chart-legend">
            <div class="legend-item" onclick="toggleSeries('tctl', this)"><span class="legend-swatch" style="background:#f59e0b"></span>Tctl (°C)</div>
            <div class="legend-item" onclick="toggleSeries('tccd1', this)"><span class="legend-swatch" style="background:#fb7185"></span>Tccd1 (°C)</div>
            <div class="legend-item" onclick="toggleSeries('ppt', this)"><span class="legend-swatch" style="background:#38bdf8"></span>Package PPT (W)</div>
            <div class="legend-item" onclick="toggleSeries('freq', this)"><span class="legend-swatch" style="background:#10b981"></span>Peak Clock (GHz &times;10)</div>
          </div>
        </div>
        <div class="chart-wrap">
          <canvas id="telemetryChart"></canvas>
        </div>
      </section>

      <!-- Per-Core Curve Optimizer -->
      <section class="panel" id="secCo">
        <div class="panel-header">
          <div>
            <div class="panel-title" id="coSectionTitle">Per-Core PBO Curve Optimizer (RSMU 0x06 / MP1 0x35 / Readback 0xD5)</div>
            <div class="panel-subtitle">Negative values undervolt (~3–5 mV per count). Gold #1 cores boost highest (~5.5 GHz) and prefer conservative offsets (-5 to -12).</div>
          </div>
        </div>

        <div class="bulk-bar">
          <div style="display:flex;align-items:center;gap:12px;flex-wrap:wrap;">
            <span style="font-size:15px;font-weight:700;" id="quickSetAllLabel">Quick Set All Cores:</span>
            <button class="step-btn" onclick="stepAllCores(-1)">-</button>
            <input type="range" id="allCoreSlider" min="-50" max="10" value="-10" style="width:180px" oninput="setAllCoresStaged(parseInt(this.value))">
            <button class="step-btn" onclick="stepAllCores(1)">+</button>
            <span class="co-readout" id="allCoreLabel">-10</span>
          </div>
          <div style="display:flex;gap:8px;flex-wrap:wrap;">
            <button onclick="stagePresetOffsets([-10,-10,-10,-10,-10,-10,-10,-10])">All -10 (Validated)</button>
            <button onclick="stagePresetOffsets([-12,-15,-15,-15,-10,-15,-10,-15])">CPPC Tiered (-10 / -12 / -15)</button>
            <button onclick="stagePresetOffsets([0,0,0,0,0,0,0,0])">Reset 0 (Stock)</button>
          </div>
        </div>

        <div class="cores-grid" id="coresGrid"></div>
      </section>

      <!-- Thermal & PBO Power Limits + Presets -->
      <section class="panel" id="secLimits" style="display:none;">
        <div class="panel-header">
          <div>
            <div class="panel-title">Thermal Limit (TctlMax) &amp; PBO Power Limits (PPT / TDC / EDC)</div>
            <div class="panel-subtitle">Real-time SMU limits applied via RSMU (0x56–0x59) &amp; MP1 (0x3C–0x3F). Uncheck overrides to use BIOS defaults.</div>
          </div>
        </div>

        <div class="limits-grid">
          <div class="limit-box">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="font-size:15px;font-weight:700;">TctlMax Thermal Limit (°C)</span>
              <span class="co-readout" id="lblTctlMax" style="color:var(--accent-amber)">90 °C</span>
            </div>
            <input type="range" id="rngTctlMax" min="45" max="95" value="90" oninput="onLimitEdit()">
            <div style="display:flex;justify-content:space-between;font-size:13px;color:var(--text-muted);">
              <span>Range: 45°C – 95°C (Stock: 95°C)</span>
              <span id="liveTctlLimit">Live SMU: -- °C</span>
            </div>
          </div>

          <div class="limit-box">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="font-size:15px;font-weight:700;">PPT Fast Power Limit (W)</span>
              <span class="co-readout" id="lblPpt">BIOS</span>
            </div>
            <input type="range" id="rngPpt" min="35" max="230" value="142" oninput="onLimitEdit()">
            <div style="display:flex;justify-content:space-between;align-items:center;font-size:13px;color:var(--text-muted);">
              <label style="cursor:pointer;display:flex;align-items:center;gap:6px;">
                <input type="checkbox" id="chkPpt" onchange="onLimitEdit()"> Override PPT (35–230W)
              </label>
              <span id="livePptLimit">Live SMU: -- W</span>
            </div>
          </div>

          <div class="limit-box">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="font-size:15px;font-weight:700;">TDC VDD Current Limit (A)</span>
              <span class="co-readout" id="lblTdc">BIOS</span>
            </div>
            <input type="range" id="rngTdc" min="20" max="180" value="110" oninput="onLimitEdit()">
            <div style="display:flex;justify-content:space-between;align-items:center;font-size:13px;color:var(--text-muted);">
              <label style="cursor:pointer;display:flex;align-items:center;gap:6px;">
                <input type="checkbox" id="chkTdc" onchange="onLimitEdit()"> Override TDC (20–180A)
              </label>
              <span id="liveTdcLimit">Live SMU: -- A</span>
            </div>
          </div>

          <div class="limit-box">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="font-size:15px;font-weight:700;">EDC VDD Peak Current (A)</span>
              <span class="co-readout" id="lblEdc">BIOS</span>
            </div>
            <input type="range" id="rngEdc" min="30" max="250" value="170" oninput="onLimitEdit()">
            <div style="display:flex;justify-content:space-between;align-items:center;font-size:13px;color:var(--text-muted);">
              <label style="cursor:pointer;display:flex;align-items:center;gap:6px;">
                <input type="checkbox" id="chkEdc" onchange="onLimitEdit()"> Override EDC (30–250A)
              </label>
              <span id="liveEdcLimit">Live SMU: -- A</span>
            </div>
          </div>
        </div>

        <div class="panel-header" style="margin-top:26px;">
          <div>
            <div class="panel-title">Hardware-Validated Profiles &amp; Presets</div>
            <div class="panel-subtitle">Click any preset to immediately apply to SMU and save to config.toml</div>
          </div>
        </div>
        <div class="limits-grid">
          <div class="preset-card" onclick="applyPreset('daily')">
            <div>
              <div class="preset-name">Daily Validated (90°C / -10 All-Core)</div>
              <div class="preset-desc" style="margin-top:6px;">Overnight stability-validated baseline. Trims ~35–45mV across all cores and caps Tctl at 90°C.</div>
            </div>
            <button class="emerald" style="align-self:flex-start;">Activate Daily Profile</button>
          </div>

          <div class="preset-card" onclick="applyPreset('cppc_tiered')">
            <div>
              <div class="preset-name">CPPC Preferred-Core Tiered Curve</div>
              <div class="preset-desc" style="margin-top:6px;">Gold #1 cores at -10, Silver #3 core at -12, and remaining cores at -15 with 90°C TctlMax.</div>
            </div>
            <button class="primary" style="align-self:flex-start;">Activate CPPC Tiered</button>
          </div>

          <div class="preset-card" onclick="applyPreset('eco_88w')">
            <div>
              <div class="preset-name">Eco 65W-Class Mode (88W PPT / 85°C)</div>
              <div class="preset-desc" style="margin-top:6px;">Caps Package Power to 88W PPT, 75A TDC, 150A EDC, and 85°C TctlMax with -10 all-core CO.</div>
            </div>
            <button style="align-self:flex-start;">Activate 88W Eco Mode</button>
          </div>

          <div class="preset-card" onclick="applyPreset('stock')">
            <div>
              <div class="preset-name">AMD Factory Stock (95°C / 0 CO)</div>
              <div class="preset-desc" style="margin-top:6px;">Restores factory limits: 95°C TctlMax, 142W PPT, 110A TDC, 170A EDC, and 0 CO offset on all cores.</div>
            </div>
            <button style="align-self:flex-start;">Restore Stock Defaults</button>
          </div>
        </div>
      </section>

      <!-- 3. AMD P-State & CPPC Sub-Panel -->
      <section class="panel" id="secPstate" style="display:none;">
        <div class="panel-header">
          <div>
            <div class="panel-title">AMD P-State &amp; CPPC Hardware Controls</div>
            <div class="panel-subtitle" id="pstateSubtitle">Driver: -- | Energy Performance Preference (EPP), Scaling Governor &amp; Frequency Caps</div>
          </div>
        </div>

        <div class="limits-grid">
          <div class="limit-box">
            <div style="font-size:15px;font-weight:700;">Energy Performance Preference (EPP)</div>
            <div style="font-size:13px;color:var(--text-muted);">Hardware autonomous CPPC voltage/frequency bias across all cores:</div>
            <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:4px;" id="eppButtons">
              <button onclick="postPstate({epp:'performance',save:true})" id="btnEpp_performance">performance</button>
              <button onclick="postPstate({epp:'balance_performance',save:true})" id="btnEpp_balance_performance">balance_performance</button>
              <button onclick="postPstate({epp:'balance_power',save:true})" id="btnEpp_balance_power">balance_power</button>
              <button onclick="postPstate({epp:'power',save:true})" id="btnEpp_power">power</button>
            </div>
          </div>

          <div class="limit-box">
            <div style="font-size:15px;font-weight:700;">CPUFreq Scaling Governor &amp; Boost</div>
            <div style="font-size:13px;color:var(--text-muted);">Toggle scaling governor, Core Performance Boost (CPB), and Dynamic EPP:</div>
            <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:4px;">
              <button onclick="postPstate({governor:'powersave',save:true})" id="btnGov_powersave">Gov: powersave</button>
              <button onclick="postPstate({governor:'performance',save:true})" id="btnGov_performance">Gov: performance</button>
              <button onclick="togglePstateBoost()" id="btnPstateBoost">CPB Boost: ON</button>
              <button onclick="togglePstateDynEpp()" id="btnPstateDynEpp">Dynamic EPP: OFF</button>
            </div>
          </div>

          <div class="limit-box">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="font-size:15px;font-weight:700;">Max Scaling Frequency Cap</span>
              <span class="co-readout" id="lblPstateMaxFreq">5535 MHz</span>
            </div>
            <input type="range" id="rngPstateMaxFreq" min="3000" max="5550" step="50" value="5535" onchange="postPstate({scaling_max_freq_mhz:parseInt(this.value),save:true})" oninput="document.getElementById('lblPstateMaxFreq').textContent=this.value+' MHz'">
            <div style="font-size:13px;color:var(--text-muted);" id="subPstateMaxFreq">Hardware range: 545 – 5535 MHz</div>
          </div>

          <div class="limit-box">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="font-size:15px;font-weight:700;">Min Scaling Frequency Floor</span>
              <span class="co-readout" id="lblPstateMinFreq">545 MHz</span>
            </div>
            <input type="range" id="rngPstateMinFreq" min="545" max="4500" step="50" value="545" onchange="postPstate({scaling_min_freq_mhz:parseInt(this.value),save:true})" oninput="document.getElementById('lblPstateMinFreq').textContent=this.value+' MHz'">
            <div style="font-size:13px;color:var(--text-muted);">Raise floor for low-latency workloads or keep 545 MHz for deep C-state idle</div>
          </div>
        </div>
      </section>

      <!-- 4. SMU Advanced & SoC Sub-Panel -->
      <section class="panel" id="secSmuAdv" style="display:none;">
        <div class="panel-header">
          <div>
            <div class="panel-title">AMD SMU Advanced &amp; SoC / DDR5 Controls</div>
            <div class="panel-subtitle" id="smuAdvSubtitle">RSMU iGPU Curve Optimizer (0x08/0xD6), SoC Current Limits (0x6B/0x6C), FIT Scalar (0x5E), PROCHOT Ramp (0x5A) &amp; Coupled DDR5 Telemetry</div>
          </div>
        </div>

        <div class="limits-grid">
          <div class="limit-box">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="font-size:15px;font-weight:700;">iGPU Graphics Curve Optimizer (0x08 / 0xD6)</span>
              <span class="co-readout" id="lblGfxCo">0</span>
            </div>
            <input type="range" id="rngGfxCo" min="-30" max="10" value="0" onchange="postSmuAdv({gfx_co_margin:parseInt(this.value),save:true})" oninput="document.getElementById('lblGfxCo').textContent=(this.value>0?'+'+this.value:this.value)">
            <div style="font-size:13px;color:var(--text-muted);">DLDO PSM voltage margin for integrated RDNA2 Graphics (-30 to +10)</div>
          </div>

          <div class="limit-box">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="font-size:15px;font-weight:700;">TDC_SOC Thermal Current Limit (0x6B)</span>
              <span class="co-readout" id="lblTdcSoc">-- A</span>
            </div>
            <input type="range" id="rngTdcSoc" min="10" max="60" value="30" onchange="postSmuAdv({tdc_soc_limit_a:parseInt(this.value),save:true})" oninput="document.getElementById('lblTdcSoc').textContent=this.value+' A'">
            <div style="font-size:13px;color:var(--text-muted);" id="subTdcSoc">Configured SoC TDC: 30 A</div>
          </div>

          <div class="limit-box">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="font-size:15px;font-weight:700;">EDC_SOC Peak Current Limit (0x6C)</span>
              <span class="co-readout" id="lblEdcSoc">-- A</span>
            </div>
            <input type="range" id="rngEdcSoc" min="15" max="80" value="45" onchange="postSmuAdv({edc_soc_limit_a:parseInt(this.value),save:true})" oninput="document.getElementById('lblEdcSoc').textContent=this.value+' A'">
            <div style="font-size:13px;color:var(--text-muted);" id="subEdcSoc">Configured SoC EDC: 45 A</div>
          </div>

          <div class="limit-box">
            <div style="font-size:15px;font-weight:700;">PBO FIT Reliability Scalar (0x5E / 0x6D)</div>
            <div style="font-size:13px;color:var(--text-muted);">Multiplies Silicon Fitness voltage/reliability ceiling (1x = safest stock FIT):</div>
            <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:4px;">
              <button onclick="postSmuAdv({pbo_scalar:1,save:true})" id="btnScalar_1">1x</button>
              <button onclick="postSmuAdv({pbo_scalar:2,save:true})" id="btnScalar_2">2x</button>
              <button onclick="postSmuAdv({pbo_scalar:3,save:true})" id="btnScalar_3">3x</button>
              <button onclick="postSmuAdv({pbo_scalar:5,save:true})" id="btnScalar_5">5x</button>
              <button onclick="postSmuAdv({pbo_scalar:7,save:true})" id="btnScalar_7">7x</button>
              <button onclick="postSmuAdv({pbo_scalar:10,save:true})" id="btnScalar_10">10x</button>
            </div>
          </div>

          <div class="limit-box">
            <div style="font-size:15px;font-weight:700;">PROCHOT Deassertion Ramp Time (0x5A)</div>
            <div style="font-size:13px;color:var(--text-muted);">Controls boost recovery speed after thermal throttling (1–100 ms):</div>
            <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:4px;">
              <button onclick="postSmuAdv({prochot_ramp_ms:2,save:true})" id="btnRamp_2">2 ms (Fast)</button>
              <button onclick="postSmuAdv({prochot_ramp_ms:10,save:true})" id="btnRamp_10">10 ms</button>
              <button onclick="postSmuAdv({prochot_ramp_ms:20,save:true})" id="btnRamp_20">20 ms (Stock)</button>
              <button onclick="postSmuAdv({prochot_ramp_ms:50,save:true})" id="btnRamp_50">50 ms (Gradual)</button>
            </div>
          </div>

          <div class="limit-box">
            <div style="font-size:15px;font-weight:700;">Infinity Fabric &amp; DDR5 UMC Telemetry</div>
            <div style="font-size:14px;color:var(--text-muted);line-height:1.7;" id="smuAdvFabricStats">Loading...</div>
          </div>
        </div>
      </section>

      <!-- 5. Raphael iGPU (DPM) Sub-Panel -->
      <section class="panel" id="secIgpu" style="display:none;">
        <div class="panel-header">
          <div>
            <div class="panel-title">AMD Raphael RDNA2 iGPU DPM Power Controls</div>
            <div class="panel-subtitle" id="igpuSubtitle">PCI 1002:164e | Dynamic Power Management (power_dpm_force_performance_level)</div>
          </div>
        </div>

        <div class="limit-box" style="margin-bottom:16px;">
          <div style="font-size:15px;font-weight:700;">DPM Force Performance Level</div>
          <div style="font-size:13px;color:var(--text-muted);">Select amdgpu DPM policy (`auto` recommended, `low` locks 400MHz to save package power, `high`/`profile_peak` locks 2200MHz):</div>
          <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:6px;">
            <button onclick="postIgpuDpm('auto')" id="btnDpm_auto">auto</button>
            <button onclick="postIgpuDpm('low')" id="btnDpm_low">low (400 MHz)</button>
            <button onclick="postIgpuDpm('high')" id="btnDpm_high">high (2200 MHz)</button>
            <button onclick="postIgpuDpm('profile_standard')" id="btnDpm_profile_standard">profile_standard</button>
            <button onclick="postIgpuDpm('profile_min_sclk')" id="btnDpm_profile_min_sclk">profile_min_sclk</button>
            <button onclick="postIgpuDpm('profile_min_mclk')" id="btnDpm_profile_min_mclk">profile_min_mclk</button>
            <button onclick="postIgpuDpm('profile_peak')" id="btnDpm_profile_peak">profile_peak</button>
          </div>
        </div>

        <div class="limits-grid">
          <div class="limit-box">
            <div style="font-size:15px;font-weight:700;">iGPU Core &amp; Telemetry</div>
            <div style="font-size:14px;color:var(--text-muted);line-height:1.7;" id="igpuLiveStats">Loading...</div>
          </div>
          <div class="limit-box">
            <div style="font-size:15px;font-weight:700;">SCLK / MCLK DPM States &amp; Memory</div>
            <div style="font-size:14px;color:var(--text-muted);line-height:1.7;" id="igpuDpmStates">Loading...</div>
          </div>
        </div>
      </section>
    </div>
  </main>

  <div class="toast" id="toast"></div>

<script>
let lastSnap = null;
let currentView = 'dashboard';
let showSetupGuide = false;
let stagedOffsets = [-10, -10, -10, -10, -10, -10, -10, -10];
let stagedLimits = { tctl_max_c: 90, ppt_limit_w: null, tdc_limit_a: null, edc_limit_a: null };
let hasStagedEdits = false;
let seriesVis = { tctl: true, tccd1: true, ppt: true, freq: true };

const SETUP_GUIDES = {
  dashboard: {
    title: 'AMD Ryzen SMU Driver (ryzen_smu)',
    summary: 'The ryzen_smu kernel module provides direct RSMU (0x56–0x59, 0x06, 0xD5) and MP1 mailbox access along with the 3.7 KB PM Table.',
    steps: [
      { title: 'Build and Load the Vendored ryzen_smu v0.1.7 Driver', desc: 'Compile against running kernel headers and load with insmod:', cmd: 'cd /opt/projects/CPUAdjust/driver/ryzen_smu && make && sudo insmod ryzen_smu.ko' },
      { title: 'Verify Sysfs Mailbox & PM Table', desc: 'Confirm /sys/kernel/ryzen_smu_drv/ exposes version, smu_args, rsmu_cmd, and pm_table:', cmd: 'ls -la /sys/kernel/ryzen_smu_drv/' }
    ]
  },
  limits: {
    title: 'AMD PBO Power & Thermal Limits (ryzen_smu)',
    summary: 'PPT, TDC, EDC, and TctlMax limits require the ryzen_smu kernel module and root privileges (or cpu-adjust.service).',
    steps: [
      { title: 'Enable the CPUAdjust Systemd Service', desc: 'Start cpu-adjust.service so non-root GUI/Web sessions can apply limits cleanly:', cmd: 'sudo systemctl enable --now cpu-adjust.service' }
    ]
  },
  pstate: {
    title: 'AMD P-State & CPPC Driver (/sys/devices/system/cpu/amd_pstate)',
    summary: 'Provides autonomous CPPC hardware frequency scaling, Energy Performance Preference (EPP), and Preferred Core ranking.',
    steps: [
      { title: 'Enable CPPC in UEFI/BIOS', desc: 'Ensure AMD CBS -> NBIO Common Options -> SMU Common Options -> CPPC and CPPC Preferred Cores are Enabled.', cmd: 'grep -H . /sys/devices/system/cpu/amd_pstate/* /sys/devices/system/cpu/cpufreq/policy0/scaling_driver' },
      { title: 'Enable Active Mode via Kernel Parameter', desc: 'Set amd_pstate=active in /etc/default/grub for hardware EPP control:', cmd: 'sudo sed -i \'s/GRUB_CMDLINE_LINUX_DEFAULT="/GRUB_CMDLINE_LINUX_DEFAULT="amd_pstate=active /\' /etc/default/grub && sudo update-grub' }
    ]
  },
  smu_adv: {
    title: 'AMD SMU Advanced & SoC Controls (/sys/kernel/ryzen_smu_drv)',
    summary: 'Exposes RSMU iGPU Curve Optimizer (0x08/0xD6), SoC TDC/EDC current limits (0x6B/0x6C), PBO FIT Scalar (0x5E/0x6D), PROCHOT Ramp (0x5A), and DDR5 UMC telemetry.',
    steps: [
      { title: 'Load ryzen_smu Driver', desc: 'Ensure ryzen_smu.ko is loaded and cpu-adjust.service is running:', cmd: 'sudo systemctl restart cpu-adjust.service' }
    ]
  },
  igpu: {
    title: 'AMD Raphael RDNA2 iGPU DPM (/sys/class/drm/card*/device)',
    summary: 'Provides Dynamic Power Management (power_dpm_force_performance_level) and live SCLK/MCLK/temp/power readouts via amdgpu.',
    steps: [
      { title: 'Enable Integrated Graphics in UEFI/BIOS', desc: 'Set Internal Graphics to Enabled in BIOS and ensure amdgpu is loaded:', cmd: 'ls -la /sys/class/drm/card*/device/power_dpm_force_performance_level' }
    ]
  }
};

function showToast(msg, isErr = false) {
  const el = document.getElementById('toast');
  el.textContent = msg;
  el.className = 'toast show' + (isErr ? ' error' : '');
  clearTimeout(el._timer);
  el._timer = setTimeout(() => { el.className = 'toast'; }, 3200);
}

function setView(view, navEl) {
  currentView = view;
  showSetupGuide = false;
  document.querySelectorAll('.nav-item').forEach(n => n.classList.remove('active'));
  if (navEl) navEl.classList.add('active');
  refreshViewVisibility();
}

function toggleSetupGuide() {
  showSetupGuide = !showSetupGuide;
  refreshViewVisibility();
}

function isCurrentSubsystemActive() {
  if (!lastSnap) return true;
  if (currentView === 'pstate') return !!(lastSnap.pstate && lastSnap.pstate.active);
  if (currentView === 'smu_adv') return !!(lastSnap.smu_adv && lastSnap.smu_adv.active);
  if (currentView === 'igpu') return !!(lastSnap.igpu && lastSnap.igpu.active);
  return !!lastSnap.drv_loaded;
}

function refreshViewVisibility() {
  const active = isCurrentSubsystemActive();
  const displayGuide = showSetupGuide || !active;
  const guideEl = document.getElementById('secSetupGuide');
  const guideBtn = document.getElementById('btnToggleGuide');
  if (guideBtn) {
    guideBtn.className = displayGuide ? 'active-choice' : '';
  }

  if (displayGuide) {
    const g = SETUP_GUIDES[currentView] || SETUP_GUIDES.dashboard;
    guideEl.style.display = 'block';
    guideEl.innerHTML = `
      <div style="display:flex;justify-content:space-between;align-items:center;flex-wrap:wrap;gap:10px;">
        <div style="font-size:17px;font-weight:700;color:var(--accent-cyan);">${g.title} — Setup &amp; Enablement Guide</div>
        <span class="pill ${active ? 'synced' : 'staged'}">${active ? 'SUBSYSTEM ACTIVE' : 'INACTIVE / NOT INSTALLED'}</span>
      </div>
      <div style="font-size:14px;color:var(--text-muted);margin-top:6px;">${g.summary}</div>
      ${g.steps.map((s, i) => `
        <div class="guide-step">
          <div style="font-size:14.5px;font-weight:700;">Step ${i+1}: ${s.title}</div>
          <div style="font-size:13.5px;color:var(--text-muted);margin-top:3px;">${s.desc}</div>
          <code class="guide-code">$ ${s.cmd}</code>
        </div>
      `).join('')}
    `;
  } else {
    guideEl.style.display = 'none';
  }

  // Show the active sub-panel controls ONLY when the setup/instruction page is not replacing it
  const showPanel = !displayGuide;
  document.getElementById('secChart').style.display = (showPanel && currentView === 'dashboard') ? 'block' : 'none';
  document.getElementById('secCo').style.display = (showPanel && currentView === 'dashboard') ? 'block' : 'none';
  document.getElementById('secLimits').style.display = (showPanel && currentView === 'limits') ? 'block' : 'none';
  document.getElementById('secPstate').style.display = (showPanel && currentView === 'pstate') ? 'block' : 'none';
  document.getElementById('secSmuAdv').style.display = (showPanel && currentView === 'smu_adv') ? 'block' : 'none';
  document.getElementById('secIgpu').style.display = (showPanel && currentView === 'igpu') ? 'block' : 'none';
}

function updateSyncBadge() {
  const badge = document.getElementById('syncBadge');
  if (hasStagedEdits) {
    badge.textContent = 'Unsaved Staged Edits';
    badge.className = 'pill staged';
  } else {
    badge.textContent = 'SMU Synced';
    badge.className = 'pill synced';
  }
}

function renderCoresGrid(cores) {
  const grid = document.getElementById('coresGrid');
  if (grid.children.length !== cores.length) {
    grid.innerHTML = '';
    cores.forEach(c => {
      const isGold = c.cppc_ranking >= 196;
      const isSilver = c.cppc_ranking >= 190 && c.cppc_ranking < 196;
      const tierCls = isGold ? 'gold' : (isSilver ? 'silver' : '');
      const card = document.createElement('div');
      card.className = `core-card ${tierCls}`;
      card.id = `coreCard_${c.core_id}`;
      card.innerHTML = `
        <div class="core-top">
          <div>
            <span class="core-name">Core ${c.core_id}</span>
            <span class="core-threads">(cpu ${c.threads})</span>
          </div>
          <span class="tier-badge ${tierCls}">CPPC ${c.cppc_ranking} &bull; ${c.tier}</span>
        </div>
        <div class="core-metrics">
          <div><div class="cm-label">Clock</div><div class="cm-val" id="cmFreq_${c.core_id}">-- MHz</div></div>
          <div><div class="cm-label">Temp</div><div class="cm-val" id="cmTemp_${c.core_id}">-- °C</div></div>
          <div><div class="cm-label">VID</div><div class="cm-val" id="cmVid_${c.core_id}">-- V</div></div>
          <div><div class="cm-label">Power</div><div class="cm-val" id="cmPwr_${c.core_id}">-- W</div></div>
        </div>
        <div class="co-control-row">
          <button class="step-btn" onclick="stepCore(${c.core_id}, -1)">-</button>
          <input type="range" id="coRng_${c.core_id}" min="-50" max="10" value="${stagedOffsets[c.core_id] || 0}" oninput="onCoreSlider(${c.core_id}, parseInt(this.value))">
          <button class="step-btn" onclick="stepCore(${c.core_id}, 1)">+</button>
          <span class="co-readout" id="coVal_${c.core_id}">${stagedOffsets[c.core_id] || 0}</span>
        </div>
        <div class="core-foot">
          <span id="coLive_${c.core_id}">Live SMU: ${c.live_co} | Saved: ${c.saved_co}</span>
          <span>Rec: ${c.recommended_co}</span>
        </div>
      `;
      grid.appendChild(card);
    });
  }

  cores.forEach(c => {
    const i = c.core_id;
    document.getElementById(`cmFreq_${i}`).textContent = `${c.cur_freq_mhz} MHz`;
    document.getElementById(`cmTemp_${i}`).textContent = `${c.temp_c.toFixed(1)} °C`;
    document.getElementById(`cmVid_${i}`).textContent = `${c.voltage_v.toFixed(3)} V`;
    document.getElementById(`cmPwr_${i}`).textContent = `${c.power_w.toFixed(1)} W`;
    const fmt = v => (v > 0 ? `+${v}` : `${v}`);
    document.getElementById(`coLive_${i}`).textContent = `Live SMU: ${fmt(c.live_co)} | Config: ${fmt(c.saved_co)}`;
    const card = document.getElementById(`coreCard_${i}`);
    const rng = document.getElementById(`coRng_${i}`);
    const valEl = document.getElementById(`coVal_${i}`);
    if (!hasStagedEdits) {
      stagedOffsets[i] = c.live_co;
      rng.value = c.live_co;
    }
    valEl.textContent = fmt(stagedOffsets[i]);
    if (stagedOffsets[i] !== c.live_co) {
      card.classList.add('modified');
    } else {
      card.classList.remove('modified');
    }
  });
}

function stepCore(coreId, delta) {
  const next = Math.max(-50, Math.min(10, (stagedOffsets[coreId] || 0) + delta));
  onCoreSlider(coreId, next);
  document.getElementById(`coRng_${coreId}`).value = next;
}

function onCoreSlider(coreId, val) {
  stagedOffsets[coreId] = val;
  hasStagedEdits = true;
  const fmt = v => (v > 0 ? `+${v}` : `${v}`);
  document.getElementById(`coVal_${coreId}`).textContent = fmt(val);
  if (lastSnap && lastSnap.cores[coreId]) {
    const card = document.getElementById(`coreCard_${coreId}`);
    if (val !== lastSnap.cores[coreId].live_co) card.classList.add('modified');
    else card.classList.remove('modified');
  }
  updateSyncBadge();
}

function stepAllCores(delta) {
  const slider = document.getElementById('allCoreSlider');
  const next = Math.max(-50, Math.min(10, parseInt(slider.value) + delta));
  slider.value = next;
  setAllCoresStaged(next);
}

function setAllCoresStaged(val) {
  const fmt = v => (v > 0 ? `+${v}` : `${v}`);
  document.getElementById('allCoreLabel').textContent = fmt(val);
  const count = lastSnap ? lastSnap.cores.length : stagedOffsets.length;
  for (let i = 0; i < count; i++) {
    stagedOffsets[i] = val;
    const rng = document.getElementById(`coRng_${i}`);
    if (rng) rng.value = val;
    const lbl = document.getElementById(`coVal_${i}`);
    if (lbl) lbl.textContent = fmt(val);
  }
  hasStagedEdits = true;
  if (lastSnap) renderCoresGrid(lastSnap.cores);
  updateSyncBadge();
}

function stagePresetOffsets(arr) {
  const count = lastSnap ? lastSnap.cores.length : arr.length;
  for (let i = 0; i < count; i++) {
    stagedOffsets[i] = arr[i % arr.length];
    const rng = document.getElementById(`coRng_${i}`);
    if (rng) rng.value = stagedOffsets[i];
  }
  hasStagedEdits = true;
  if (lastSnap) renderCoresGrid(lastSnap.cores);
  updateSyncBadge();
}

function onLimitEdit() {
  hasStagedEdits = true;
  const tctl = parseInt(document.getElementById('rngTctlMax').value);
  document.getElementById('lblTctlMax').textContent = `${tctl} °C`;
  stagedLimits.tctl_max_c = tctl;

  const chkPpt = document.getElementById('chkPpt').checked;
  const valPpt = parseInt(document.getElementById('rngPpt').value);
  document.getElementById('lblPpt').textContent = chkPpt ? `${valPpt} W` : 'BIOS';
  stagedLimits.ppt_limit_w = chkPpt ? valPpt : null;

  const chkTdc = document.getElementById('chkTdc').checked;
  const valTdc = parseInt(document.getElementById('rngTdc').value);
  document.getElementById('lblTdc').textContent = chkTdc ? `${valTdc} A` : 'BIOS';
  stagedLimits.tdc_limit_a = chkTdc ? valTdc : null;

  const chkEdc = document.getElementById('chkEdc').checked;
  const valEdc = parseInt(document.getElementById('rngEdc').value);
  document.getElementById('lblEdc').textContent = chkEdc ? `${valEdc} A` : 'BIOS';
  stagedLimits.edc_limit_a = chkEdc ? valEdc : null;

  updateSyncBadge();
}

function syncLimitsUiFromConfig(cfg) {
  const tctl = cfg.tctl_max_c || 90;
  document.getElementById('rngTctlMax').value = tctl;
  document.getElementById('lblTctlMax').textContent = `${tctl} °C`;
  stagedLimits.tctl_max_c = tctl;

  const hasPpt = cfg.ppt_limit_w !== null && cfg.ppt_limit_w > 0;
  document.getElementById('chkPpt').checked = hasPpt;
  if (hasPpt) document.getElementById('rngPpt').value = cfg.ppt_limit_w;
  document.getElementById('lblPpt').textContent = hasPpt ? `${cfg.ppt_limit_w} W` : 'BIOS';
  stagedLimits.ppt_limit_w = hasPpt ? cfg.ppt_limit_w : null;

  const hasTdc = cfg.tdc_limit_a !== null && cfg.tdc_limit_a > 0;
  document.getElementById('chkTdc').checked = hasTdc;
  if (hasTdc) document.getElementById('rngTdc').value = cfg.tdc_limit_a;
  document.getElementById('lblTdc').textContent = hasTdc ? `${cfg.tdc_limit_a} A` : 'BIOS';
  stagedLimits.tdc_limit_a = hasTdc ? cfg.tdc_limit_a : null;

  const hasEdc = cfg.edc_limit_a !== null && cfg.edc_limit_a > 0;
  document.getElementById('chkEdc').checked = hasEdc;
  if (hasEdc) document.getElementById('rngEdc').value = cfg.edc_limit_a;
  document.getElementById('lblEdc').textContent = hasEdc ? `${cfg.edc_limit_a} A` : 'BIOS';
  stagedLimits.edc_limit_a = hasEdc ? cfg.edc_limit_a : null;
}

function toggleSeries(key, el) {
  seriesVis[key] = !seriesVis[key];
  el.classList.toggle('off', !seriesVis[key]);
  if (lastSnap) drawChart(lastSnap.history);
}

function drawChart(history) {
  const canvas = document.getElementById('telemetryChart');
  if (!canvas || canvas.offsetParent === null) return;
  const rect = canvas.parentElement.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.max(100, (rect.width - 20)) * dpr;
  canvas.height = Math.max(80, (rect.height - 20)) * dpr;
  const ctx = canvas.getContext('2d');
  ctx.scale(dpr, dpr);
  const w = rect.width - 20;
  const h = rect.height - 20;

  ctx.clearRect(0, 0, w, h);
  const padL = 44, padR = 16, padT = 12, padB = 22;
  const pw = w - padL - padR;
  const ph = h - padT - padB;

  ctx.strokeStyle = '#222938';
  ctx.lineWidth = 1;
  ctx.fillStyle = '#9fb0c7';
  ctx.font = 'bold 13px "JetBrains Mono", "DejaVu Sans Mono", monospace';
  const yMax = 120;
  for (let v = 0; v <= yMax; v += 30) {
    const y = padT + ph - (v / yMax) * ph;
    ctx.beginPath();
    ctx.moveTo(padL, y);
    ctx.lineTo(padL + pw, y);
    ctx.stroke();
    ctx.fillText(`${v}`, 6, y + 4);
  }

  if (!history || history.length < 2) return;

  function plotLine(extractor, color, dashed = false) {
    ctx.save();
    ctx.strokeStyle = color;
    ctx.lineWidth = 2.5;
    if (dashed) ctx.setLineDash([5, 4]);
    ctx.beginPath();
    history.forEach((pt, idx) => {
      const x = padL + (idx / Math.max(1, history.length - 1)) * pw;
      const val = Math.max(0, Math.min(yMax, extractor(pt)));
      const y = padT + ph - (val / yMax) * ph;
      if (idx === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();
    ctx.restore();
  }

  plotLine(p => p.tctl_limit_c, 'rgba(244, 63, 94, 0.5)', true);
  if (seriesVis.tctl) plotLine(p => p.tctl_c, '#f59e0b');
  if (seriesVis.tccd1) plotLine(p => p.tccd1_c, '#fb7185');
  if (seriesVis.ppt) plotLine(p => p.ppt_w, '#38bdf8');
  if (seriesVis.freq) plotLine(p => (p.max_freq_mhz / 100.0), '#10b981');
}

let uiZoom = 1.0;
function detectDefaultZoom() {
  const saved = parseFloat(localStorage.getItem('cpu_adjust_ui_zoom'));
  if (!isNaN(saved) && saved >= 0.75 && saved <= 2.5) return saved;
  const sw = window.screen.width || window.innerWidth;
  if (sw >= 4500) return 1.75;
  if (sw >= 2800) return 1.35;
  if (sw >= 2200) return 1.20;
  return 1.0;
}

function applyUiZoom(val, persist = true) {
  uiZoom = Math.round(Math.max(0.75, Math.min(2.5, val)) * 100) / 100;
  document.documentElement.style.zoom = `${uiZoom}`;
  const btn = document.getElementById('zoomPctBtn');
  if (btn) btn.textContent = `${Math.round(uiZoom * 100)}%`;
  if (persist) localStorage.setItem('cpu_adjust_ui_zoom', `${uiZoom}`);
  if (lastSnap) setTimeout(() => drawChart(lastSnap.history), 30);
}

function adjustUiZoom(delta) {
  applyUiZoom(uiZoom + delta, true);
}

function resetUiZoom() {
  localStorage.removeItem('cpu_adjust_ui_zoom');
  applyUiZoom(detectDefaultZoom(), false);
}

window.addEventListener('keydown', e => {
  if (e.target && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA')) return;
  if (e.key === '+' || e.key === '=') { adjustUiZoom(0.1); }
  else if (e.key === '-') { adjustUiZoom(-0.1); }
  else if (e.key === '0') { resetUiZoom(); }
});

function updateUi(snap) {
  lastSnap = snap;
  const numCores = snap.num_cores || (snap.cores ? snap.cores.length : 8);
  const pstate = snap.pstate || {};
  const smuAdv = snap.smu_adv || {};
  const igpu = snap.igpu || {};

  document.getElementById('drvDot').className = 'dot' + (snap.drv_loaded ? '' : ' offline');
  document.getElementById('drvText').textContent = `ryzen_smu v${snap.drv_version}`;
  document.getElementById('smuVerText').textContent = `SMU ${snap.smu_version}`;

  const cpuTitle = snap.cpu_name || snap.cpu_model;
  if (cpuTitle) {
    document.getElementById('cpuHeaderTitle').innerHTML =
      `${cpuTitle} <span style="color:var(--text-muted);font-weight:500;font-size:15px;">(${numCores}C/${numCores*2}T &bull; ${snap.board_name || 'AM5'})</span>`;
  }
  document.getElementById('kpiCoreCountBadge').textContent = `${numCores}C DLDO PSM`;
  document.getElementById('coSectionTitle').textContent =
    `Per-Core PBO Curve Optimizer (${numCores} Physical Cores • RSMU 0x06 / MP1 0x35 / Readback 0xD5)`;
  document.getElementById('quickSetAllLabel').textContent = `Quick Set All ${numCores} Cores:`;

  // Sidebar status dots
  document.getElementById('dotDashboard').className = 'status-dot' + (snap.drv_loaded ? '' : ' inactive');
  document.getElementById('dotLimits').className = 'status-dot' + (snap.drv_loaded ? '' : ' inactive');
  document.getElementById('dotPstate').className = 'status-dot' + (pstate.active ? '' : ' inactive');
  document.getElementById('dotSmuAdv').className = 'status-dot' + (smuAdv.active ? '' : ' inactive');
  document.getElementById('dotIgpu').className = 'status-dot' + (igpu.active ? '' : ' inactive');

  const tctl = snap.tctl_c || 0;
  const tccd1 = snap.tccd1_c || 0;
  const tlim = snap.tctl_limit_c || 90;
  document.getElementById('kpiTctl').textContent = `${tctl.toFixed(1)} °C`;
  document.getElementById('kpiTccd1').textContent = `${tccd1.toFixed(1)} °C`;
  document.getElementById('kpiTctlLim').textContent = `Max ${tlim.toFixed(0)}°C`;
  document.getElementById('kpiHeadroom').textContent = `${(tlim - tctl).toFixed(1)}°C margin`;
  document.getElementById('barTctl').style.width = `${Math.min(100, (tctl / tlim) * 100)}%`;

  const ppt = snap.ppt_value_w || 0;
  const pptLim = snap.ppt_limit_w || 142;
  document.getElementById('kpiPpt').textContent = `${ppt.toFixed(1)} W`;
  document.getElementById('kpiPptLim').textContent = `Limit ${pptLim.toFixed(0)} W`;
  document.getElementById('kpiTdc').textContent = snap.tdc_value_a ? `${snap.tdc_value_a.toFixed(1)} A` : '--';
  document.getElementById('kpiEdc').textContent = snap.edc_value_a ? `${snap.edc_value_a.toFixed(1)} A` : '--';
  document.getElementById('barPpt').style.width = `${Math.min(100, (ppt / pptLim) * 100)}%`;

  const maxFreq = Math.max(...snap.cores.map(c => c.cur_freq_mhz), 0);
  const avgFreq = Math.round(snap.cores.reduce((a, c) => a + c.cur_freq_mhz, 0) / Math.max(1, snap.cores.length));
  document.getElementById('kpiPeakFreq').textContent = `${maxFreq} MHz`;
  document.getElementById('kpiAvgFreq').textContent = `Avg ${avgFreq} MHz`;
  document.getElementById('kpiVcore').textContent = snap.vddcr_cpu_v ? `${snap.vddcr_cpu_v.toFixed(3)} V` : '--';
  document.getElementById('kpiVsoc').textContent = snap.vddcr_soc_v ? `${snap.vddcr_soc_v.toFixed(3)} V` : '--';
  document.getElementById('barFreq').style.width = `${Math.min(100, (maxFreq / 5550) * 100)}%`;

  const liveOffsets = snap.cores.map(c => c.live_co);
  const allSame = liveOffsets.every(v => v === liveOffsets[0]);
  const minCo = Math.min(...liveOffsets);
  const maxCo = Math.max(...liveOffsets);
  document.getElementById('kpiCoSummary').textContent = allSame ? `All ${liveOffsets[0]}` : `${minCo} .. ${maxCo}`;
  const avgCo = liveOffsets.reduce((a, b) => a + b, 0) / Math.max(1, liveOffsets.length);
  document.getElementById('kpiCoMv').textContent = `Avg ${avgCo.toFixed(1)} (~${Math.round(avgCo * 4)} mV)`;
  document.getElementById('kpiPoll').textContent = `Poll ${snap.config.poll_interval_secs}s`;

  // Sidebar tree
  document.getElementById('stTctl').textContent = `${tctl.toFixed(1)} / ${tlim.toFixed(0)} °C`;
  document.getElementById('stTccd1').textContent = `${tccd1.toFixed(1)} °C`;
  document.getElementById('stPpt').textContent = `${ppt.toFixed(1)} / ${pptLim.toFixed(0)} W`;
  document.getElementById('stCurrents').textContent = `${(snap.tdc_value_a||0).toFixed(0)}A / ${(snap.edc_value_a||0).toFixed(0)}A`;
  document.getElementById('stVcore').textContent = snap.vddcr_cpu_v ? `${snap.vddcr_cpu_v.toFixed(3)} V` : '--';
  document.getElementById('stVsoc').textContent = snap.vddcr_soc_v ? `${snap.vddcr_soc_v.toFixed(3)} V` : '--';
  document.getElementById('stClocks').textContent = snap.fclk_mhz ? `${snap.fclk_mhz.toFixed(0)}/${snap.uclk_mhz.toFixed(0)}/${snap.mclk_mhz.toFixed(0)}` : '--';
  document.getElementById('stEpp').textContent = pstate.epp || '--';
  document.getElementById('stIgpu').textContent = (igpu.temp_c || snap.igpu_temp_c) ? `${(igpu.temp_c || snap.igpu_temp_c).toFixed(0)}°C • ${igpu.sclk_mhz || snap.igpu_sclk_mhz || 0}MHz` : '--';
  const dimmStr = (snap.dimm_temps_c && snap.dimm_temps_c.length) ? `${snap.dimm_temps_c[0].toFixed(0)}°C` : '--';
  document.getElementById('stStorage').textContent = `${snap.nvme_temp_c ? snap.nvme_temp_c.toFixed(0)+'°C' : '--'} / DDR5 ${dimmStr}`;

  const fansEl = document.getElementById('stFans');
  fansEl.innerHTML = (snap.fans || []).map(f =>
    `<div class="tree-row"><span>${f.label}</span><span class="tree-val">${f.rpm} RPM</span></div>`
  ).join('');

  // Live limits labels
  document.getElementById('liveTctlLimit').textContent = `Live SMU: ${tlim.toFixed(0)} °C`;
  document.getElementById('livePptLimit').textContent = `Live SMU: ${pptLim.toFixed(0)} W`;
  document.getElementById('liveTdcLimit').textContent = `Live SMU: ${(snap.tdc_limit_a||0).toFixed(0)} A`;
  document.getElementById('liveEdcLimit').textContent = `Live SMU: ${(snap.edc_limit_a||0).toFixed(0)} A`;

  // Update P-State sub-panel
  document.getElementById('pstateSubtitle').textContent =
    `Driver: ${pstate.scaling_driver || 'inactive'} (${pstate.status || 'off'}) | Governor: ${pstate.scaling_governor || '-'} | Prefcore: ${pstate.prefcore || '-'}`;
  ['performance', 'balance_performance', 'balance_power', 'power'].forEach(ep => {
    const btn = document.getElementById('btnEpp_' + ep);
    if (btn) btn.className = (pstate.epp === ep) ? 'active-choice' : '';
  });
  ['powersave', 'performance'].forEach(gv => {
    const btn = document.getElementById('btnGov_' + gv);
    if (btn) btn.className = (pstate.scaling_governor === gv) ? 'active-choice' : '';
  });
  const boostBtn = document.getElementById('btnPstateBoost');
  if (boostBtn) {
    boostBtn.textContent = `CPB Boost: ${pstate.boost_enabled ? 'ON' : 'OFF'}`;
    boostBtn.className = pstate.boost_enabled ? 'active-choice' : '';
  }
  const dynBtn = document.getElementById('btnPstateDynEpp');
  if (dynBtn) {
    dynBtn.textContent = `Dynamic EPP: ${pstate.dynamic_epp ? 'ON' : 'OFF'}`;
    dynBtn.className = pstate.dynamic_epp ? 'active-choice' : '';
  }
  if (pstate.scaling_max_freq_mhz) {
    document.getElementById('rngPstateMaxFreq').value = pstate.scaling_max_freq_mhz;
    document.getElementById('lblPstateMaxFreq').textContent = `${pstate.scaling_max_freq_mhz} MHz`;
    document.getElementById('subPstateMaxFreq').textContent =
      `Hardware range: ${pstate.cpuinfo_min_freq_mhz || 545} – ${pstate.cpuinfo_max_freq_mhz || 5535} MHz (Nonlinear ${pstate.lowest_nonlinear_freq_mhz || 3000} MHz)`;
  }
  if (pstate.scaling_min_freq_mhz) {
    document.getElementById('rngPstateMinFreq').value = pstate.scaling_min_freq_mhz;
    document.getElementById('lblPstateMinFreq').textContent = `${pstate.scaling_min_freq_mhz} MHz`;
  }

  // Update SMU Advanced sub-panel
  const gfxCo = (smuAdv.gfx_co_margin !== null && smuAdv.gfx_co_margin !== undefined) ? smuAdv.gfx_co_margin : (snap.config.gfx_co_margin || 0);
  document.getElementById('rngGfxCo').value = gfxCo;
  document.getElementById('lblGfxCo').textContent = gfxCo > 0 ? `+${gfxCo}` : `${gfxCo}`;
  const tdcSocLim = Math.round(smuAdv.tdc_soc_limit_a || snap.config.tdc_soc_limit_a || 30);
  document.getElementById('rngTdcSoc').value = tdcSocLim;
  document.getElementById('lblTdcSoc').textContent = `${tdcSocLim} A`;
  document.getElementById('subTdcSoc').textContent = `Configured SoC TDC: ${tdcSocLim} A (${smuAdv.tdc_soc_limit_a ? 'Custom' : 'BIOS Default'})`;
  const edcSocLim = Math.round(smuAdv.edc_soc_limit_a || snap.config.edc_soc_limit_a || 45);
  document.getElementById('rngEdcSoc').value = edcSocLim;
  document.getElementById('lblEdcSoc').textContent = `${edcSocLim} A`;
  document.getElementById('subEdcSoc').textContent = `Configured SoC EDC: ${edcSocLim} A (${smuAdv.edc_soc_limit_a ? 'Custom' : 'BIOS Default'})`;
  const scalar = Math.round(smuAdv.pbo_scalar || snap.config.pbo_scalar || 1);
  [1, 2, 3, 5, 7, 10].forEach(sc => {
    const b = document.getElementById('btnScalar_' + sc);
    if (b) b.className = (scalar === sc) ? 'active-choice' : '';
  });
  const rampMs = Math.round(smuAdv.prochot_ramp_ms || snap.config.prochot_ramp_ms || 20);
  [2, 10, 20, 50].forEach(rm => {
    const b = document.getElementById('btnRamp_' + rm);
    if (b) b.className = (rampMs === rm) ? 'active-choice' : '';
  });
  const fclk = smuAdv.fclk_mhz || snap.fclk_mhz || 0;
  const uclk = smuAdv.uclk_mhz || snap.uclk_mhz || 0;
  const mclk = smuAdv.mclk_mhz || snap.mclk_mhz || 0;
  const vsoc = smuAdv.vddcr_soc_v || snap.vddcr_soc_v || 0;
  document.getElementById('smuAdvSubtitle').textContent =
    `FCLK ${Math.round(fclk)} MHz : UCLK ${Math.round(uclk)} MHz : MCLK ${Math.round(mclk)} MHz (${smuAdv.coupled_mode ? '1:1 Coupled' : 'Async'}) | VDDCR_SOC: ${vsoc.toFixed(3)} V`;
  const fabEl = document.getElementById('smuAdvFabricStats');
  if (fabEl) {
    fabEl.innerHTML = `
      <div>Infinity Fabric (FCLK): <b>${fclk ? fclk.toFixed(0) + ' MHz' : 'N/A'}</b></div>
      <div>Memory Controller (UCLK): <b>${uclk ? uclk.toFixed(0) + ' MHz' : 'N/A'}</b></div>
      <div>DDR5 Memory Clock (MCLK): <b>${mclk ? mclk.toFixed(0) + ' MHz (DDR5-' + Math.round(mclk * 2) + ')' : 'N/A'}</b></div>
      <div>UMC Sync Mode &amp; VDDCR_SOC: <b>${smuAdv.coupled_mode ? '1:1 Coupled (UCLK == MCLK)' : '2:1 Decoupled'} &bull; ${vsoc ? vsoc.toFixed(3) + ' V' : 'N/A'}</b></div>
    `;
  }

  // Update iGPU sub-panel
  ['auto', 'low', 'high', 'profile_standard', 'profile_min_sclk', 'profile_min_mclk', 'profile_peak'].forEach(lvl => {
    const b = document.getElementById('btnDpm_' + lvl);
    if (b) b.className = (igpu.dpm_level === lvl) ? 'active-choice' : '';
  });
  document.getElementById('igpuLiveStats').innerHTML = `
    <div>Card / Driver: <b>${igpu.card_name || 'card1'} (${igpu.pci_id || '1002:164e'} &bull; amdgpu)</b></div>
    <div>Active SCLK Clock: <b>${igpu.sclk_mhz || 0} MHz</b> (Load: <b>${igpu.gpu_busy_pct || 0}%</b>)</div>
    <div>iGPU Temperature / Power: <b>${(igpu.temp_c || 0).toFixed(1)} °C</b> / <b>${(igpu.power_w || 0).toFixed(1)} W</b></div>
    <div>Graphics Rail VDDGFX: <b>${igpu.vddgfx_mv || 0} mV</b></div>
  `;
  document.getElementById('igpuDpmStates').innerHTML = `
    <div>SCLK DPM Table: <b>${igpu.sclk_dpm_summary || '0: 400Mhz * 1: 2200Mhz'}</b></div>
    <div>Power DPM State: <b>${igpu.power_dpm_state || 'balanced'}</b></div>
    <div>Dedicated VRAM Used: <b>${igpu.vram_used_mb || 0} / ${igpu.vram_total_mb || 512} MB</b></div>
    <div>Shared GTT Memory: <b>${igpu.gtt_used_mb || 0} / ${igpu.gtt_total_mb || 0} MB</b></div>
  `;

  if (!hasStagedEdits) {
    syncLimitsUiFromConfig(snap.config);
  }
  renderCoresGrid(snap.cores);
  refreshViewVisibility();
  drawChart(snap.history);
}

async function fetchStatus() {
  try {
    const res = await fetch('/api/status');
    const data = await res.json();
    if (data.ok) updateUi(data);
  } catch (e) {
    document.getElementById('drvDot').className = 'dot offline';
  }
}

async function reloadFromSmu() {
  hasStagedEdits = false;
  updateSyncBadge();
  await fetchStatus();
  showToast('Reloaded live SMU state');
}

async function applyAll(saveToConfig) {
  try {
    const numCores = lastSnap ? lastSnap.cores.length : stagedOffsets.length;
    const res1 = await fetch('/api/co/cores', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({ offsets: stagedOffsets.slice(0, numCores), save: saveToConfig })
    });
    const d1 = await res1.json();
    if (!d1.ok) throw new Error(d1.error || 'Failed to apply Curve Optimizer offsets');

    const res2 = await fetch('/api/limits', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({
        tctl_max_c: stagedLimits.tctl_max_c,
        ppt_limit_w: stagedLimits.ppt_limit_w,
        tdc_limit_a: stagedLimits.tdc_limit_a,
        edc_limit_a: stagedLimits.edc_limit_a,
        save: saveToConfig
      })
    });
    const d2 = await res2.json();
    if (!d2.ok) throw new Error(d2.error || 'Failed to apply thermal/power limits');

    hasStagedEdits = false;
    updateSyncBadge();
    updateUi(d2);
    showToast(saveToConfig ? 'Applied to SMU & saved to config.toml' : 'Applied live to SMU registers');
  } catch (err) {
    showToast(err.message, true);
  }
}

async function applyPreset(presetName) {
  try {
    const res = await fetch('/api/preset', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({ preset: presetName, save: true })
    });
    const data = await res.json();
    if (!data.ok) throw new Error(data.error || 'Preset failed');
    hasStagedEdits = false;
    updateSyncBadge();
    updateUi(data);
    showToast(`Activated preset '${presetName}' & saved to config.toml`);
  } catch (err) {
    showToast(err.message, true);
  }
}

async function postPstate(payload) {
  try {
    const res = await fetch('/api/pstate', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify(payload)
    });
    const data = await res.json();
    if (!data.ok) throw new Error(data.error || 'AMD P-State update failed');
    updateUi(data);
    showToast('Updated AMD P-State configuration');
  } catch (err) {
    showToast(err.message, true);
  }
}

function togglePstateBoost() {
  const next = !(lastSnap && lastSnap.pstate && lastSnap.pstate.boost_enabled);
  postPstate({ boost: next, save: true });
}

function togglePstateDynEpp() {
  const next = !(lastSnap && lastSnap.pstate && lastSnap.pstate.dynamic_epp);
  postPstate({ dynamic_epp: next, save: true });
}

async function postSmuAdv(payload) {
  try {
    const res = await fetch('/api/smu-advanced', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify(payload)
    });
    const data = await res.json();
    if (!data.ok) throw new Error(data.error || 'SMU Advanced update failed');
    updateUi(data);
    showToast('Updated SMU Advanced configuration');
  } catch (err) {
    showToast(err.message, true);
  }
}

async function postIgpuDpm(level) {
  try {
    const res = await fetch('/api/igpu', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({ dpm_level: level, save: true })
    });
    const data = await res.json();
    if (!data.ok) throw new Error(data.error || 'iGPU DPM update failed');
    updateUi(data);
    showToast(`Set Raphael iGPU DPM level to '${level}'`);
  } catch (err) {
    showToast(err.message, true);
  }
}

window.addEventListener('resize', () => { if (lastSnap) drawChart(lastSnap.history); });
applyUiZoom(detectDefaultZoom(), false);
fetchStatus();
setInterval(fetchStatus, 1500);
</script>
</body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_ui_schema_bindings_and_subpanels() {
        assert!(INDEX_HTML.contains("lastSnap.pstate && lastSnap.pstate.active"));
        assert!(INDEX_HTML.contains("lastSnap.smu_adv && lastSnap.smu_adv.active"));
        assert!(INDEX_HTML.contains("lastSnap.igpu && lastSnap.igpu.active"));
        assert!(INDEX_HTML.contains("scaling_max_freq_mhz"));
        assert!(INDEX_HTML.contains("scaling_min_freq_mhz"));
        assert!(INDEX_HTML.contains("prochot_ramp_ms"));
        assert!(!INDEX_HTML.contains("2. Curve Optimizer"));
    }
}

