# Bitness Correctness + UI Polish — Design

**Date:** 2026-07-26
**Status:** Approved for planning
**Supersedes nothing.** Extends `2026-07-25-log-collector-design.md`.

## Problem

Two independent problems, discovered in the same session.

### A. The monitor picks the wrong ProcDump binary for C·CURE targets

The real monitoring targets are the Software House processes:

```
SoftwareHouse.CrossFire.ImportWatcherService.exe
SoftwareHouse.CrossFire.ReportServerService.exe
SoftwareHouse.CrossFire.Server.exe
SoftwareHouse.CrossFire.ServerComponentFramework.exe
SoftwareHouse.NextGen.Client.AdminWorkstation.exe        <- 32-bit
SoftwareHouse.NextGen.Client.MonitoringStation.exe       <- 32-bit
SoftwareHouse.NextGen.HardwareInterface.Nantucket.*.exe
SoftwareHouse.NextGen.iSTAR_DriverService.exe
```

The client and monitoring station are **32-bit**. A 64-bit ProcDump attached to a
WOW64 process captures the 64-bit view; the 32-bit managed stacks a support
engineer actually needs require the 32-bit dump. Binary selection must match
target bitness.

`bitness::select_binary` already implements the correct rule (X86 ->
`procdump.exe`, X64 -> `procdump64.exe`, with fallbacks and warnings) and has 6
passing unit tests. **Every input feeding it is wrong**, in three distinct ways:

1. **Service targets are never detected.** `bitness::detect()` matches Toolhelp
   *exe names*, but for `TargetType::Service` the config holds the *service*
   name (`procdump -service CrossFire`). `services.rs` has no service->PID
   resolution — only `sc.exe query` for listing. So a service name never matches
   an exe, yielding `Bitness::Unknown` -> **`procdump64.exe`, unconditionally**.

2. **`-w` guarantees the wrong answer.** `wait_for_process` defaults to `true`
   (`config.rs:120`), so the designed workflow is *arm the monitor before the
   process exists*. But `detect()` runs once at monitor startup
   (`monitor.rs:42`), when the target is by definition not running. Unknown ->
   `procdump64.exe`.

3. **The choice is never revisited.** `detect()` is called exactly twice in the
   codebase — `monitor.rs:42` and the GUI preview `page_monitor.rs:485`. The
   cycle loop re-launches ProcDump every cycle from a `cfg.proc_dump_path`
   frozen at startup, so a target that appears later never corrects it.

### B. Measured UI defects

Probed against the running debug build (not inferred):

| Defect | Evidence |
|---|---|
| Footer buttons unreachable | window outer height 1022px vs work area 1020px; window top at y=53 => bottom **55px below the work area**. `Create/Run/Stop/Remove` is sliced; the second footer row (`Save/Open/Logs/Copy/Scheduler`) is entirely off-screen. |
| Services buried | 122 `Proc:` entries then 151 `Svc:` entries; first service at **index 122** — 5 wheel-pages down. |
| Picker blank on load | `CB_GETCURSEL = -1`, and `CBS_DROPDOWNLIST` cannot render placeholder text. |
| Duplicate service naming | `Svc: AppX Deployment Service (AppXSVC) (AppXSvc)` |
| Misplaced warning | `No ProcDump binary found...` renders directly above **Dump type** and reads as its label. |
| Cryptic numeric row | `CPU% / Low% / Dur / Max:` — four unlabeled boxes. |
| Weak status semantics | Monitor status rows use `o` glyphs with no color. |

**Root cause of the footer clipping:** nwg's `center(true)` centers the *client*
height (780 logical) against the *screen* height (864 logical), ignoring both the
title bar and the taskbar. Top lands at 42 logical / 53 physical; true outer
height is ~818 logical, overshooting the 816-logical work area by ~44 logical.
So ~53px is a **placement** bug and only ~2px is genuine oversize.

## Non-Goals

- Dark mode. The binding design system (`docs/plans/gui-redesign-plan.md`) is
  light; a dark lock is a large diff against ~30KB of binary headroom.
- Supporting screens below ~850 logical px tall (1024x768 consoles). Decided:
  target 1080p and up. Revisit if field evidence contradicts.
- VSS capture, Controller Logs / Integrations collectors — unchanged, still out.
- Any new crate dependency.

## Design

### 1. Bitness resolution (highest priority — correctness)

All additions land in `rust/src/bitness.rs`. No new files.

```rust
/// Read IMAGE_FILE_HEADER.Machine from a PE on disk.
/// DOS header e_lfanew @0x3C -> seek -> verify "PE\0\0" -> Machine @+4.
pub fn pe_machine(path: &Path) -> Option<u16>;

/// 0x014C -> X86, 0x8664 | 0xAA64 -> X64, else Unknown.
pub fn bitness_from_pe(path: &Path) -> Bitness;

/// Process: cfg.target_path if usable, else QueryFullProcessImageName if running.
/// Service:  reg.exe query HKLM\SYSTEM\CurrentControlSet\Services\<name> /v ImagePath
///           -> strip quotes, strip trailing args, expand \??\ and env vars.
///           -> resolves to svchost.exe? return None (shared host; PE says nothing).
pub fn resolve_target_path(cfg: &Config) -> Option<PathBuf>;

/// Ordered resolution, returns the source for logging/UI.
///   1. PE header from resolved path   (correct even if the target never ran)
///   2. runtime detect() if running    (fallback)
///   3. Unknown                        (last resort; warns loudly)
pub fn resolve(cfg: &Config) -> (Bitness, &'static str);
```

