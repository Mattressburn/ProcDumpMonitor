# PROJECT CONTINUATION DOCUMENT
## Session — 23 July 2026

### 1. PROJECT IDENTITY

- **Project Name:** ProcDumpMonitor (Rust port)
- **What This Project Is:** A Windows utility that configures Sysinternals ProcDump as an unattended SYSTEM Scheduled Task for crash-dump monitoring, with a GUI setup wizard and email/webhook notifications. For support engineers who drop one exe on a customer machine (e.g. C•CURE deployments).
- **Primary Objective:** Replace the heavy C# .NET 8 WinForms app with a **lightweight single-exe** the field can drop-and-run. **Achieved:** 1.86 MB single native Windows exe, no runtime install (was 70–150 MB self-contained .NET).
- **Strategic Intent:** Frictionless field deployment — one small exe, no .NET runtime prerequisite, fast startup.
- **Hard Constraints (non-negotiable, do not change without asking):**
  - Windows 10 / **Server 2016** floor → `IsWow64Process2` MUST stay resolved via `GetProcAddress` (static import fails to load on Server 2016); no WebView2/Tauri.
  - Task registration via `schtasks.exe /Create /XML` **only, never Task Scheduler COM**. Task XML: principal `<UserId>S-1-5-18</UserId>` + `<RunLevel>HighestAvailable</RunLevel>`, **no `<LogonType>` element** (schtasks rejects `ServiceAccount`), file written **UTF-16LE with BOM**. (Proven by live spike.)
  - DPAPI **LocalMachine** scope (GUI encrypts as elevated user, SYSTEM monitor decrypts).
  - config.json field names match the C# app exactly (a V3 .NET config loads field-for-field).
  - No async runtime (blocking `SmtpTransport` + `ureq`); release profile `panic = "abort"`.
  - Cut features — do NOT reimplement without asking: one-shot, self-test, support-diagnostics ZIP, config export/import/migration, themes.

### 2. WHAT EXISTS RIGHT NOW

- **Built and working (verified this session):**
  - Full Rust crate in `rust/`, merged to `main` (merge commit `01a0984`). 41 tests green (Linux + VM, warning-free).
  - **Headless core** — VM-verified end-to-end (Task 8): registers a real SYSTEM boot task from XML, captured a live notepad dump, wrote `health.json` with correct dedup, clean teardown. CLI verbs: `monitor/install/uninstall/start/stop/status/version/help` (leading dashes optional), exit codes 0/1/2, elevation via embedded `requireAdministrator` manifest.
  - **Full 6-page nwg GUI wizard** (Target/ProcDump/Task/Notify/Review/About) — compiles, builds, and launches on the VM (process stays alive, message loop responding).
  - Email (lettre/rustls), webhook (ureq MessageCard), DPAPI LocalMachine password+webhook encryption, 32/64-bit procdump auto-select, retention, dump-stability gate, disk guard, health heartbeat, rotating logger.
