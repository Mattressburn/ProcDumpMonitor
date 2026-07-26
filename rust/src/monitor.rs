#![cfg(windows)]
use crate::config::Config;
use crate::notify::NotifyQueue;
use crate::{bitness, diskguard, health, logger, paths, procdump, retention, stability};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

static STOPPING: AtomicBool = AtomicBool::new(false);

fn install_ctrl_c_handler() {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::System::Console::SetConsoleCtrlHandler;
    unsafe extern "system" fn handler(_: u32) -> BOOL {
        STOPPING.store(true, Ordering::SeqCst);
        BOOL(1)
    }
    unsafe { let _ = SetConsoleCtrlHandler(Some(handler), true); }
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

pub fn run(mut cfg: Config) {
    STOPPING.store(false, Ordering::SeqCst);
    install_ctrl_c_handler();

    let health_path = paths::health_path();
    let mut h = health::load(&health_path); // resume TotalDumpCount across restarts
    h.monitor_pid = std::process::id();
    h.version = env!("CARGO_PKG_VERSION").into();

    logger::log("Monitor", "ProcDump Monitor started.");
    logger::log("Monitor", &format!("Target: {} ({:?})", cfg.target_name, cfg.target_type));

    // Bitness-based binary switch (non-fatal on failure). Resolved in the loop,
    // not here: with -w (the default) the monitor is armed BEFORE the target
    // exists, so a one-shot resolve at startup answers Unknown and every target
    // gets procdump64.exe. See `bitness_step`.
    let pd_dir = Path::new(&cfg.proc_dump_path).parent()
        .map(|p| p.to_path_buf()).unwrap_or_else(paths::install_dir);
    let os64 = bitness::os_is_64();
    let mut resolved: Option<bitness::Bitness> = None;

    // build_args does not contain the binary path, so this stays true across a
    // switch — logged once here so the log is diagnosable even if bitness never
    // resolves.
    logger::log("Monitor", &format!("ProcDump args: {}", procdump::build_args(&cfg)));

    if std::fs::create_dir_all(&cfg.dump_directory).is_err() {
        logger::log("Monitor", "Cannot create dump directory - exiting.");
        return;
    }

    let queue = NotifyQueue::new();
    let mut last_low_disk_notify: Option<Instant> = None;

    while !STOPPING.load(Ordering::SeqCst) {
        let cycle_start = SystemTime::now();
        h.last_cycle_utc = now_iso();
        h.last_error.clear();
        h.disk_space_low = false;
        logger::log("Monitor", "-- Cycle start --");

        // Before run_procdump_cycle, which reads cfg.proc_dump_path.
        for line in bitness_step(&mut resolved, &mut cfg, &pd_dir, os64, bitness::resolve) {
            logger::log("Monitor", &line);
        }

        // Disk guard
        let mut skip_cycle = false;
        if cfg.min_free_disk_mb > 0 {
            let (ok, free_mb) = diskguard::check_free_space(Path::new(&cfg.dump_directory), cfg.min_free_disk_mb);
            h.free_disk_mb = free_mb;
            h.disk_space_low = !ok;
            if !ok {
                let warn = format!("Skipping cycle -- only {free_mb} MB free (threshold: {} MB)", cfg.min_free_disk_mb);
                logger::log("Monitor", &warn);
                // rate-limited to once per hour
                if last_low_disk_notify.map_or(true, |t| t.elapsed() >= Duration::from_secs(3600)) {
                    last_low_disk_notify = Some(Instant::now());
                    queue.enqueue_warning(cfg.clone(),
                        format!("[ProcDump] Low disk warning on {}", crate::notify::machine_name()),
                        warn);
                }
                skip_cycle = true;
            }
        }

        if !skip_cycle {
            retention::apply(Path::new(&cfg.dump_directory), cfg.dump_retention_days, cfg.dump_retention_max_gb);
            if let Err(e) = run_procdump_cycle(&cfg, cycle_start, &queue, &mut h, &health_path) {
                h.last_error = e.clone();
                logger::log("Monitor", &format!("Cycle error: {e}"));
            }
        }

        h.next_retry_utc = now_iso();
        health::write(&health_path, &h);

        // interruptible sleep
        let delay = cfg.restart_delay_seconds.max(0) as u64 * 10;
        for _ in 0..delay {
            if STOPPING.load(Ordering::SeqCst) { break; }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    logger::log("Monitor", "ProcDump Monitor stopped.");
}

/// One cycle's bitness bookkeeping: re-resolve while the answer is still
/// Unknown, switch `cfg.proc_dump_path` when the chosen binary changes, and
/// hand back the lines to log.
///
/// Lines are RETURNED rather than logged, and `resolve` is injected, so the
/// decision is testable without a running monitor.
///
/// `cached` is both the cache and the "have we ever resolved" flag:
/// `None` = never, `Some(Unknown)` = tried and failed (retry), `Some(b)` =
/// settled (stop). It is an Option and not a bare `Bitness` because a run where
/// the target never starts AND the configured path already equals the chosen
/// default would otherwise log nothing about bitness at all — a regression
/// against the old unconditional startup line.
///
/// ponytail: re-resolution is per cycle, unrate-limited. In the designed
/// workflow that is rare — with -w the ProcDump child blocks until the target
/// appears, so a "cycle" is as long as the target's life. Ceiling: with
/// `restart_delay_seconds == 0` AND a ProcDump that exits instantly (missing
/// binary) the pre-existing tight spin now also spawns one reg.exe per
/// iteration for Service targets. Upgrade path: a min-interval on the retry,
/// if that pathology ever shows up in the field.
fn bitness_step(
    cached: &mut Option<bitness::Bitness>,
    cfg: &mut Config,
    pd_dir: &Path,
    os64: bool,
    resolve: fn(&Config) -> (bitness::Bitness, &'static str),
) -> Vec<String> {
    // Once known it cannot change without the config changing, and resolving
    // spawns reg.exe for Service targets — so only retry while Unknown.
    if matches!(cached, Some(b) if *b != bitness::Bitness::Unknown) {
        return Vec::new();
    }
    let first = cached.is_none();
    let (b, source) = resolve(cfg);
    *cached = Some(b);

    let choice = bitness::select_binary(b, pd_dir, os64);
    // Compare the chosen BINARY PATH, not the source string: the source can
    // change while the selected binary does not.
    let switching = choice.actual.exists() && choice.actual != Path::new(&cfg.proc_dump_path);

    // This runs every cycle while Unknown; a line per cycle would fill the log.
    // The three arms that may speak: the first attempt, an actual switch, and
    // the Unknown -> known transition (which fires at most once — after it,
    // `cached` is settled and every later cycle returns above).
    if !first && !switching && b == bitness::Bitness::Unknown {
        return Vec::new();
    }

    let mut lines = Vec::new();
    if let Some(w) = &choice.warning {
        lines.push(format!("Bitness WARNING: {w}"));
    }
    if switching {
        lines.push(format!("Bitness: {} (via {source}) -> {}", choice.summary, choice.actual.display()));
        cfg.proc_dump_path = choice.actual.display().to_string();
    } else {
        lines.push(format!("Bitness: {} (via {source})", choice.summary));
    }
    lines
}

fn run_procdump_cycle(
    cfg: &Config,
    cycle_start: SystemTime,
    queue: &NotifyQueue,
    h: &mut health::HealthStatus,
    health_path: &Path,
) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut child = std::process::Command::new(&cfg.proc_dump_path)
        .raw_arg(procdump::build_args(cfg))
        .current_dir(&cfg.dump_directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("cannot launch procdump: {e}"))?;

    h.proc_dump_pid = child.id();

    // stream output to the log from reader threads
    let spawn_reader = |stream: Option<Box<dyn std::io::Read + Send>>, tag: &'static str| {
        if let Some(s) = stream {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                for line in BufReader::new(s).lines().map_while(Result::ok) {
                    logger::log(tag, &line);
                }
            });
        }
    };
    spawn_reader(child.stdout.take().map(|s| Box::new(s) as _), "ProcDump");
    spawn_reader(child.stderr.take().map(|s| Box::new(s) as _), "ProcDump-ERR");

    // wait with 30s health heartbeat so "waiting for target" != "stalled"
    let mut beats = 0u32;
    let exit_code = loop {
        if STOPPING.load(Ordering::SeqCst) {
            let _ = child.kill();
            break -1;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) => {
                std::thread::sleep(Duration::from_secs(1));
                beats += 1;
                if beats % 30 == 0 {
                    h.last_cycle_utc = now_iso();
                    health::write(health_path, h);
                }
            }
            Err(e) => return Err(format!("wait failed: {e}")),
        }
    };

    h.proc_dump_pid = 0;
    h.last_proc_dump_exit_code = exit_code;
    logger::log("Monitor", &format!("ProcDump exited with code {exit_code}."));

    detect_and_notify(cfg, cycle_start, queue, h);
    Ok(())
}

