# PROJECT CONTINUATION DOCUMENT
## Session 5 — 26 July 2026

### 1. PROJECT IDENTITY

- **Project Name:** ProcDumpMonitor (a rename is the first agenda item next session)
- **What This Project Is:** A single-exe Windows utility for C·CURE 9000 field support. It (a) watches a process or service and captures ProcDump crash/hang/resource dumps automatically via an installed Scheduled Task, with retention and email/webhook alerts, and (b) collects CCURE diagnostic log bundles for Johnson Controls support.
- **Primary Objective:** One self-contained ~2.07MB Rust exe (monitor + CLI + GUI) that replaces the legacy C#/.NET app and the customer's `CCURE_LogCollector_GUI_v2.0.ps1`, deployable by dropping one file on a server.
- **Strategic Intent:** Kill the .NET runtime dependency and the PowerShell-script distribution problem. A field tech drops one exe on a C-CURE box, runs it elevated, and can both arm crash capture and produce a support bundle in under a minute — no execution-policy fights, no AV flagging, no installer.
- **Hard Constraints:**
  - GUI stays `native-windows-gui` 1.0.13 (NOT egui/iced). **No new crate dependencies** — new Windows API surface comes from the already-present `windows` crate or `std`.
  - **Binary size gate 2,097,152 B (2.0 MiB).** Currently **2,071,040 B — only 26,112 B headroom.** This is now genuinely tight.
  - Release manifest keeps `requireAdministrator`; `build.rs` panics if `PDM_TEST_MANIFEST=1` leaks into a `--release` build.
  - All GUI coordinates LOGICAL px (nwg `high-dpi` scales them). Never multiply by `scale_factor()` outside raw GDI paint paths.
  - Rust toolchain: `%USERPROFILE%\.cargo\bin\cargo.exe` (NOT on PATH), run from `rust/`.
  - No PowerShell script ships as part of the product. (`scripts/*.ps1` are dev/field tools, not shipped artifacts.)
  - `panic = "abort"` is deliberate — the Scheduled Task restarts the monitor. Any panic kills the process.

### 2. WHAT EXISTS RIGHT NOW

- **Built and working (✅ verified this session):**
  - **Bitness resolution, end to end.** `bitness::resolve()` is the single source of truth, used by both the monitor loop and the GUI label. Order: PE header on disk → runtime `detect()` (Process only) → `Unknown`.
  - **AnyCPU-correct PE classification.** `bitness_from_pe` reads the COR20 header when `Machine == 0x014C`. Validated against 233 real binaries (120 AnyCPU → X64, 113 `32BITREQUIRED` → X86, 0 misclassified).
  - **Service targets resolve** via registry `ImagePath` (`reg.exe` through `collect::run_tool`, which sets `CREATE_NO_WINDOW`). svchost-hosted services deliberately return `None` and fall through.
  - **`Config.TargetPath`** persists a Process target's image path so bitness survives a stopped target. `set_target` clears it on a target change; `capture_target_path` writes it from `save()`.
  - **Monitor re-resolves per cycle while `Unknown`, caches once known.** Self-corrects when a target starts later.
  - **Target picker:** hint row at index 0, defaults to `SoftwareHouse.CrossFire.Server.exe` when running (exact match, not substring), saved config target always wins.
  - **e2e hardened:** `scripts/gui-e2e.ps1` asserts `TargetPath` after a real Save Config click — and this assertion **provably fails under the mutant** (comment out the capture call → 133 Rust tests stay green, e2e dies). Scroll probe now distinguishes harness failure from product failure.
  - **142/142 tests pass. Release exe 2,071,040 B. `requireAdministrator` intact.** All merged to `main` and pushed to `personal`.
  - Everything from session 4 remains: mode-based sidebar shell, merged Monitor page, 2 owned dialogs, 3 collector pages, native collection engine, `collect` CLI verb, auto-collect-on-dump.
- **Partially built:**
  - `collect::pdm_bundle::auto_bundle` still has **never executed** (needs a real dump to fire).
  - Install Logs workflow runs but always hits "InstallHistory.xml not found" on this dev box.
- **Broken or blocked:**
  - **`git push origin` fails** — `remote: Repository not found` for `https://github.com/jraburm_jcplc/ProcDump-Monitor.git` (JCI work account). All pushes go to `personal` (`Mattressburn/ProcDumpMonitor`), which works. Needs the user to create the JCI repo or refresh credentials.
  - **The app window does not fit 1920×1080 @125%.** Measured twice by independent routes: outer height 1022 px vs a 1020 px work area, and nwg's `center(true)` centres the *client* height against the *screen* height, so it lands 55 physical px too low and **the entire footer sits under the taskbar, unclickable**. Specified as Phase 2; NOT fixed.
