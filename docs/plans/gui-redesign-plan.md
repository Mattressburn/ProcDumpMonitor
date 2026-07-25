# GUI Redesign Plan — ProcDump Monitor (Rust/nwg)

## Problem

The Rust rewrite's wizard GUI (native-windows-gui) uses hardcoded 96-DPI pixel
coordinates. At the user's 125% display scaling (AppliedDPI=120), fonts scale but
control rects don't: text is cut off ("Show all servi", "efresh Service"), and the
overall layout is bland default-gray Win32 with no visual hierarchy.

## Goal

Modern, legible, nothing-cut-off wizard. Stay on native-windows-gui (preserves the
~1.9MB single-exe size gate; egui would triple it). Modernize via: correct DPI
scaling, a sidebar-step layout, white content canvas, accent color, real typography,
and a consistent spacing grid.

## Global Constraints

- GUI framework stays `native-windows-gui` 1.0.13 + `native-windows-derive`. No new crate dependencies.
- Every page keeps its existing public interface EXACTLY: same struct name, same public fields (control fields used by `mod.rs` for event handles), same `build(parent, state)` signature, same `load()`/`save()` behavior and semantics. Only layout, positions, sizes, fonts, colors, and helper-text wording may change.
- No business-logic changes. `save()`/`load()` bodies untouched except where a control was renamed/added purely for layout (avoid adding controls except static labels/frames).
- All coordinates in LOGICAL pixels; the `high-dpi` nwg feature handles scaling. No manual `* scale` math in pages.
- Text must never be clipped: size labels generously (at least ~9px per character of text at body size, round up) and prefer full available width for labels on their own row.
- Release profile and size gate unchanged. `panic = "abort"` stays.
- The release manifest keeps `requireAdministrator`. A test manifest (asInvoker) is selected only via `PDM_TEST_MANIFEST=1` env var at build time.
- Rust toolchain: `%USERPROFILE%\.cargo\bin\cargo.exe` (not on PATH). Build from `rust/` dir.
- Commit only files under `rust/`, `scripts/gui-e2e.ps1`, and `docs/plans/`. Never `git add` the modified C# files (Config.cs, ConfigMigrator.cs, TaskSchedulerService.cs) or other stray untracked files.

## Design System (binding spec)