- **Partially built / verified-incomplete:**
  - **GUI visual/interaction correctness is NOT verified.** Only build + launch-stays-alive was checked (SSH is Session 0, can't click). Needs a human **RDP acceptance walkthrough** (see §4).
- **Broken or blocked:** None known.
- **Not started:**
  - Deletion of the old C# sources (deliberately deferred to user).
  - Optional tidies: prune README "Expansion feasibility" section (stale C#-era prose); drop `lettre`'s unused `pool` feature.
  - Push to GitHub `origin` (user chose local merge; not pushed).

### 3. ARCHITECTURE & TECHNICAL MAP

- **Tech stack:** Rust 2021, MSVC target. Crates: `native-windows-gui`(+derive, image-decoder), `serde`/`serde_json`, `chrono`, `base64`, `lettre`(rustls), `ureq`, `windows` 0.58 (feature-gated), `winresource`(build). One binary = both GUI (no args) and CLI (verbs).
- **Key files (`rust/src/`):** `config.rs` (serde model, C#-compatible + tolerant `TargetType` reader), `paths.rs`, `procdump.rs` (arg builder + 5 scenario presets), `task.rs` (task XML gen + `schtasks` wrappers), `monitor.rs` (the loop), `notify.rs` (email/webhook + bounded panic-isolated queue), `secrets.rs` (DPAPI), `bitness.rs` (`IsWow64Process2` via GetProcAddress), `services.rs` (`sc query` parser), `cli.rs`, `logger.rs`/`health.rs`/`retention.rs`/`stability.rs`/`diskguard.rs`, `gui/` (mod.rs shell + 6 page_*.rs). `build.rs` + `app.manifest` embed icon + requireAdministrator.
- **End-to-end flow:**
  1. GUI wizard writes `config.json` (DPAPI blobs for secrets) next to the exe.
  2. GUI Review page "Create Task" shells out to the exe's own `install` verb → `task::install` generates Task XML (UTF-16LE BOM, SYSTEM principal) → `schtasks /Create /XML`.
  3. At boot the task runs `ProcDumpMonitor.exe --monitor --config <path>` as SYSTEM.
  4. Monitor loop per cycle: disk guard → retention → launch procdump (bitness-selected binary, raw arg string, CREATE_NO_WINDOW) → detect newest new `.dmp` → stability gate → notify (email/webhook, deduped) → write `health.json` → interruptible sleep.
- **Naming/standards:** C#-parity JSON field names; pure-logic modules `cargo test` on Linux, Windows-API modules `#[cfg(windows)]` tested on the VM.
- **External deps:** Sysinternals `procdump.exe`/`procdump64.exe` (placed beside the exe); Windows `schtasks.exe`, `sc.exe`, DPAPI.

### 4. RECENT WORK — WHAT JUST HAPPENED (HIGH PRIORITY)

- **Worked on:** The entire rewrite, this session, end to end — brainstorm → design spec → 12-task implementation plan → subagent-driven execution (fresh implementer + independent reviewer per task, fix loops) → final Opus whole-branch review → merged to `main`.
- **Decisions and WHY:**
  - **Rust + nwg, built on the win11-lab VM** (not cross-compiled from Linux): a Win32-GUI+DPAPI+schtasks app can't be run or seen on Linux; nwg gives a tiny native binary matching the old WinForms feel. egui was the documented fallback if nwg failed (it didn't).
  - **schtasks-XML over Task Scheduler COM:** spiked live on the VM *before* the spec committed — dropped the whole COM dependency. Gotchas (no LogonType, UTF-16LE BOM) were discovered empirically and are now in the code.
  - **Split headless core from GUI, core first:** the core is what runs on customers and is CLI-testable; GUI built last as a thin front-end that shells its own verbs (one code path).
  - **`panic = "abort"` kept** despite nullifying the notify queue's `catch_unwind`: Task Scheduler restart-on-failure (1 min ×999) is the recovery net, and abort meaningfully shrinks the binary. Documented in README.
  - **Merged locally, NOT pushed:** user explicitly chose local merge; pushing is theirs to trigger.
- **What changed in the system:** New `rust/` crate + `scripts/vm.sh`/`vm-build.sh` + `docs/superpowers/` spec & plan, all merged to `main`. C# sources untouched.
- **Discussed but NOT implemented:** Deleting C# sources; the two optional tidies; pushing to origin; the all-in-one "diagnostics hub" expansion (still hypothetical, only in README prose).
- **Open threads:**
  1. **RDP GUI acceptance walkthrough** — RDP `192.168.69.110` (pw `<redacted-rotate-me>`). Confirm: 6-page nav; scenario presets apply + live effective-command matches; editing an option flips scenario to "Custom"; **Browse click updates preview AND flips to Custom**; bitness label; Task page exists/new branch; Notify bad-email modal blocks Create Task; Review actions (Create/Run/Stop/Remove/Open Dumps/View Logs/Copy/Task Scheduler); About logo + build date render.
  2. C# source deletion decision.
  3. Push to origin?

### 5. WHAT COULD GO WRONG

- **Known behavioral limitation (faithful C# parity, documented in README):** one notification per monitor cycle even when a preset writes several dumps (`-n 3` presets: High CPU spike, Memory threshold) — only the newest `.dmp` triggers a notify; `TotalDumpCount` counts capturing-cycles, not dump files. All dumps are still written/retained.
- **Edge cases:** hand-editing `config.json` with a bad field resets the WHOLE config to defaults (C# parity) → empty dump dir → `create_dir_all("")` fails → monitor exits → Task Scheduler restart loop. Machine-generated configs are fine.
- **Tech debt / shortcuts (all reviewed acceptable-to-ship):** `schtasks /End` (Stop Task) hard-kills the monitor and can orphan an in-flight `procdump.exe` (parity; Job Object is the future fix); fixed temp filename `pdm_task.xml` (concurrent-install race, non-workflow); `FOO.EXE` uppercase-extension process misses bitness match → safe pd64 default; `logger::init` uses `lock().unwrap()` (runs once, pre-contention).
- **Assumptions that could be wrong:** GUI *renders and behaves* correctly (only construction verified — the RDP walk is the real test); `sc query`/schtasks output tokens are en-US (C•CURE is en-US); the VM's Windows build (28000) behaves like the Server 2016 floor for the GetProcAddress path (logic is correct by construction but not tested on actual Server 2016).

### 6. HOW TO THINK ABOUT THIS PROJECT

1. **Core pattern:** one exe, two faces (GUI on no-args, CLI on verbs), with a **headless core the GUI drives by shelling its own CLI verbs**. Chosen so the thing that runs on customers is small, CLI-testable, and has exactly one code path per operation. Platform split: pure logic tests on Linux, Windows-API behind `#[cfg(windows)]` tested on the VM.
2. **Most common mistake a newcomer makes:** "fixing" the schtasks Task XML — re-adding `<LogonType>`, changing the encoding off UTF-16LE+BOM, or reaching for the `windows` crate's Task Scheduler COM. All three break SYSTEM-task registration; the current form was empirically proven. Second most common: assuming `cargo build` works on Linux — it doesn't for the GUI/monitor (Windows-only); use `scripts/vm-build.sh`.
3. **Looks refactorable but is intentionally NOT:** the procdump **flag-assembly order** in `procdump.rs` (mirrors C# `BuildProcDumpArgs` exactly — order is a contract); the DPAPI "keep existing blob when the field is empty" dance in the Notify page (prevents silently wiping a saved password); the per-page `if cur==N {save}` / `if next==N {load}` nav dispatch (verbose but additive — a trait abstraction would buy nothing for 6 pages).

### 7. DO NOT TOUCH LIST

- Do NOT re-add `<LogonType>` to the Task XML, change it off UTF-16LE+BOM, or replace `schtasks` with Task Scheduler COM.
- Do NOT convert `IsWow64Process2` to a static import (breaks Server 2016 load).
- Do NOT change DPAPI scope off LocalMachine (SYSTEM monitor won't decrypt).
- Do NOT reorder `procdump::build_args` flags or rename config.json fields (C# parity contracts).
- Do NOT reimplement the cut features (one-shot, self-test, support-diagnostics, export/import, migration, themes) without asking.
- Do NOT delete the C# sources unless asked.
- Do NOT push to `origin` without explicit instruction (user chose local merge).
- Do NOT add new crates/dependencies without asking (no-async-runtime constraint).
- Preserve naming conventions; maintain the documented tradeoffs (`panic=abort`, newest-dump-per-cycle).

### 8. CONFIDENCE & FRESHNESS

- §1 Identity / constraints — ✅ HIGH (set this session)
- §2 What exists — ✅ HIGH for headless core + build/test; ⚠️ MEDIUM for GUI (build+launch only, not visual/interaction)
- §3 Architecture — ✅ HIGH (built + reviewed this session)
- §4 Recent work — ✅ HIGH (this session)
- §5 What could go wrong — ✅ HIGH (from per-task + whole-branch reviews); ❓ LOW on actual-Server-2016 behavior (not tested on that OS)
- §6 How to think — ✅ HIGH
- §7 Do not touch — ✅ HIGH
