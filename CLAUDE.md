# ProcDumpMonitor — project notes for Claude

Two implementations live here: the legacy C#/.NET app (repo root: `*.cs`) and the
**current** Rust rewrite in `rust/` (single ~1.97MB exe, GUI + CLI). New work goes
in `rust/` unless explicitly about the C# app.

## App shape (mode-based shell, since 2026-07-25)

The GUI is NOT a linear wizard anymore. It's a freely-clickable sidebar shell
(`rust/src/gui/mod.rs`, window 920×780) with two groups:
- **MONITOR → `page_monitor.rs`**: the merged "do everything" page — combined
  Svc:/Proc: target dropdown, dump triggers + output, schedule + notify
  essentials, a live status panel (schtasks + `health.json`, 3s poll timer),
  and footer Create/Run/Stop/Remove/Save/Open/Logs/Copy/Scheduler buttons.
  Power-user ProcDump fields live in `dlg_advanced.rs`; full SMTP in `dlg_smtp.rs`
  (both owned reusable windows, hidden on close). `save()` is the real-persist
  path; `write_fields()` is control-pure for `refresh_preview()` (do NOT call
  `save()` on a throwaway clone — it clears the webhook field).
- **LOG COLLECTOR → `page_datacoll.rs` / `page_installlogs.rs` / `page_syshealth.rs`**:
  ports of the CCURE LogCollector v2.0 PS1's three real tabs, driven by the
  native engine in `rust/src/collect/` (no PowerShell script shipped; shells to
  robocopy / wevtutil / reg.exe / systeminfo / inline `powershell -Command` /
  tar.exe). Collection runs on a worker thread via `collect_runner.rs` +
  `nwg::Notice`. Same engine is the `collect` CLI verb.
- Auto-collect-on-dump: `config.auto_collect_on_dump` → `monitor.rs` hook →
  `collect::pdm_bundle::auto_bundle` (rate-limited 60 min, skipped on low disk).
- Design spec: `docs/superpowers/specs/2026-07-25-log-collector-design.md`.

Dialog gotcha: nwg does NOT `AdjustWindowRectEx` a parented (owned) window, so
its requested size IS the outer size — pad height for the title bar or bottom
controls clip off the client area. Labels take `&` literally (STATIC); only
BUTTON/checkbox captions need `&&` to render one `&`.

## Build / test (Rust)

- Cargo is NOT on PATH: use `%USERPROFILE%\.cargo\bin\cargo.exe`, run from `rust/`.
- `rust/app.manifest` requires Administrator. For anything automated (tests, UI
  automation), build with env `PDM_TEST_MANIFEST=1` → embeds `app.test.manifest`
  (asInvoker, no UAC). build.rs PANICS if that env is set on a `--release` build —
  release exes must keep `requireAdministrator`.
- `cargo test` needs `PDM_TEST_MANIFEST=1` (the admin manifest makes the test exe
  unlaunchable from an unelevated shell).
- Size gate: release exe ≈ 1.97MB (`opt-level=z`, lto, `panic="abort"` — all
  deliberate; grew from 1.86MB when the log-collector subsystem + 2 dialogs landed).

## GUI end-to-end testing

- `scripts\gui-e2e.ps1 -Exe rust\target\debug\ProcDumpMonitor.exe -OutDir <dir>` —
  run under **powershell.exe 5.1** (UIA assemblies). Clicks through the sidebar,
  opens/closes both dialogs, runs a real System Health collection into `<dir>`,
  screenshots every page, captures the app log + run transcript, exits nonzero on
  failure. The machine's mouse/keyboard must be idle during a run: synthesized
  cursor clicks lose to a moving physical mouse.
- nwg controls expose NO UIA patterns (everything is a Pane) — locate elements by
  Win32 class + name; interact via synthesized input, or `BM_CLICK`/`CB_GETCOUNT`
  style window messages for deterministic probing (the script adjudicates the
  target dropdown with CB_GETCOUNT, not screenshots).
- Two e2e traps: `BoundingRectangle.X` is an ABSOLUTE screen coord (subtract the
  window's left edge for sidebar-column filtering), and PowerShell `-eq` is
  case-INSENSITIVE so the "MONITOR" group caption shadows the "Monitor" nav item
  — match nav labels with `-ceq`. Owned dialogs nest UNDER their owner in the UIA
  tree (search Descendants, not root Children).

## nwg (native-windows-gui) rules learned the hard way

- **Control lifetime:** nwg destroys a control's HWND when the Rust value drops.
  Every control created in a page's `build()` — including static Labels — must be
  stored in the page struct. (`nwg::Font` is the exception: it has NO Drop impl.)
- **Paint-before-register:** windows are built hidden and shown only after all
  `theme::register_*` calls (`rust/src/gui/theme.rs`) — statics that first-paint
  before registration keep stale pixels forever.
- **Console children stall the pump:** any `Command` spawn from the GUI must set
  `CREATE_NO_WINDOW` (see `services.rs` / `task.rs`) or the wizard freezes.
- All GUI coordinates are LOGICAL px — nwg's `high-dpi` feature scales positions,
  sizes, and fonts. Never multiply by `scale_factor()` except in raw GDI paint paths.
- The GUI design system (grid, colors, fonts) is specified in
  `docs/plans/gui-redesign-plan.md`; follow it for any layout change.
