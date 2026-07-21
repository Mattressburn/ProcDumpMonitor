# ProcDumpMonitor Rust Rewrite — Design

**Date:** 2026-07-21
**Status:** Approved (user, this session)

## Goal

Replace the .NET 8 WinForms app with a single small native Windows exe
(~3–6 MB, no runtime install) that does the same job: configure Sysinternals
ProcDump as a SYSTEM scheduled task via a 6-page GUI wizard, run a headless
monitor loop, and send email/webhook notifications when dumps are captured.

Current build is ~70–150 MB self-contained .NET. Target is a true
"drop one exe on the customer machine" deliverable.

## Decisions (user-selected)

| Decision | Choice |
|---|---|
| Language | Rust |
| App shape | Full 6-page GUI wizard (Target · ProcDump · Task · Notify · Review · About) |
| Scope | Core + bitness auto-select + encrypted SMTP password |
| GUI toolkit | native-windows-gui (nwg) — thin Win32 wrapper, native look |
| Build/test host | win11-lab VM (192.168.69.110) for GUI/Windows-API work; Linux for pure-logic unit tests |
| Exe name | `ProcDumpMonitor.exe` (kept — drop-in familiarity) |

### Cut from scope (vs. the C# app)

Support-diagnostics ZIP, config export/import, config **migration** (existing
deployed `config.json` files will NOT load — accepted), themes/dark mode,
one-shot self-test, `--oneshot` verb, `--selftest` verb.

### Rejected alternatives

- **egui/eframe:** cross-compiles from Linux, but non-native look, larger
  binary, and GUI/DPAPI/schtasks still can't be exercised on Linux — so the
  cross-compile advantage buys nothing.
- **Tauri/WebView2:** Server 2016 is a supported target and has no guaranteed
  WebView2 runtime.
- **Task Scheduler COM:** replaced by `schtasks.exe /Create /XML` — proven in
  a live spike on win11-lab (see Proven facts).

## Architecture

One exe, two faces: no args → GUI wizard; verbs → headless CLI. Embedded
manifest sets `requireAdministrator` (UAC prompts at launch; replaces the C#
self-relaunch logic).

### Core modules (headless, built and CLI-testable before any GUI work)

| Module | Responsibility |
|---|---|
| `config` | serde model of `config.json`, load/save next to the exe |
| `secrets` | DPAPI encrypt/decrypt of SMTP password, **LocalMachine scope** (must decrypt under SYSTEM) |
| `procdump` | Assemble procdump command line from config; detect target process 32/64-bit via `IsWow64Process2` → pick `procdump.exe` vs `procdump64.exe` |
| `task` | Generate Task Scheduler XML; register/remove/query via `schtasks.exe` |
| `monitor` | Loop: disk guard → launch procdump → detect new `.dmp` → stability check (size stops growing) → retention (age-days + total-GB) → notify → write `health.json` → sleep restart-delay → repeat |
| `notify` | Email via lettre (rustls) + webhook POST via ureq, with dedup so one dump = one notification |
| `cli` | Verbs: `--monitor`, `install`, `uninstall`, `start`, `stop`, `status` (JSON to stdout); exit codes 0=ok 1=fail 2=bad-args |

### GUI module (built last, thin)

nwg 6-page wizard. Reads/writes config through `config`; performs task
operations by shelling out to its own exe's CLI verbs (one code path).
Service dropdown parses `sc query type= service state= all` output — no COM.
Green/red/blue status banner + session log pane as today.

### Scheduled task properties (unchanged from C# app)

Run as SYSTEM (S-1-5-18), boot trigger, restart-on-failure 1 min × 999,
ignore-new instances, no execution time limit, runs on battery.
Task action: `ProcDumpMonitor.exe --monitor --config <path>`.

## Proven facts (spiked live on win11-lab, 2026-07-21)

1. `schtasks /Create /TN <n> /XML <file> /F` with a principal of only
   `<UserId>S-1-5-18</UserId>` + `<RunLevel>HighestAvailable</RunLevel>`
   registers without `/RU` or password; Task Scheduler reports
   **Run As User: SYSTEM** with the boot trigger intact.
2. **Omit `<LogonType>`** for the SYSTEM principal — `ServiceAccount` fails
   schtasks XML validation ("value incorrectly formatted or out of range").
3. The XML file must be **UTF-16LE with BOM** to match its
   `encoding="UTF-16"` declaration.

## Unproven assumption (first implementation step)

nwg builds and runs with an embedded `requireAdministrator` manifest on the
VM. The VM currently lacks Rust + MSVC Build Tools (~1 GB install) — setting
up the toolchain and running this spike is step 1 of the implementation plan.
If nwg fails the spike, fall back to egui (design unchanged; only the `gui`
module and binary size change).

## Dependencies

`native-windows-gui` + `native-windows-derive`, `serde` + `serde_json`,
`lettre` (rustls feature, no native TLS), `ureq`, `windows` (feature-gated:
DPAPI + `IsWow64Process2` only). Everything else stdlib.

## Error handling

Monitor loop never exits on per-cycle errors: catch, record in `health.json`
`last_error`, sleep, retry. Missing procdump exe / unwritable dump dir are
config-time errors surfaced in the GUI banner and as CLI exit 1. schtasks
nonzero exit → stderr passthrough + exit 1.

## Testing

- **Linux (`cargo test`, fast loop):** procdump flag assembly, task-XML
  generation (byte-exact vs known-good spike XML), retention selection,
  config round-trip, notification dedup.
- **VM (manual + CLI):** DPAPI round-trip under admin and under SYSTEM
  (`psexec -s`), bitness detection, `install`/`status`/`uninstall` against
  real Task Scheduler, GUI walkthrough, end-to-end monitor cycle against a
  toy crashing process.

## Config schema (fresh, no migration)

`config.json` next to exe, schema equivalent to today's V3 minus cut
features: target process/scenario, procdump path + flags, dump dir,
restart-delay, min-free-disk-MB, task name, email (server/port/ssl/from/
to/cc/user/DPAPI-blob), webhook URL, retention (log-size-MB, log-files,
dump-age-days, dump-total-GB, stability-timeout-s).
