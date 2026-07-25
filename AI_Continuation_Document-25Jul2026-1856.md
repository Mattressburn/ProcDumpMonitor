# PROJECT CONTINUATION DOCUMENT
## Session 3 — 25 July 2026

### 1. PROJECT IDENTITY

- **Project Name:** ProcDumpMonitor
- **What This Project Is:** A Windows utility that watches a target process or service and captures ProcDump crash/hang/resource dumps automatically, with retention policies and email/webhook notifications. Configured through a 6-step GUI wizard that installs a Scheduled Task running the monitor.
- **Primary Objective:** A single small (~1.86MB) self-contained Rust exe that replaces the legacy C# app: monitor + CLI + setup-wizard GUI, reliable on real deployments (C-CURE / en-US environments).
- **Strategic Intent:** Kill the .NET runtime dependency and installer complexity; one exe a field tech can drop on a box, run elevated, and configure in under a minute.
- **Hard Constraints:**
  - GUI stays `native-windows-gui` 1.0.13 (NOT egui/iced — binary size gate ~1.9MB, `opt-level=z`, lto, `panic="abort"`).
  - Release manifest must keep `requireAdministrator` (build.rs panics if `PDM_TEST_MANIFEST=1` on a release build).
  - All GUI coordinates LOGICAL px (nwg `high-dpi` feature scales everything); the design system in `docs/plans/gui-redesign-plan.md` is binding for layout work.
  - Rust toolchain: `%USERPROFILE%\.cargo\bin\cargo.exe` (not on PATH), run from `rust/`.

### 2. WHAT EXISTS RIGHT NOW

- **Built and working (✅ verified this session):**
  - Full modern GUI redesign merged to `main` (merge commit `52b75ff`): sidebar-step wizard shell, white canvas + accent theming (`rust/src/gui/theme.rs`), all 6 pages on a shared layout grid, DPI-correct at 125% (and by construction at 150%+).
  - Wizard is functionally verified end-to-end by automation: typing, checkbox toggles, service-list refresh (149 running → 312 with show-all, probed via CB_GETCOUNT), full forward/back navigation.
  - 43/43 cargo tests pass; release exe rebuilt from merged main: 1.86MB, admin manifest confirmed embedded.
  - `scripts/gui-e2e.ps1`: reusable UI-automation driver (real mouse/keyboard input, screenshots every page, exit-code semantics). Run under powershell.exe 5.1 with the machine's mouse idle.
  - Monitor/CLI/notify/retention core from the earlier rust-rewrite sessions (untouched this session, still passing its tests).
- **Partially built:**
  - Legacy C# side has uncommitted-then-committed WIP (see §4): Config.cs/ConfigMigrator.cs/TaskSchedulerService.cs changes + `tests/ProcDumpMonitor.Tests/TaskSchedulerServiceTests.cs` + `scripts/Watch-Dump.ps1` — from a session before this one, state not re-verified.
- **Broken or blocked:** Nothing known.
- **Not started yet:** The log-gathering tool the user wants next session (no spec yet). Pushing `main` to a remote was offered but not requested until wrap-up (now done — see §4).

### 3. ARCHITECTURE & TECHNICAL MAP

- **Tech stack:** Rust (stable-msvc 1.97), native-windows-gui 1.0.13 + windows crate, serde/chrono/lettre/ureq; winresource for icon+manifest; MSVC 2019 Build Tools linker. Legacy C#/.NET app still in repo root.
- **Key files:**
  - `rust/src/gui/mod.rs` — wizard shell: window, sidebar, header, frames, ONE event dispatcher (`full_bind_event_handler`) wiring every page's controls.
  - `rust/src/gui/theme.rs` — colors/fonts + raw WM_CTLCOLORSTATIC / WM_ERASEBKGND handlers (white canvas, gray sidebar, accent bar). Thread-local registries; window built hidden, shown after registration (first-paint ordering matters).
  - `rust/src/gui/page_*.rs` — six pages; each keeps a frozen public struct API (mod.rs holds control handles) and stores EVERY created control in the struct (nwg drops = HWND destroyed).
  - `rust/src/{monitor,procdump,task,services,notify,retention,...}.rs` — core logic; `services.rs`/`task.rs` spawn `sc.exe`/`schtasks` with `CREATE_NO_WINDOW` (GUI pump freezes without it).
  - `rust/build.rs` — BUILD_DATE, icon/manifest embed, `PDM_TEST_MANIFEST=1` → asInvoker test manifest (panics on release).
  - `scripts/gui-e2e.ps1` — e2e driver; `docs/plans/gui-redesign-plan.md` — binding design system; `.superpowers/sdd/progress.md` — session ledger.
