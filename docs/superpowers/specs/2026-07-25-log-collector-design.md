# ProcDumpMonitor 2.0 — Mode-Based App with Integrated Log Collector

**Date:** 2026-07-25 · **Status:** Approved by user (Option A + native engine + 2 additions)

## Goal

Meld ProcDumpMonitor's setup wizard and the CCURE Log Collector GUI v2.0
(`CCURE_LogCollector_GUI_v2.0.ps1`, spec-by-example) into one mode-based app:
the wizard becomes a single "do everything" Monitor page, and the collector's
real workflows become sidebar entries — one exe, one shell, one theme.

## Non-negotiable constraints (unchanged)

- nwg 1.0.13, no new crate dependencies, size gate ~1.9MB, `panic="abort"`.
- Release manifest keeps `requireAdministrator`; `PDM_TEST_MANIFEST=1` only for test builds.
- All coordinates LOGICAL px; design system palette/fonts/grid from
  `docs/plans/gui-redesign-plan.md` still bind (colors, fonts, PAD/FIELD grid).
- Every nwg control stored in its page struct; console children spawned with
  `CREATE_NO_WINDOW`; windows built hidden, shown after theme registration.

## UI

### Shell (free navigation replaces the wizard)

- Window 920×780 logical, page frames at (240,100) size 680×596.
- Sidebar rows become clickable; Back/Next buttons and step numbers removed.
- Sidebar layout: app title + subtitle "Monitor & log collection", then groups:
  - `MONITOR` → **Monitor**
  - `LOG COLLECTOR` → **Data Collection**, **Install Logs**, **System Health**
  - bottom: **About**, version string.
- Group captions: 10px semibold uppercase, muted. Active row: accent text +
  3px accent bar (existing treatment). Page switch = save current page (save
  failures no longer block navigation — see Validation), hide frame, show target.
- Header keeps per-page title/subtitle; adds a green "Administrator" pill
  (static — the exe is always elevated).

### Monitor page (merged Target+ProcDump+Task+Notify+Review)

- **Target**: ONE editable ComboBox listing `Svc: <display> (<name>)` for
  services and `Proc: <exe>` for running processes, + Refresh + "Show all"
  checkbox (stopped services & all processes). Picking sets type from the tag;
  typing free text = process (existing `infer_target_type` semantics kept).
- **Dump triggers & output**: scenario presets, Crash/Hang toggles, CPU ≥ n%,
  Mem ≥ n MB, dump count, Full-dump toggle, dump folder + Browse,
  procdump.exe path + Browse. Power-user fields (Incl/Excl/Avoid filters,
  custom args) move to an **Advanced…** modal dialog.
- **Schedule & notifications**: task name, start-at-boot, notification email,
  webhook URL. Full SMTP config (host/port/TLS/from/credentials/test send) in
  an **SMTP…** modal dialog. New checkbox: **Auto-collect on dump** (below).
- **Monitor status panel** (the "prove it worked" requirement):
  - Row 1: Scheduled task created — verified via `schtasks` query; on failure
    red ✕ + actual stderr text.
  - Row 2: Monitor running — PID + heartbeat age from `health.json`; stale
    (> 2 cycle intervals) → gray dot "not running — last seen <time>".
  - Row 3: ProcDump attached — ProcDumpPid ≠ 0 from health.json, target name.
  - Row 4: Dumps captured — TotalDumpCount, LastDumpFileName, free disk;
    amber row with LastError when present.
  - Polled by a 2s nwg timer active only while the page is visible.
  - Create/Run/Stop/Remove buttons sit in the page footer and enable/disable
    from live status.
- **Validation**: checks that used to block "Next" (e.g. invalid email) now
  run when **Create task** is clicked; inline error text, no modal nagging.

### Collector pages (ports of the PS1's three real workflows)

Common: Start button + status line per page; collection runs on a worker
thread, progress marshaled via `nwg::Notice`; "Open last output" opens
Explorer. Options reset to PS1 defaults each launch (not persisted).

1. **Data Collection** — save path (blank = Desktop) + Browse; "Extra
   collections" checkbox grid (system info, installed apps, installed
   updates, event logs, InstallHistory.xml, bulk updates, SWHSystem
   settings) **plus new: "ProcDumpMonitor logs, dumps & task state"**;
   "Log components (install-dir based)" grid (CCure Portal/Web, CrossFire
   Logging, Security Intelligence Datacache, VictorWeb, VictorWebServices
   auth/website) discovered from the SWHSystem registry key with JCI/Tyco
   vendor-root fallbacks, exactly per the PS1; Select all / Select none.
   - **Event logs option (new)**: "Last 7 days" default, "Full export"
     checkbox (PS1 behavior) — `wevtutil epl` with a TimeCreated query.
