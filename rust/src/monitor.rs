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

    // Bitness-based binary switch (non-fatal on failure)
    let pd_dir = Path::new(&cfg.proc_dump_path).parent()
        .map(|p| p.to_path_buf()).unwrap_or_else(paths::install_dir);
    let os_is_64 = std::env::var("PROCESSOR_ARCHITECTURE").map(|a| a != "x86").unwrap_or(true)
        || std::env::var("PROCESSOR_ARCHITEW6432").is_ok();
    let choice = bitness::select_binary(bitness::detect(&cfg.target_name), &pd_dir, os_is_64);
    logger::log("Monitor", &format!("Bitness: {}", choice.summary));
    if let Some(w) = &choice.warning { logger::log("Monitor", &format!("Bitness WARNING: {w}")); }
    if choice.actual.exists() && choice.actual != Path::new(&cfg.proc_dump_path) {
        logger::log("Monitor", &format!("Switching ProcDump binary -> {}", choice.actual.display()));
        cfg.proc_dump_path = choice.actual.display().to_string();
    }

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
}