- **End-to-end flow:** 1) User runs exe elevated → wizard. 2) Target page picks process/service → 3) ProcDump page sets dump triggers/paths (preset scenarios) → 4) Task page shows the schtasks command → 5) Notify page email/webhook → 6) Review page Create/Run/Stop/Remove task + save config JSON → installed Scheduled Task runs `ProcDumpMonitor.exe monitor`, which watches the target, launches procdump on triggers, applies retention, sends notifications.
- **Naming conventions:** commit prefix `gui-redesign:`/`rust-gui:` this branch; pages `page_<step>.rs`; logical-px constants PAD/FIELD_X/ROW_H per design system.
- **External dependencies:** `sc.exe`, `schtasks.exe`, procdump.exe/procdump64.exe (user-supplied path), SMTP server / webhook endpoints.

### 4. RECENT WORK — WHAT JUST HAPPENED (HIGH PRIORITY)

- **What was worked on:** Complete GUI redesign + verification loop, executed via subagent-driven development under ultracode (13 commits `b53db59..c923089`, merged as `52b75ff`). Installed the Rust toolchain on this machine (it had none — the rewrite was built elsewhere).
- **Key decisions and WHY:**
  - **Stayed on nwg instead of egui** — user's size gate (1.86MB single exe) would triple; "modern" achieved via DPI fix + sidebar shell + theming instead.
  - **nwg `high-dpi` feature + logical coordinates everywhere** — verified against vendored nwg source that positions/sizes/fonts all auto-scale; only raw GDI FillRect paths scale manually.
  - **`PDM_TEST_MANIFEST=1` asInvoker test build** — UI automation can't click UAC prompts; release keeps requireAdministrator and build.rs now panics if the env leaks into a release build (final-review fix).
  - **Real-input e2e driver** (nwg exposes zero UIA patterns — everything is a Pane) — locate via class+name, drive via synthesized mouse/keyboard; deterministic probes use BM_CLICK/CB_GETCOUNT window messages.
- **Root causes fixed (worth knowing forever):**
  1. Text cutoff = 96-DPI hardcoded coords at 125% scaling (the original complaint).
  2. Field labels were INVISIBLE since the rewrite — created as locals, dropped at end of build(), nwg destroys HWND on Drop. Rule now in CLAUDE.md + design spec.
  3. Wizard froze on nav — `schtasks`/`sc.exe` spawned without `CREATE_NO_WINDOW` from a console-less GUI stalls the message pump for seconds per call.
  4. Sidebar theming failed for early-painted statics — window was created VISIBLE so statics first-painted before color registration; now built hidden, shown after.