- **NOT started:**
  - **Product rename** (first agenda item next session).
  - Phase 2 UI defects: footer clamp, picker filter box, misplaced ProcDump warning, unlabeled `CPU%/Low%/Dur/Max` boxes, `○` status rows with no colour semantics.
  - Phase 3 visual pass (shadcn-locked light direction — see the spec).
  - Controller Logs / Integrations collector workflows (deliberately out of scope — unimplemented stubs in the source PS1).
  - VSS locked-file capture; dark mode.

### 3. ARCHITECTURE & TECHNICAL MAP

- **Tech stack:** Rust (stable-msvc), `native-windows-gui` 1.0.13 + derive, `windows` 0.58, serde/serde_json, chrono, base64, lettre (SMTP), ureq (webhook), `winresource`; MSVC 2019 Build Tools linker. Legacy C#/.NET app still in the repo root (unreviewed WIP commit `26ab167`).
- **Key files:**
  - `rust/src/bitness.rs` (~1400 lines now) — `Bitness`, `select_binary`, `pe_machine`, **`bitness_from_pe` (COR20-aware)**, `parse_image_path`, `expand_env`, `service_image_path`, `resolve_target_path`, `resolve`, `os_is_64`, `set_target`, `capture_target_path`, `detect`, `classify`, `list_process_names`. **One `File::open` drives both PE walks.**
  - `rust/src/monitor.rs` — `bitness_step` (cached re-resolution), empty-target guard, cycle loop.
  - `rust/src/gui/page_monitor.rs` — `bitness_text` / `update_bitness` (the GUI's only resolution path), `probe_cfg`, `build_target_list`, `target_selection`, `PREFERRED_TARGETS`, `TARGET_HINT`, `write_fields`/`save` split.
  - `rust/src/config.rs` — `Config.target_path` (`TargetPath`, PascalCase, struct-level `#[serde(default)]`).
  - `scripts/gui-e2e.ps1` — UI automation driver (powershell.exe 5.1, idle machine required).
  - `scripts/Check-TargetBitness.ps1` — **new**, field script mirroring `bitness_from_pe`. Also on the TRANSFER USB at `D:\ProcDumpMonitor-tools\` with a README.
  - `docs/superpowers/specs/2026-07-26-bitness-and-polish-design.md` — the 3-phase spec (Phase 1 done; Phases 2–3 pending).
  - `docs/superpowers/plans/2026-07-26-bitness-resolution.md` — the executed plan.
  - `docs/plans/gui-redesign-plan.md` — the binding design system.
- **How bitness works end-to-end:**
  1. GUI: picking a target runs `write_fields` on a throwaway clone → `set_target` (clears `target_path` if name/type changed) → `bitness_text` → `resolve` → `select_binary` → label.
  2. Persisting (Create Task / Save Config) runs `save()` → `write_fields` → then `capture_target_path` resolves and writes `TargetPath`.
  3. `install` verb → `schtasks` registers a task running `ProcDumpMonitor.exe monitor`.
  4. Monitor: each cycle, while the cached answer is `Unknown`, calls `resolve(cfg)` → PE on disk (via `TargetPath` for Process, registry `ImagePath` for Service) → `select_binary` → swaps `cfg.proc_dump_path` and logs **only on change**. Caches once known and when a binary actually exists.
  5. ProcDump launches with the matched binary; retention, notify, `health.json` as before.
- **Naming conventions:** commit prefixes `gui:`/`collect:`/`fix(monitor):`/`fix(bitness):`/`feat(bitness):`/`docs:`/`verify:`/`spec:`/`plan:`/`tools:`. GUI pages `page_<name>.rs`, dialogs `dlg_<name>.rs`. Config JSON PascalCase. Deliberate shortcuts carry a `ponytail:` comment naming the ceiling.
- **External dependencies:** `sc.exe`, `schtasks.exe`, `reg.exe`, `robocopy`, `wevtutil.exe`, `systeminfo.exe`, `tar.exe`, `powershell.exe` (inline `-Command` only), `explorer.exe`, `notepad.exe`, `mmc.exe taskschd.msc`, procdump.exe/procdump64.exe (user-supplied), SMTP / webhook endpoint.

### 4. RECENT WORK — WHAT JUST HAPPENED (HIGH PRIORITY)

- **What was worked on:** Verified session 4's dropdown-scroll fix; found and specced a 3-phase design; executed Phase 1 (bitness correctness) as an 8-task subagent-driven plan with per-task review + a whole-branch review; merged to `main`.

- **Decisions and WHY:**
  - **Read the PE header on disk rather than only probing at runtime.** This is what makes `-w` (wait-for-process, the default) correct — the designed workflow arms the monitor *before* the target exists, so a runtime-only probe can never answer. Do NOT invert to detect-first.
  - **COR20/AnyCPU handling (the session's most important finding).** `Machine == 0x014C` does NOT mean the process runs 32-bit: a managed PE32 with `ILONLY` and neither `32BITREQUIRED` nor `32BITPREFERRED` runs **64-bit** under the 64-bit CLR. Since every C·CURE target is .NET, classifying those as x86 handed them a 32-bit ProcDump, **which cannot capture a 64-bit process at all**. This was a *regression* versus pre-branch behaviour. Verified on a real binary before acting.
  - **One `File::open` for both PE walks.** Two opens meant a transient failure on the second yielded X86, which the monitor then cached for its lifetime — one I/O blip permanently pinned a 64-bit target to the wrong binary. A failed read now yields `Unknown`, which does not settle and therefore retries.
  - **`bitness::resolve` as the single source of truth.** Three OS-bitness implementations previously coexisted (inline `!= "x86"` in the monitor, hardcoded `true` in the GUI, `os_is_64`). That divergence *was* the bug class; do not recreate it.
  - **`set_target` (clear) split from `capture_target_path` (resolve+write).** The clear must live in `write_fields` to preserve ordering; the resolve must live in `save()` because it spawns `reg.exe` and `write_fields` runs per keystroke.
  - **Service targets never cache `TargetPath`.** A persisted copy of a registry-authoritative value goes stale on service upgrade and `.exists()` cannot catch it — a *wrong* answer is worse than a slow-but-correct one.
  - **CrossFire default matches exactly, not by substring** — four processes share the `SoftwareHouse.CrossFire.` prefix.

- **What changed in the system:** `rust/src/bitness.rs`, `config.rs`, `monitor.rs`, `gui/page_monitor.rs`, `gui/mod.rs`, `scripts/gui-e2e.ps1`; new `scripts/Check-TargetBitness.ps1`; new spec + plan docs; `CLAUDE.md` gained a standing "Bitness selection" section. 31 commits, merged to `main`, pushed to `personal`.

- **Discussed but NOT implemented:**
  - Phase 2 (footer clamp, picker filter box, warning placement, numeric labels, status colours) and Phase 3 (shadcn-locked visual pass) — both specced, neither started.
  - Product rename — user's stated next topic.
  - The saved-Service-target **type flip**: `effective_target` derives type from the picked *row* rather than `cfg.target_type`, so an unmatched saved Service target can be re-classified `Process` on the next save (producing `-w` where `-service` belongs). Pre-existing, documented at `page_monitor.rs:736-743`, deliberately ticketed not fixed.

- **Open threads / unresolved questions:**
  1. **The C·CURE field check has never run.** The `Svc:` → PE-header → X86 path has not executed on a real server. Run `D:\ProcDumpMonitor-tools\Check-TargetBitness.ps1` on a C-CURE box; the single most valuable line is `SoftwareHouse.CrossFire.Server.exe`, because it is now the default target — **if it is AnyCPU, it was one of the binaries the pre-fix code got wrong.**
  2. **Product rename** — next session's first task.
  3. `git push origin` still broken (JCI repo 404).
  4. Only ~26KB of size headroom remains; Phase 2/3 could breach it.
  5. `auto_bundle` still unproven.

### 5. WHAT COULD GO WRONG

- **Known bugs/issues:**
  - Window doesn't fit 1920×1080 @125%; footer unreachable (Phase 2).
  - Saved-Service-target type flip (above).
  - `git push origin` 404s.
- **Edge cases to watch:**
  - `gui-e2e.ps1` needs an **idle machine**; it now distinguishes `HARNESS:` from `LAYOUT:` failures, so read the prefix before blaming the product.
  - `reg.exe`/`sc.exe` parsing assumes **en-US** tokens.
  - A shared-host (svchost) **and** 32-bit service cannot be resolved from a PE — it stays `Unknown` → `procdump64.exe`, and the label must keep saying "Unknown". Do not shorten that string.
  - Task Manager's "(32 bit)" is the ground truth to compare against; a disagreement with our RESOLVED column is a real bug.
  - `tar.exe` absent before Win10 1803 / Server 2019 (Compress-Archive fallback untested).
- **Technical debt / shortcuts:**
  - Auto-collect rate limit hardcoded 60 min (`ponytail:`).
  - Three copies of the Toolhelp walk in `bitness.rs` (`detect`, `list_process_names`, `running_process_path`) — a shared `find_pid_by_name` is owed.
  - `cor20_walk_survives_truncation_at_every_length`'s assert is a tautology by design (its value is "did not panic") — do NOT later read it as an invariant check.
  - Theme brushes/fonts deliberately leaked (process-lifetime GUI).
- **Assumptions that could be wrong (flags for the next AI):**
  - **DO NOT assume PE `Machine` gives runtime bitness.** See §4. This is the trap that eight per-task reviews missed.
  - **DO NOT re-merge `write_fields()` into `save()`** — the split is a data-loss guard (an earlier version DPAPI-encrypted the webhook URL into a discarded clone and cleared the live field).
  - **DO NOT put `resolve()` on the 3s status poll timer or any per-keystroke path** — it spawns `reg.exe` for Service targets.
  - **DO NOT verify dropdown scrollability with `CB_SETTOPINDEX`** — it repositions even with no scrollbar. Use real wheel input; and the probe has thrown false reds too (foreground loss), so confirm before believing a failure.
  - Do not assume the C# WIP commit builds.

### 6. HOW TO THINK ABOUT THIS PROJECT

1. **Core philosophy:** smallest possible native artifact, boring Win32 done correctly. Modernity comes from discipline (grid, typography, DPI correctness, live status instead of blind "done" messages), not frameworks. Verification is empirical and adversarial — and this session added a sharper rule: **a test that cannot fail is not a test.** Mutation-check load-bearing lines by reverting the actual fix, not a proxy.
2. **Most common newcomer mistake:** creating an nwg control as a local (its HWND dies on drop); adding physical-pixel math under `high-dpi`; spawning a console child without `CREATE_NO_WINDOW`; calling `save()` where `write_fields` belongs; or "simplifying" the COR20 read back to a Machine-only check.
3. **Looks refactorable but is NOT:** the `write_fields`/`save` split (a data-loss guard, not duplication); the COR20 walk (it looks over-engineered; it is the difference between a usable dump and none); the single-`File::open` structure (two opens is a correctness hole, not a style choice); the three big event dispatchers in `mod.rs`; `panic = "abort"`; the leaked theme brushes.

### 7. DO NOT TOUCH LIST

- Do NOT reintroduce a second OS-bitness or target-bitness implementation — `bitness::resolve` / `bitness::os_is_64` are the only ones.
- Do NOT simplify `bitness_from_pe` back to a `Machine`-only check, and do NOT split the PE reads back onto two `File::open` calls.
- Do NOT invert resolution to detect-first, and do NOT make Service targets cache `TargetPath`.
- Do NOT re-merge `write_fields()` into `save()`.
- Do NOT put `resolve()` on a timer or per-keystroke path.
- Do NOT swap GUI frameworks or add crate dependencies (only ~26KB headroom).
- Do NOT touch the release manifest's `requireAdministrator` or weaken the `build.rs` release guard.
- Do NOT remove the `WS_VSCROLL` injection in `mk_combo()`.
- Do NOT "fix" the leaked brushes/fonts in `theme.rs`.
- Do NOT convert logical coordinates to physical anywhere pages are built.
- Do NOT ship a PowerShell script as part of the product (inline `powershell -Command` is fine; `scripts/*.ps1` are dev/field tools).
- Ask before changing the collector's output layout — it mirrors the PS1 so JCI support tooling still matches.
- Preserve commit-message style and the design system in `docs/plans/gui-redesign-plan.md`.

### 8. CONFIDENCE & FRESHNESS

- §1 Identity/constraints — ✅ HIGH (size gate re-measured this session)
- §2 Bitness subsystem, tests, size, e2e — ✅ HIGH (built, reviewed, mutation-tested, merged this session)
- §2 Collector subsystem — ⚠️ MEDIUM (carried from session 4, not re-verified)
- §2 `auto_bundle`, CCURE-specific collection paths — ❓ LOW (never executed)
- §3 Architecture map — ✅ HIGH (written from code as built this session)
- §4 Recent work — ✅ HIGH
- §5 Risks — ✅ HIGH for measured items (window geometry, AnyCPU, e2e flakes); ⚠️ MEDIUM for the locale and `tar.exe` caveats (reasoned, not observed)
- §6/§7 — ✅ HIGH
- **The C·CURE field check — ❓ LOW / UNVERIFIED. Nothing in this project has ever run against a real C-CURE server.**
