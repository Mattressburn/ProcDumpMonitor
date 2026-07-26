# PROJECT CONTINUATION DOCUMENT
## Session 4 — 26 July 2026

### 1. PROJECT IDENTITY

- **Project Name:** ProcDumpMonitor
- **What This Project Is:** A single-exe Windows utility for C•CURE 9000 field support. It (a) watches a process or service and captures ProcDump crash/hang/resource dumps automatically with retention + email/webhook alerts via an installed Scheduled Task, and (b) — new this session — collects CCURE diagnostic log bundles for Johnson Controls support. One tool for "arm crash capture" and "gather evidence".
- **Primary Objective:** One self-contained ~1.97MB Rust exe (monitor + CLI + GUI) that replaces the legacy C#/.NET app and the customer's `CCURE_LogCollector_GUI_v2.0.ps1`, deployable by dropping one file on a server.
- **Strategic Intent:** Kill the .NET runtime dependency and the PowerShell-script distribution problem. A field tech drops one exe on a C-CURE box, runs it elevated, and can both configure monitoring and produce a support bundle in under a minute — no execution-policy fights, no AV flagging of scripts, no installer.
- **Hard Constraints:**
  - GUI stays `native-windows-gui` 1.0.13 (NOT egui/iced). No new crate dependencies — new Windows API surface comes from the already-present `windows` crate or `std`.
  - **Binary size gate ~2.0MB** (raised from ~1.9MB and explicitly accepted by the user on 2026-07-25; currently 1.97MB, so only ~30KB headroom). `opt-level=z`, `lto`, `panic="abort"`, `strip` all stay.
  - Release manifest keeps `requireAdministrator`; `build.rs` panics if `PDM_TEST_MANIFEST=1` leaks into a `--release` build.
  - All GUI coordinates LOGICAL px (nwg `high-dpi` scales them). Never multiply by `scale_factor()` outside raw GDI paint paths.
  - Rust toolchain: `%USERPROFILE%\.cargo\bin\cargo.exe` (NOT on PATH), run from `rust/`.
  - No PowerShell script ships with the product. The collector is native Rust that shells only to built-in Windows tools.

### 2. WHAT EXISTS RIGHT NOW

- **Built and working (✅ verified this session):**
  - **Mode-based sidebar shell** (`rust/src/gui/mod.rs`, 920×780): freely-clickable nav, groups `MONITOR` and `LOG COLLECTOR` plus About. The 6-step Back/Next wizard is gone.
  - **Merged Monitor page** (`page_monitor.rs`): combined `Proc:`/`Svc:` target dropdown, dump triggers + output, schedule + notify essentials, footer Create/Run/Stop/Remove/Save/Open/Logs/Copy/Scheduler, and a **live status panel** (schtasks + `health.json`, 3s poll) that proves task-created → monitor-running → ProcDump-attached → dumps-captured.
  - **Two owned dialogs**: `dlg_advanced.rs` (power-user ProcDump switches + logs/retention + manual target override), `dlg_smtp.rs` (full SMTP + validate + test send).
  - **Three collector pages** (`page_datacoll.rs`, `page_installlogs.rs`, `page_syshealth.rs`) on a worker thread via `collect_runner.rs` + `nwg::Notice`.
  - **Native collection engine** (`rust/src/collect/`): ports the PS1's three real workflows + a ProcDumpMonitor support bundle. Also exposed as the `collect` CLI verb. **All four workflows executed successfully** (exit 0, valid zips: DataCollection 382KB with SystemInfo/InstalledApplications.csv/InstalledUpdates.csv/Application.evtx/System.evtx).
  - **Auto-collect-on-dump** hook in `monitor.rs` (`config.auto_collect_on_dump`, rate-limited 60 min, skipped on low disk).
  - GUI now initializes the logger, so GUI actions land in `procdump.log`.
  - **Target dropdown scrolling FIXED** (WS_VSCROLL — see §4). Processes listed first, services after.
  - 63/63 `cargo test` pass. `scripts/gui-e2e.ps1` exits 0: clicks all 5 pages, opens/closes both dialogs, runs a real System Health collection, captures app log + transcript, screenshots every state. Release build 1.97MB with `requireAdministrator` confirmed.