### Window & chrome
- Window: 920×640 logical, centered, fixed size (`WINDOW | VISIBLE` flags as today), title "ProcDump Monitor".
- Left sidebar: 240 wide, full height, background #F3F3F7.
- Content canvas: everything right of sidebar, background WHITE (255,255,255).
- Footer: 1px divider line across the content area at y=560 (color #E1DFDD), nav buttons below it, right-aligned: `< Back` then `Next >` at the far right. Buttons 96×32, 8px gap, ~12px above bottom edge.

### Sidebar
- App title "ProcDump Monitor" at top: Segoe UI Semibold ~18px, positioned (24, 28), single line, sized to fit.
- Subtitle under it: "Setup wizard" muted gray ~13px.
- Six step rows starting y=96, each 40 tall, full sidebar width:
  - Text: `1  Target`, `2  ProcDump`, `3  Task`, `4  Notify`, `5  Review`, `6  About` (two spaces after number), x=24.
  - Current step: bold font, accent color text, plus a 3×24 accent-colored bar flush at x=0 of the row (small Frame or Label with accent background).
  - Other steps: regular font, muted gray text.
  - Rows are NOT clickable (nav stays Back/Next).
- Sidebar bottom: version string small muted text at (24, client_height-36).

### Content header (owned by shell, per page)
- Page title: ~26px Segoe UI Semibold, near-black (#1B1B1B via default), at (272, 32), width to window edge minus 24.
- Subtitle: ~14px muted gray at (272, 68), full remaining width.
- Titles/subtitles per step:
  1. Target — "Choose what to monitor" / "Pick a Windows service or type a process name."
  2. ProcDump — "Configure ProcDump" / "Dump triggers, options, and output location."
  3. Task — "Scheduled task" / "How the monitor runs in the background."
  4. Notify — "Notifications" / "Get an email or webhook alert when a dump is captured."
  5. Review — "Review & install" / "Check the summary, then create or manage the scheduled task."
  6. About — "About" / "Version and build information."

### Page frames
- One frame per page as today, positioned at (240, 100), size (680, 456), no visible border (`FrameFlags` without border if available; else keep border but it must not draw a dark box — verify visually).
- Frame + all child static text backgrounds must render WHITE (theme raw-handler responsibility).

### Layout grid (inside each page frame; coordinates relative to frame)
- `PAD = 32` left/top padding.
- Label column: x=32, width 190. Field column: x=232, width 408 (32+190+10). Right margin ≥ 32.
- Row pitch 40 (dense pages may use 34 minimum). Inputs 26 tall, combos default height, labels 20 tall, vertically offset -2 from input top so baselines align.
- Section headers (for pages with >6 rows): Segoe UI Semibold 15px, full width, 16px extra space above, 8px below.
- Checkboxes/radios: place in field column, full remaining width.
- Hint/help text: muted gray, full content width, placed 8px under its row.
- Multi-line preview/summary text boxes: full width (x=32, width 616).
- Buttons inside pages: min 110 wide, 30 tall; size to fit their caption text generously (nothing clipped).

### Theme module API (`rust/src/gui/theme.rs`, created by Task 1; pages consume it)
```rust
pub const ACCENT: [u8; 3] = [15, 108, 189];      // #0F6CBD
pub const MUTED:  [u8; 3] = [96, 94, 92];        // #605E5C
pub const WHITE:  [u8; 3] = [255, 255, 255];
pub const SIDEBAR_BG: [u8; 3] = [243, 243, 247]; // #F3F3F7
pub const DIVIDER:    [u8; 3] = [225, 223, 221]; // #E1DFDD

pub fn body_font() -> &'static nwg::Font;      // Segoe UI 15px
pub fn semibold(size: u32) -> nwg::Font;       // Segoe UI Semibold, given px size
pub fn title_font() -> &'static nwg::Font;     // Segoe UI Semibold 26px
pub fn subtitle_font() -> &'static nwg::Font;  // Segoe UI 14px

// Text-color registry (single raw WM_CTLCOLORSTATIC handler per parent):
pub fn register_muted(h: &nwg::ControlHandle);   // gray text on white
pub fn register_accent(h: &nwg::ControlHandle);  // accent text on white
// Attach white-canvas painting + text-color handling to a parent (window or frame):
pub fn attach(parent: &nwg::ControlHandle);
```
Implementation notes (Task 1): thread_local registries of HWNDs; `nwg::bind_raw_event_handler` on the window AND on each page frame handling `WM_CTLCOLORSTATIC` (SetBkMode TRANSPARENT / SetBkColor white, SetTextColor from registry, return white brush) and `WM_ERASEBKGND` (fill white; sidebar area on the main window filled SIDEBAR_BG). Static brushes via `CreateSolidBrush`, leaked once (GUI lives for process lifetime). Checkboxes with visual styles paint from parent theme; verify visually in screenshots that no gray boxes remain behind checkbox/radio text on white pages.

### Fonts & DPI
- Enable nwg cargo feature `high-dpi`. VERIFY in the vendored nwg source (`%USERPROFILE%\.cargo\registry\src\...\native-windows-gui-1.0.13\src\win32\high_dpi.rs`) whether builder positions/sizes AND font sizes are auto-scaled; add `nwg::scale_factor()` compensation ONLY where the source shows no auto-scaling. No double scaling.
- Global default font: Segoe UI 15px via `Font::set_global_default` (replaces bare `set_global_family` call).
- Manifest keeps `dpiAware=true` (system-DPI aware; matches nwg high-dpi's model).

## Verification harness

- `rust/app.test.manifest`: identical to `app.manifest` but `level="asInvoker"` (so the exe launches without UAC under automation). `build.rs` selects it when env `PDM_TEST_MANIFEST=1`; also `println!("cargo:rerun-if-env-changed=PDM_TEST_MANIFEST")`.
- `scripts/gui-e2e.ps1`: PowerShell 7 driver that (a) launches the given exe, (b) uses System.Windows.Automation UIA to find the "ProcDump Monitor" window, (c) walks the wizard: screenshot page 1, type a process name ("notepad"), toggle "Show all services", click Refresh, click Next, screenshot page 2, … through all 6 pages, then Back to page 1 to prove reverse nav, screenshotting every state to a given output dir (Graphics.CopyFromScreen of the window rect, PNG named `NN-pagename.png`), (d) closes the app. Parameters: `-Exe <path> -OutDir <dir>`. Prints each action; exits nonzero if a UIA element it needs is not found.

## Tasks

### Task 1: Foundation — DPI, theme module, wizard shell
Files: `rust/Cargo.toml`, `rust/app.manifest` (only if needed), `rust/app.test.manifest` (new), `rust/build.rs`, `rust/src/gui/theme.rs` (new), `rust/src/gui/mod.rs`.
- Add `high-dpi` to nwg features (verify scaling behavior per Fonts & DPI above).
- Add test-manifest switch to build.rs per Verification harness.
- Create `theme.rs` implementing the exact API in the Design System.
- Rewrite `mod.rs` shell per Design System: 920×640 window, sidebar with step list + active indicator, page title/subtitle labels (shell updates text + moves accent bar on nav), frames at (240,100) 680×456, footer divider + Back/Next. Keep ALL existing event wiring, nav/save/load ordering, and page interfaces untouched. Remove the old "Step X of 6" label (sidebar + title replace it).
- Sidebar step rows: implement active/inactive as two prebuilt fonts + set_text_color via theme registries (re-register or swap on nav; simplest correct approach wins).
- Must compile: `cargo check` clean (warnings OK). Pages will still use old coordinates — that's fine this task; do NOT touch page files.

### Task 2: Target page layout (`rust/src/gui/page_target.rs`)
Apply the Design System grid. Label "Process name" + input; "Or pick a service" + combo; checkbox "Show all services (including stopped)" and button "Refresh services" on one row in the field column; the existing hint sentence as muted hint text. Generous widths — nothing clipped at 125% or 150% scaling. Keep struct/public API identical.

### Task 3: ProcDump page layout (`rust/src/gui/page_procdump.rs`)
Apply the grid. This is the dense page: use section headers to group (e.g. "Scenario", "Triggers", "Output"), 34–40 row pitch, keep every existing control and its public field. Preview/command text full-width. Keep struct/public API identical.

### Task 4: Task page layout (`rust/src/gui/page_task.rs`)
Apply the grid. Section header(s) if useful. Keep struct/public API identical.

### Task 5: Notify page layout (`rust/src/gui/page_notify.rs`)
Apply the grid. Sections: "Email" (checkbox + SMTP fields + Validate/Test buttons) and "Webhook" (checkbox + URL). Keep enable/disable toggle behavior. Keep struct/public API identical.

### Task 6: Review page layout (`rust/src/gui/page_review.rs`)
Apply the grid. Summary area full-width at top; action buttons in aligned rows (grouped: primary task actions row — Create/Update, Run, Stop, Remove; secondary row — Save config only, Open dump folder, View logs, Copy args, Task Scheduler). All captions fully visible. Keep struct/public API identical.

### Task 7: About page layout (`rust/src/gui/page_about.rs`)
Apply the grid. Logo + name + version centered-ish in content area, muted detail lines. Keep struct/public API identical.

### Task 8: Integration build + E2E driver
- Fix any compile errors from Tasks 2–7 (`cargo check` then `cargo build` with `PDM_TEST_MANIFEST=1`).
- Write `scripts/gui-e2e.ps1` per Verification harness.
- Run it against the test build; produce the full screenshot set. All 6 pages + back-nav shot must be captured.

### Task 9: Visual verification & fix loop
Verifier agents read every screenshot and hunt: clipped/truncated text, overlapping controls, rows off-grid, gray boxes on white canvas, sidebar state wrong, anything unreadable. Fix wave per finding set, re-run e2e, re-verify. Loop until clean (max 3 rounds; report residue).

### Task 10: Finalization
- `cargo test` (workspace tests must pass).
- Release build WITHOUT test manifest; confirm exe size ≤ 2.5MB (size gate headroom; current 1.86MB).
- Final whole-branch code review; fix Critical/Important; commit(s) on branch `rust-gui-redesign`.