Reading the PE **on disk** is what makes `-w` correct: it needs no running
process. `reg.exe` is already the established registry path (`discover.rs`
shells it via `super::run_tool`), so this adds no new Win32 API surface.

**Wiring:**

- `monitor.rs`: remove the one-shot selection at line 42. Re-resolve per cycle
  inside/ahead of `run_procdump_cycle`, so a target that starts later
  self-corrects. **Log only when the resolved choice changes** — a per-cycle log
  line would flood `procdump.log`.
- `page_monitor.rs:485`: call the same `resolve()` so the GUI preview cannot
  disagree with what the monitor will actually do. Fix its hardcoded
  `os_is_64: true`.
- `page_monitor.rs`: capture the full image path when a process is picked from
  the dropdown, into the new config field.
- `config.rs`: add `target_path: String` with `#[serde(default)]` (PascalCase
  `TargetPath` on the wire) so existing `config.json` files still load.

**Surfacing (decided):** reuse the existing `lbl_bitness` control
(`page_monitor.rs:225`) to show the resolved answer as soon as a target is
picked — `32-bit process -> procdump.exe`, or an explicit
`Bitness unknown - defaulting to procdump64.exe. Verify manually.` Do **not**
block Create Task; pre-arming a not-yet-installed target is legitimate.

**Tests (no fixtures, no frameworks):**

- `pe_machine` against `C:\Windows\SysWOW64\notepad.exe` (x86) and
  `C:\Windows\System32\notepad.exe` (x64) — real PEs, present on every x64
  Windows.
- Pure-string tests for ImagePath parsing: quoted path, unquoted path with
  arguments, `\??\` prefix, embedded environment variables, svchost detection.
- Existing 6 `select_binary` tests must keep passing untouched.

### 2. Measured defects

- **Footer:** after build and before show, query `SPI_GETWORKAREA`, take the
  *outer* `GetWindowRect`, and clamp position (and height, if it still
  overflows) into the work area. Shave the window 780 -> 764 logical for margin.
  ~12 lines in `mod.rs`.
- **Picker:** a filter `Edit` above the combo; typing repopulates the combo from
  a retained source `Vec` on `OnTextInput`. nwg has no autocomplete, so this is
  the mechanism. Empty state: hint row `- Select a process or service -` at
  index 0, preselecting the saved target when config names one.
- **Duplicate service naming:** suppress the appended `(short)` when the display
  name already ends with it, case-insensitively.
- **Warning placement:** move the ProcDump-binary warning out of the field grid
  so it cannot read as the `Dump type` label.
- **Numeric row:** label the four boxes individually.
- **Status rows:** red/green semantics via the existing
  `theme::set_status_color()`.

### 3. Visual pass

Reference-locked, light. Primary: **shadcn/ui** (`c14c0a94`) — monochrome light,
hairline borders instead of shadows, compact density, hierarchy from weight and
spacing, semantic color reserved for status. Borrowed narrowly from **Mezmo**
(`aab1d358`): monospace confined to technical output (the raw-args preview), and
the metric treatment for status rows. **Rejected:** dark canvas, neon accents,
pill buttons, gradients — all of which fight the binding design system.

Existing tokens (`#0F6CBD` accent, `#F3F3F7` sidebar, `#E1DFDD` divider) are
preserved, not replaced. The lock adds discipline, not a new palette.

## Risks

- **Binary size.** ~30KB headroom under the accepted 2.0MB gate. PE reading is
  `std::fs` and `reg.exe` is already shelled, so the delta should be small — but
  it will be **measured and reported**, not assumed.
- **Shared-host services.** A svchost-hosted service cannot be classified from
  its PE. Handled: return `None`, fall through to runtime detection, then to a
  loud Unknown. The Software House services are standalone exes, so this is an
  edge case, not the main path.
- **`reg.exe` output parsing** assumes en-US token layout, consistent with the
  existing documented `sc.exe` caveat.
- **e2e scroll probe is flaky in both directions.** It has produced a false green
  (`CB_SETTOPINDEX`, shipped a bug) and a false red (foreground loss, this
  session). Harden it: before firing the wheel, assert
  `GetForegroundWindow() == app hwnd` and `WindowFromPoint(cursor) == hwndList`,
  and fail with a distinct "harness could not foreground the window" message.

## Verification

1. `cargo test` with `PDM_TEST_MANIFEST=1` — existing 63 plus the new PE and
   ImagePath tests.
2. `scripts/gui-e2e.ps1` exits 0, with the hardened scroll probe.
3. Fresh probe confirming the window bottom now sits inside the work area.
4. Fresh probe confirming a service target resolves to a real bitness rather
   than Unknown.
5. Release build: confirm `requireAdministrator` and **report the measured exe
   size delta**.