- **Partially built:**
  - `collect::pdm_bundle::auto_bundle` (the on-dump wrapper) has **never executed** — it needs a real dump to fire. Its inner `run_into` is exercised by the `pdm` workflow, so risk is low but it is unproven.
  - Install Logs workflow runs correctly but on this dev machine always hits "InstallHistory.xml not found" (no CCURE installed). **Never validated against a real CCURE server** — same for the log-component robocopy paths, vendor-root discovery, bulk updates, and SWHSystem settings.
  - Legacy C# app in the repo root: WIP commit `26ab167` remains unreviewed and may not build.
- **Broken or blocked:**
  - **`git push origin main` fails**: `remote: Repository not found` for `https://github.com/jraburm_jcplc/ProcDump-Monitor.git` (JCI work account). All pushes this session went to the `personal` remote (`Mattressburn/ProcDumpMonitor`), which works. Needs the user to confirm whether the JCI repo must be created or a credential refreshed.
- **NOT started yet:**
  - **Full UI polish pass** (the user's stated next objective, with design MCP servers they intend to add).
  - **Target-picker UX redesign** — see §4/§5. Scrolling now works, but 278 entries with processes first still buries services ~130 rows down. This is the substance behind the user's "I don't see any services".
  - Controller Logs and Integrations collector workflows (unimplemented stubs in the source PS1; deliberately out of scope).
  - VSS locked-file capture (v1 uses robocopy best-effort, matching the PS1's non-admin path).
  - No dark mode / theming beyond the existing light design system.

### 3. ARCHITECTURE & TECHNICAL MAP

- **Tech stack:** Rust (stable-msvc), `native-windows-gui` 1.0.13 + `native-windows-derive`, `windows` 0.58, serde/serde_json, chrono, base64, lettre (SMTP), ureq (webhook); `winresource` for icon+manifest; MSVC 2019 Build Tools linker. Legacy C#/.NET app still present in the repo root.
- **Key files:**
  - `rust/src/gui/mod.rs` — shell: window, sidebar groups + clickable nav rows, header + Administrator pill, page frames, footer action buttons, status poll timer, and **three event dispatchers** (main window, Advanced dialog, SMTP dialog).
  - `rust/src/gui/page_monitor.rs` — the merged page. **Critical contract:** `write_fields()` is control-pure (safe on a throwaway clone, used by `refresh_preview`); `save()` = `write_fields` + side effects (syncs the auto task-name box, DPAPI-protects and clears a typed webhook URL). `mk_combo()` injects `WS_VSCROLL`.
  - `rust/src/gui/dlg_advanced.rs`, `dlg_smtp.rs` — owned reusable windows, hidden (not destroyed) on close; their dispatchers re-enable the owner.
  - `rust/src/gui/page_datacoll.rs` (also hosts the shared `pump()` + `resolve_base()`), `page_installlogs.rs`, `page_syshealth.rs`, `collect_runner.rs`.
  - `rust/src/gui/theme.rs` — colors/fonts, raw `WM_CTLCOLORSTATIC`/`WM_ERASEBKGND`, plus `set_status_color()` for the live status rows.
  - `rust/src/collect/` — `mod.rs` (RunContext, run-folder layout, transcript/summary, robocopy/zip/evtx/powershell helpers, `redact_config_json`), `discover.rs` (registry + vendor roots + InstallHistory discovery, all pure + unit-tested), `datacoll.rs`, `installlogs.rs`, `syshealth.rs`, `pdm_bundle.rs`.
  - `rust/src/{monitor,procdump,task,services,notify,retention,health,bitness,cli,config,logger,paths,secrets,stability,diskguard}.rs` — core. `bitness.rs` also provides `list_process_names()` for the target dropdown.
  - `scripts/gui-e2e.ps1` — UI automation driver (PowerShell 5.1).
  - `docs/superpowers/specs/2026-07-25-log-collector-design.md` — the approved design spec.
  - `docs/plans/gui-redesign-plan.md` — the binding design system (grid/colors/fonts).
- **How the system works end-to-end:**
  1. Elevated launch with no args → GUI. `Config::load()` from `config.json` beside the exe; logger initialized.
  2. **Monitor page**: user picks a target from the combined dropdown (`Proc: name` / `Svc: display (name)`), sets triggers/paths, task name, notification essentials; Advanced/SMTP dialogs write straight into the shared `Config`.
  3. **Create Task** → validate notify → `Config::save()` → shells this same exe's `install` verb → `schtasks` registers a task running `ProcDumpMonitor.exe monitor`.
  4. The installed task's `monitor` loop watches the target, launches procdump on triggers, waits for dump stability, applies retention, sends email/webhook, writes `health.json` each cycle (and every 30s while ProcDump is attached), and — if enabled — runs a rate-limited auto-collect bundle.
  5. The Monitor page's status panel polls `schtasks` + `health.json` every 3s and reports the verified chain; failures show real error text.
  6. **Collector pages** build an options struct from checkboxes → `CollectRunner::start()` spawns a worker → `collect::RunContext::start()` creates `<base>\YYYY-MM-DD\Run_HHMMSS\` → workflow modules shell to robocopy/wevtutil/reg/systeminfo/powershell → each zips via `tar.exe` (fallback `Compress-Archive`) → `Collection_Summary.txt` + `Run_Transcript.txt`. Progress marshals to the UI via `nwg::Notice`.
  7. `ProcDumpMonitor.exe collect [--out DIR] [--workflows data,install,health,pdm]` drives the identical engine headlessly.
- **Naming conventions:** commit prefixes `gui:`/`collect:`/`fix(monitor):`/`docs:`/`verify:`/`spec:`. GUI pages `page_<name>.rs`, dialogs `dlg_<name>.rs`. Config JSON is PascalCase (C#-compatible, `#[serde(rename_all = "PascalCase")]`). Deliberate shortcuts are marked with `ponytail:` comments naming the ceiling.
- **External dependencies:** `sc.exe`, `schtasks.exe`, `robocopy`, `wevtutil.exe`, `reg.exe`, `systeminfo.exe`, `tar.exe`, `powershell.exe` (inline `-Command` only), `explorer.exe`, `notepad.exe`, `mmc.exe taskschd.msc`, procdump.exe/procdump64.exe (user-supplied), SMTP server / webhook endpoint.

### 4. RECENT WORK — WHAT JUST HAPPENED (HIGH PRIORITY)

- **What was worked on:** Brainstormed, specced, built, and verified the merge of the CCURE log collector into ProcDumpMonitor, converting the app from a wizard to a mode-based shell. Then fixed the target-dropdown defects the user reported. Commits `54d9e86..9fe711f` on `main`.

- **Decisions and WHY:**
  - **Wizard → mode-based sidebar (user-approved "Option A").** Each page already load/saved independently against one shared `Config`; Back/Next was just show/hide. Making nav free was shell-only work. The monitor became ONE page because the user explicitly asked "can procdump just be 1 tab where you do everything". Cost: window grew to 920×780 and the dense power-user fields moved into `dlg_advanced`.
  - **Native Rust collection engine, NOT bundling the PS1.** The PS1 becomes the spec, not a dependency. Keeps the one-exe story, avoids execution-policy/AV problems and two codebases with two authors. Cost: ~3 workflows to port by hand.
  - **Dropped Controller Logs + Integrations.** They are unimplemented stubs in v2.0 of the PS1 ("Planning In Progress", TODO comments) — porting placeholders adds surface with no value.
  - **Dropped "Relaunch as Admin" / non-admin mode.** This exe's release manifest is `requireAdministrator`, so the PS1's whole limited-mode branch is dead code here. Replaced the mode banner with a static "Administrator" pill.
  - **7-day event-log default** (user-approved addition). The PS1 always exported FULL Application+System EVTX, which can be gigabytes; a "Full export" checkbox preserves the old behavior.
  - **Auto-collect-on-dump** (user-approved addition) — the synergy that only exists because the two tools merged: evidence bundle already waiting after a 3am crash.
  - **`write_fields()` split out of `save()`** — because `refresh_preview()` calls it on a throwaway clone. The original `save()` encrypted the typed webhook URL into the discarded clone AND cleared the live field, silently destroying the URL on any trigger edit. This was caught in review before shipping; **do not re-merge these two functions.**
  - **`WS_VSCROLL` injected via `ComboBoxFlags::from_bits_unchecked`.** nwg's `ComboBoxFlags` exposes only VISIBLE/DISABLED/TAB_STOP and its `forced_flags()` is `CBS_DROPDOWNLIST | WS_CHILD | WS_BORDER`; Win32 needs WS_VSCROLL **at creation** for a dropdown scrollbar. Style-patching after creation is unreliable, and `from_bits_unchecked` (bitflags 1.3) is the only way through nwg's typed builder. Applied in the shared `mk_combo` so all combos benefit; Windows only draws the bar when items overflow.
  - **Processes listed before services**, per explicit user instruction, and always shown (previously they were gated behind the "show all" checkbox and appended after 150+ services, making them unreachable). The checkbox is now "Include stopped services". `[System Process]` (PID 0) is filtered — undumpable and it sorted to the top looking like the default pick.

- **What changed in the system:** everything under `rust/src/gui/` (5 wizard page files deleted, 6 files added), `rust/src/collect/` (new, 6 files), `rust/src/{main,cli,config,monitor,bitness}.rs`, `scripts/gui-e2e.ps1` (rewritten for sidebar nav + real collection + log capture), `CLAUDE.md`, the design spec, and the size-gate line in the previous continuation doc.

- **Discussed but NOT implemented:**
  - Full UI polish (the user's next objective; they plan to add design-oriented MCP servers first).
  - A filter/search box or Process↔Service toggle for the target picker (identified as the real fix for "can't find services", deferred to the polish round because it is a design decision).
  - VSS snapshot capture for locked files; Controller Logs / Integrations; configurable auto-collect rate limit.

- **Open threads / unresolved questions:**
  1. **The user's directive: "scroll doesn't work I don't see any services now."** The scroll defect is fixed and verified (0 → 45 rows under real wheel input). What remains is UX: services sit ~130 rows below the processes. **Confirm with the user whether the scroll fix alone resolves their complaint, and get a decision on the picker redesign** (filter box vs. type toggle vs. two dropdowns).
  2. `git push origin` is broken (JCI repo 404). Ask whether to create it / refresh credentials.
  3. Only ~30KB of size headroom remains under the accepted 2.0MB gate — a polish pass that adds controls could breach it.
  4. `auto_bundle` and the CCURE-specific collection paths remain unproven on a real C-CURE server.

### 5. WHAT COULD GO WRONG

- **Known bugs/issues:**
  - Target picker **UX** (not correctness): 278 default entries, services after all processes. Scrolling works but finding a service is tedious. This is the top polish item.
  - `git push origin` fails (see above).
- **Edge cases to watch for:**
  - `gui-e2e.ps1` requires an **idle machine** — synthesized clicks and the real-wheel scroll probe lose to a moving physical mouse.
  - A closed ComboBox always LOOKS empty in a screenshot; adjudicate with `CB_GETCOUNT`/`CB_GETLBTEXT`, never pixels.
  - Status panel treats a heartbeat older than 60s as "not running". The monitor writes one per cycle and every 30s while ProcDump is attached, so this is safe today — but a future longer cycle would make a healthy monitor read as down.
  - `sc.exe` output parsing assumes en-US tokens (documented; C-CURE deployments are en-US).
  - `tar.exe` is absent before Win10 1803 / Server 2019 — there is a `Compress-Archive` fallback, untested on such a box.
- **Technical debt / shortcuts:**
  - The Monitor page is dense by design; power-user fields live behind Advanced.
  - Auto-collect rate limit is a hardcoded 60 minutes (`ponytail:` comment marks it).
  - No VSS: locked files are skipped by robocopy best-effort.
  - `[System Process]` is filtered by a `starts_with('[')` heuristic.
  - Theme brushes/fonts are deliberately leaked (process-lifetime GUI) — intentional, do not "fix".
- **Assumptions that could be wrong (flags for the next AI):**
  - **DO NOT assume a combobox's create-height controls its dropdown size.** Measured: 26 and 300 both yield the same 30-row (572px) list. The lever is `WS_VSCROLL` + `CB_GETMINVISIBLE`.
  - **DO NOT verify scrollability with `CB_SETTOPINDEX`.** It repositions the list even when there is no scrollbar and the wheel is dead — it produced a false green that shipped, and the user found the bug. Use real input (`SetCursorPos` + `mouse_event` wheel) and assert `CB_GETTOPINDEX` advanced.
  - **DO NOT call `MonitorPage::save()` on a throwaway `Config` clone** — use `write_fields()`. `save()` mutates live controls and consumes the typed webhook secret.
  - Do not trust screenshot-only verification verdicts for anything a static image cannot decide.
  - The C# WIP commit is unreviewed safekeeping — do not assume it builds.

### 6. HOW TO THINK ABOUT THIS PROJECT

1. **Core philosophy:** smallest possible native artifact, boring Win32 done correctly. Modernity comes from discipline (grid, typography, DPI correctness, live status instead of blind "done" messages), not frameworks. Verification is empirical and adversarial: build it, click it like a user, and when pixels are ambiguous probe with window messages — but make sure the probe exercises the same mechanism the user does.
2. **Most common newcomer mistake:** creating an nwg control as a local (its HWND is destroyed on drop, so it vanishes); adding physical-pixel math under `high-dpi`; spawning a console child without `CREATE_NO_WINDOW` (freezes the message pump); or calling `save()` where `write_fields()` belongs.
3. **Looks refactorable but is NOT:** the three big event dispatchers in `mod.rs` (nwg's handler model makes distributed handlers messier, and they are load-bearing and reviewed); the `write_fields`/`save` split (it looks like duplication; it is a data-loss guard); `panic = "abort"` (part of the size gate AND the designed crash-recovery model — the Scheduled Task restarts the monitor); the leaked theme brushes/fonts; the dense packed rows on the Monitor page and in `dlg_advanced`.

### 7. DO NOT TOUCH LIST

- Do NOT swap GUI frameworks or add crate dependencies (size gate — only ~30KB headroom).
- Do NOT touch the release manifest's `requireAdministrator` or weaken the `build.rs` release guard.
- Do NOT re-merge `write_fields()` into `save()` in `page_monitor.rs`.
- Do NOT remove the `WS_VSCROLL` injection in `mk_combo()`, and do NOT replace the e2e real-wheel scroll guard with a `CB_SETTOPINDEX` check.
- Do NOT "fix" the leaked brushes/fonts in `theme.rs` or add Drop handling — intentional.
- Do NOT convert logical coordinates to scaled/physical anywhere pages are built.
- Do NOT refactor the `mod.rs` dispatchers without need.
- Do NOT assume the C# WIP commit works.
- Do NOT ship a PowerShell script as part of the product (inline `powershell -Command` is fine).
- Preserve commit-message style, the design system in `docs/plans/gui-redesign-plan.md`, and the approved spec in `docs/superpowers/specs/2026-07-25-log-collector-design.md`.
- Ask before changing the collector's output layout — it mirrors the PS1 so JCI support tooling/expectations still match.

### 8. CONFIDENCE & FRESHNESS

- §1 Identity/constraints — ✅ HIGH (size gate decision made with the user this session)
- §2 Rust app state — ✅ HIGH for the shell, Monitor page, dialogs, collector pages, engine, CLI, scroll fix (all built + executed + e2e-verified this session). ⚠️ MEDIUM for CCURE-specific collection paths (code ran, but no CCURE install exists here). ❓ LOW for `auto_bundle` (never executed) and the C# WIP commit (unreviewed).
- §3 Architecture map — ✅ HIGH (written from the code as built this session)
- §4 Recent work — ✅ HIGH
- §5 Risks — ✅ HIGH for the measured items (dropdown geometry, probe validity, push failure); ⚠️ MEDIUM for the `tar.exe`-absent and locale caveats (reasoned, not observed)
- §6/§7 — ✅ HIGH (derived from this session's reviews and the user's explicit decisions)