- **What changed in the system:** everything under `rust/src/gui/`, `rust/build.rs`, `rust/app.test.manifest` (new), `rust/services.rs`/`task.rs` (spawn flags), `scripts/gui-e2e.ps1` (new), `docs/plans/gui-redesign-plan.md` (new), `CLAUDE.md` (new, wrap-up). Wrap-up also committed pre-session C# WIP (Config/ConfigMigrator/TaskSchedulerService + scheduler tests + Watch-Dump.ps1) for safekeeping — that work was NOT reviewed this session.
- **Discussed but NOT implemented:** wordier captions on the dense ProcDump page (user hasn't objected to terse "Incl (-f):" style); async Review-page task actions (worker thread + nwg::Notice) if the brief schtasks stall ever annoys.
- **Open threads:** user's next-session directive: "iron some things out" (unspecified — ask what) + "incorporate a log gathering tool" (no spec yet — likely: collect app log + dumps + task info into a bundle for support; needs requirements).

### 5. WHAT COULD GO WRONG

- **Known bugs/issues:** none open in the Rust app. The C# WIP commit is unreviewed/unverified (may not even build) — it was committed only to avoid data loss.
- **Edge cases to watch:**
  - e2e driver: machine must be unattended (physical mouse movement swallows synthesized clicks); a closed ComboBox always LOOKS empty — screenshot verifiers false-positive on it, adjudicate with CB_GETCOUNT.
  - Windows' separate "Text size" accessibility multiplier could clip the dense ProcDump page's ~7px/char labels (verified safe at 125% display scaling only).
  - `sc.exe` output parsing assumes en-US tokens (documented in services.rs; C-CURE deployments are en-US).
- **Technical debt / shortcuts:** ProcDump page abandons the strict single field column for packed multi-control rows (documented, reviewed, accepted); Review-page task actions are synchronous on the UI thread (brief stall, no longer a freeze); theme brushes/fonts deliberately leaked (process-lifetime GUI).
- **Wrong-assumption flags:** don't trust screenshot-only verification verdicts (two panel runs produced garbage when a stringified `args` gave agents `undefined` paths — they "found" stale screenshots and reported confidently wrong findings); a local shell hook can inject log lines into `>`-redirected files (write critical files with the Write tool).

### 6. HOW TO THINK ABOUT THIS PROJECT

1. **Core philosophy:** smallest-possible native artifact, boring Win32 done correctly. Modernity comes from discipline (grid, typography, DPI correctness), not frameworks. Verification is empirical: build it, click it like a user, screenshot it, probe it with window messages when pixels are ambiguous.
2. **Most common newcomer mistake:** creating an nwg control as a local (it vanishes — HWND destroyed on Drop), or adding physical-pixel math (double-scaling under high-dpi), or spawning a console child without CREATE_NO_WINDOW (freezes the GUI).
3. **Looks-like-it-needs-refactoring but DON'T:** the single giant event dispatcher in mod.rs (one match, all pages) — nwg's handler model makes distributed handlers messier, and the current shape is load-bearing and reviewed; the ProcDump page's packed rows — 29 controls in a 456px frame won't fit the pretty single-column grid; `panic="abort"` — part of the size gate AND the designed crash-recovery model (Scheduled Task restarts the monitor).

### 7. DO NOT TOUCH LIST

- Do NOT swap GUI frameworks or add dependencies (size gate).
- Do NOT touch the release manifest's `requireAdministrator` or weaken the build.rs release guard.
- Do NOT "fix" the leaked brushes/fonts in theme.rs or add Drop handling — intentional.
- Do NOT convert logical coordinates to scaled/physical anywhere pages are built.
- Do NOT refactor the mod.rs dispatcher or page struct APIs without need — mod.rs holds copies of control handles; page public fields are a frozen contract.
- Do NOT assume the C# WIP commit works — it's unreviewed safekeeping.
- Preserve commit-message style and the design system in `docs/plans/gui-redesign-plan.md`.

### 8. CONFIDENCE & FRESHNESS

- §1 Identity/constraints — ✅ HIGH (enforced/verified this session)
- §2 Rust app state — ✅ HIGH (built, tested, e2e-verified this session); C# WIP state — ❓ LOW (committed blind)
- §3 Architecture map — ✅ HIGH for gui/build/e2e; ⚠️ MEDIUM for monitor/notify/retention internals (from prior sessions, tests pass but not re-read)
- §4 Recent work — ✅ HIGH
- §5 Risks — ✅ HIGH (each item empirically observed this session except the accessibility-text-size caveat, which is ⚠️ reasoned)
- §6/§7 — ✅ HIGH (derived from this session's reviews)