2. **Install Logs** — InstallHistory.xml path + Browse, auto-discover
   checkbox (ProgramData\{Tyco,JCI}\InstallerTemp, newest first), include
   InstallerTemp contents, include InstallCache (JCI or Tyco).
3. **System Health** — uptime, process snapshot, service snapshot (with
   dependencies) checkboxes + the PS1's default comma-separated match
   patterns in two edit boxes. JSON + CSV + TXT outputs like the PS1.

### Dropped from the PS1 (deliberate)

- Controller Logs & Integrations tabs — unimplemented stubs in v2.0 source.
- "Relaunch as Admin", non-admin best-effort mode, admin-mode banner — this
  exe is always elevated.
- VSS locked-file capture — v1 uses robocopy `/R:1 /W:1` best-effort (the
  PS1's own non-admin path). ponytail: VSS snapshot support if locked files
  become a real support gap.

## Collection engine (native Rust, no new deps)

New `rust/src/collect/` module tree: `mod.rs` (run orchestration, run-folder
layout, summary + transcript), `datacoll.rs`, `installlogs.rs`, `health_wf.rs`,
`pdm_bundle.rs`, `discover.rs` (registry + vendor roots).

- Output layout mirrors the PS1: `<base>\YYYY-MM-DD\Run_HHMMSS\` containing
  `Collection_Summary.txt`, `Run_Transcript.txt`, and one zip per workflow.
- Folder capture: spawn `robocopy /E /Z /R:1 /W:1 /XJ /NFL /NDL /NP`
  (CREATE_NO_WINDOW), exit code ≥ 8 logged as partial.
- Event logs: `wevtutil epl Application|System`, 7-day TimeCreated filter by
  default, full when requested.
- System info: OS/computer info via existing helpers + `systeminfo.exe`
  output; installed apps + updates from registry Uninstall keys (both views).
- Zip: `tar.exe -a -cf out.zip -C <dir> .`; if tar.exe missing (pre-1803 /
  Server 2016), fallback `powershell -NoProfile Compress-Archive`.
- PDM bundle: app log, `health.json`, config JSON **with SMTP password
  redacted**, `schtasks /query /v` output for the task, dump-folder listing
  + newest dumps (size-capped).
- CLI: `ProcDumpMonitor collect [--out <dir>] [--workflows data,install,health,pdm]`
  drives the same engine headless (also how we smoke-test).

## Auto-collect on dump

- `Config.auto_collect_on_dump: bool` (serde default false — config-file
  compatible both directions).
- Hook in monitor.rs after dump-stability + notification: run a mini
  collection (new dump, PDM log, health.json, redacted config, last-24h
  event logs, task state) into `<dump_dir>\SupportBundles\<run layout>`.
- Guards: skip when `disk_space_low`; rate-limit one auto-bundle per 60 min
  (crash-loop protection). ponytail: fixed 60-min limit; make configurable
  only if a real deployment needs it.

## Verification (ship criteria)

- All existing tests keep passing + new unit tests: discover paths, evtx
  query string, redaction, run-folder naming, summary content (Windows-only
  bits behind cfg(windows), pure logic testable everywhere).
- `PDM_TEST_MANIFEST=1 cargo test` green; release build ~1.9MB with
  requireAdministrator manifest confirmed.
- `collect` CLI smoke run produces a real bundle on this machine.
- `scripts/gui-e2e.ps1` updated for free-nav (sidebar clicks instead of
  Next), click-through of ALL five pages with a screenshot per page and
  captured logs; run under powershell.exe 5.1 with the machine idle.

## Files touched (planned)

- New: `rust/src/collect/*`, `rust/src/gui/page_monitor.rs`,
  `rust/src/gui/page_datacoll.rs`, `rust/src/gui/page_installlogs.rs`,
  `rust/src/gui/page_syshealth.rs`, dialogs in `rust/src/gui/dlg_*.rs`.
- Changed: `gui/mod.rs` (shell), `config.rs` (auto_collect flag),
  `monitor.rs` (hook), `main.rs` (CLI), `scripts/gui-e2e.ps1`.
- Removed from build: old `page_target/procdump/task/notify/review.rs`
  (About stays). Old page structs' frozen-API contract is superseded by
  this approved redesign.
