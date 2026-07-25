# ProcDumpMonitor — project notes for Claude

Two implementations live here: the legacy C#/.NET app (repo root: `*.cs`) and the
**current** Rust rewrite in `rust/` (single ~1.86MB exe, GUI + CLI). New work goes
in `rust/` unless explicitly about the C# app.

## Build / test (Rust)

- Cargo is NOT on PATH: use `%USERPROFILE%\.cargo\bin\cargo.exe`, run from `rust/`.
- `rust/app.manifest` requires Administrator. For anything automated (tests, UI
  automation), build with env `PDM_TEST_MANIFEST=1` → embeds `app.test.manifest`
  (asInvoker, no UAC). build.rs PANICS if that env is set on a `--release` build —
  release exes must keep `requireAdministrator`.
- `cargo test` needs `PDM_TEST_MANIFEST=1` (the admin manifest makes the test exe
  unlaunchable from an unelevated shell).
- Size gate: release exe ≈ 1.86MB (`opt-level=z`, lto, `panic="abort"` — all deliberate).

## GUI end-to-end testing

- `scripts\gui-e2e.ps1 -Exe rust\target\debug\ProcDumpMonitor.exe -OutDir <dir>` —
  run under **powershell.exe 5.1** (UIA assemblies). Launches the wizard, drives it
  with real mouse/keyboard input, screenshots every page, exits nonzero on failure.
  The machine's mouse/keyboard must be idle during a run: synthesized cursor clicks
  lose to a moving physical mouse.
- nwg controls expose NO UIA patterns (everything is a Pane) — locate elements by
  Win32 class + name; interact via synthesized input, or `BM_CLICK`/`CB_GETCOUNT`
  style window messages for deterministic probing.

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