fn detect_and_notify(cfg: &Config, cycle_start: SystemTime, queue: &NotifyQueue, h: &mut health::HealthStatus) {
    let Ok(rd) = std::fs::read_dir(&cfg.dump_directory) else { return };
    let newest = rd.flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("dmp")))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            let t = m.modified().ok()?;
            (t >= cycle_start).then_some((e.path(), t))
        })
        .max_by_key(|(_, t)| *t);

    let Some((path, _)) = newest else {
        logger::log("Monitor", "No new dump file detected in this cycle.");
        return;
    };

    logger::log("Monitor", &format!("New dump detected: {}. Checking stability...", path.display()));
    if !stability::wait_for_stable_file(&path, cfg.dump_stability_timeout_seconds, cfg.dump_stability_poll_seconds) {
        h.last_error = "Dump file still locked after timeout - notification suppressed.".into();
        return;
    }

    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    h.last_dump_file_name = file_name.clone();
    h.total_dump_count += 1;

    if h.last_notified_dump_file == file_name {
        logger::log("Monitor", "Dump already notified - skipping duplicate notification.");
        return;
    }

    queue.enqueue_dump(cfg.clone(), path.display().to_string());
    h.last_notified_dump_file = file_name;
    h.last_notified_utc = now_iso();

    // Auto-collect a support bundle (rate-limited, skipped when disk is low).
    if cfg.auto_collect_on_dump {
        if h.disk_space_low {
            logger::log("Monitor", "Auto-collect skipped: disk space low.");
        } else {
            let opts = crate::collect::pdm_bundle::Options {
                log_dir: paths::log_dir(),
                health_path: paths::health_path(),
                config_path: paths::config_path(),
                task_name: crate::task::sanitize_task_name(&cfg.task_name),
                dump_dir: std::path::PathBuf::from(&cfg.dump_directory),
                max_dump_bytes: crate::collect::pdm_bundle::DEFAULT_MAX_DUMP_BYTES,
            };
            match crate::collect::pdm_bundle::auto_bundle(&opts) {
                Ok(dir) => logger::log("Monitor", &format!("Auto-collected support bundle: {}", dir.display())),
                Err(e) => logger::log("Monitor", &format!("Auto-collect skipped: {e}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitness::Bitness;

    /// A ProcDump directory with real (empty) binaries, unique per call so the
    /// fixtures cannot race under cargo's parallel test threads. Prefix differs
    /// from bitness.rs's `pdm_bit_` — both fixtures live in the same test binary.
    fn pd_dir(files: &[&str]) -> std::path::PathBuf {
        use std::sync::atomic::AtomicU32;
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("pdm_mon_{n}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for f in files {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        d
    }

    fn cfg_at(dir: &Path, binary: &str) -> Config {
        let mut c = Config::default();
        c.proc_dump_path = dir.join(binary).display().to_string();
        c
    }

    fn unresolved(_: &Config) -> (Bitness, &'static str) { (Bitness::Unknown, "unresolved") }
    fn x86(_: &Config) -> (Bitness, &'static str) { (Bitness::X86, "PE header") }
    fn x64(_: &Config) -> (Bitness, &'static str) { (Bitness::X64, "PE header") }

    #[test]
    fn a_target_that_starts_later_self_corrects_to_the_32bit_binary() {
        // THE BUG. Cycle 1 runs before the target exists: Unknown -> the 64-bit
        // default. Cycle 2 resolves x86 and the binary must switch. A one-shot
        // resolve at startup leaves procdump64.exe wired to a 32-bit target.
        let d = pd_dir(&["procdump.exe", "procdump64.exe"]);
        let mut cfg = cfg_at(&d, "procdump64.exe");
        let mut cached = None;

        let l1 = bitness_step(&mut cached, &mut cfg, &d, true, unresolved);
        assert_eq!(cached, Some(Bitness::Unknown));
        assert_eq!(cfg.proc_dump_path, d.join("procdump64.exe").display().to_string());
        // Exact, because `summary` itself contains an arrow — "no switch line"
        // cannot be checked by looking for "->".
        assert_eq!(l1, ["Bitness: Unknown bitness -> procdump64.exe (default) (via unresolved)"],
            "first attempt must be visible and must not claim a switch");

        let l2 = bitness_step(&mut cached, &mut cfg, &d, true, x86);
        assert_eq!(cached, Some(Bitness::X86));
        assert_eq!(cfg.proc_dump_path, d.join("procdump.exe").display().to_string(),
            "32-bit target did not switch off procdump64.exe");
        assert_eq!(l2, [format!("Bitness: 32-bit process -> procdump.exe (via PE header) -> {}",
            d.join("procdump.exe").display())]);
    }

    #[test]
    fn does_not_re_resolve_once_the_answer_is_known() {
        // The cache. resolve() spawns reg.exe for Service targets, and the cycle
        // delay can be zero — a call per cycle is a subprocess per cycle.
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        fn counted(_: &Config) -> (Bitness, &'static str) {
            CALLS.fetch_add(1, Ordering::Relaxed);
            (Bitness::X64, "PE header")
        }
        let d = pd_dir(&["procdump.exe", "procdump64.exe"]);
        let mut cfg = cfg_at(&d, "procdump.exe");
        let mut cached = None;

        for _ in 0..5 {
            bitness_step(&mut cached, &mut cfg, &d, true, counted);
        }
        assert_eq!(CALLS.load(Ordering::Relaxed), 1, "re-resolved after the answer was known");
    }

    #[test]
    fn keeps_retrying_while_unresolved() {
        // The other half of the cache: Unknown is NOT a settled answer. Cache it
        // as final and a target that starts later never corrects.
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        fn counted(_: &Config) -> (Bitness, &'static str) {
            CALLS.fetch_add(1, Ordering::Relaxed);
            (Bitness::Unknown, "unresolved")
        }
        let d = pd_dir(&["procdump.exe", "procdump64.exe"]);
        let mut cfg = cfg_at(&d, "procdump64.exe");
        let mut cached = None;

        for _ in 0..5 {
            bitness_step(&mut cached, &mut cfg, &d, true, counted);
        }
        assert_eq!(CALLS.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn logs_nothing_further_while_the_answer_stays_unknown() {
        // Log flood guard: this path runs every cycle for the entire life of a
        // monitor whose target never starts.
        let d = pd_dir(&["procdump.exe", "procdump64.exe"]);
        let mut cfg = cfg_at(&d, "procdump64.exe");
        let mut cached = None;

        assert_eq!(bitness_step(&mut cached, &mut cfg, &d, true, unresolved).len(), 1);
        for _ in 0..10 {
            assert_eq!(bitness_step(&mut cached, &mut cfg, &d, true, unresolved), Vec::<String>::new(),
                "repeated Unknown cycles must stay silent");
        }
    }

    #[test]
    fn an_unchanged_binary_is_not_logged_as_a_switch() {
        // Decide on the CHOSEN BINARY PATH, not the source string: here the
        // source changes (unresolved -> PE header) while the binary does not.
        // Drop the path comparison and both cycles claim a switch that never
        // happened. Lines are compared EXACTLY: `summary` contains its own
        // arrow, so a substring check for "->" proves nothing.
        let d = pd_dir(&["procdump.exe", "procdump64.exe"]);
        let mut cfg = cfg_at(&d, "procdump64.exe");
        let mut cached = None;

        let l1 = bitness_step(&mut cached, &mut cfg, &d, true, unresolved);
        assert_eq!(l1, ["Bitness: Unknown bitness -> procdump64.exe (default) (via unresolved)"]);

        let l2 = bitness_step(&mut cached, &mut cfg, &d, true, x64);
        assert_eq!(cached, Some(Bitness::X64));
        assert_eq!(l2, ["Bitness: 64-bit process -> procdump64.exe (via PE header)"],
            "the Unknown -> known transition must be visible, and must not read as a switch");
        assert_eq!(cfg.proc_dump_path, d.join("procdump64.exe").display().to_string());
    }

    #[test]
    fn a_missing_binary_warns_once_not_every_cycle() {
        // No ProcDump at all: select_binary hands back an empty path plus a
        // warning, forever. The warning must reach the log (first attempt) and
        // then stop.
        let d = pd_dir(&[]);
        let mut cfg = cfg_at(&d, "procdump64.exe");
        let mut cached = None;

        let l1 = bitness_step(&mut cached, &mut cfg, &d, true, unresolved);
        assert_eq!(l1.len(), 2, "{l1:?}");
        assert!(l1[0].starts_with("Bitness WARNING: Neither"), "{l1:?}");
        assert_eq!(bitness_step(&mut cached, &mut cfg, &d, true, unresolved), Vec::<String>::new());
        // ...and an empty chosen path must never be written over a real one.
        assert_eq!(cfg.proc_dump_path, d.join("procdump64.exe").display().to_string());
    }

    #[test]
    fn a_fallback_for_a_32bit_target_warns_loudly() {
        // 32-bit target, only procdump64.exe present: the dump will be useless,
        // so the warning must reach the log alongside the decision.
        let d = pd_dir(&["procdump64.exe"]);
        let mut cfg = cfg_at(&d, "procdump64.exe");
        let mut cached = None;

        let l = bitness_step(&mut cached, &mut cfg, &d, true, x86);
        assert_eq!(l, [
            "Bitness WARNING: procdump.exe not found - falling back to procdump64.exe.",
            "Bitness: 32-bit process -> procdump64.exe (fallback) (via PE header)",
        ]);
    }

    #[test]
    fn a_32bit_os_pins_procdump_exe_regardless_of_target() {
        // os64 is threaded through from bitness::os_is_64(); prove the parameter
        // is actually consulted and not shadowed by a default.
        let d = pd_dir(&["procdump.exe", "procdump64.exe"]);
        let mut cfg = cfg_at(&d, "procdump64.exe");
        let mut cached = None;

        let l = bitness_step(&mut cached, &mut cfg, &d, false, x64);
        assert_eq!(cfg.proc_dump_path, d.join("procdump.exe").display().to_string(), "{l:?}");
    }
}
