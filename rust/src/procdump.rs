#![allow(dead_code)] // consumed from Task 8 onward

use crate::config::{Config, TargetType};

/// Port of C# Config.BuildProcDumpArgs — flag order is contract, do not "tidy".
pub fn build_args(cfg: &Config) -> String {
    let mut args: Vec<String> = vec!["-accepteula".into()];

    match cfg.dump_type.as_str() {
        "Full" => args.push("-ma".into()),
        "MiniPlus" => args.push("-mp".into()),
        "Mini" => args.push("-mm".into()),
        "ThreadDump" => args.push("-mt".into()),
        _ => {}
    }

    if cfg.dump_on_exception { args.push("-e".into()); }
    if cfg.dump_on_terminate { args.push("-t".into()); }
    if cfg.hang_window_seconds > 0 { args.push("-h".into()); }

    if cfg.use_clone { args.push("-r".into()); }
    if cfg.avoid_outage { args.push("-a".into()); }
    if cfg.overwrite_existing { args.push("-o".into()); }

    if cfg.cpu_threshold > 0 { args.push(format!("-c {}", cfg.cpu_threshold)); }
    if cfg.cpu_low_threshold > 0 { args.push(format!("-cl {}", cfg.cpu_low_threshold)); }
    if cfg.cpu_duration_seconds > 0 { args.push(format!("-s {}", cfg.cpu_duration_seconds)); }
    if cfg.cpu_per_unit { args.push("-u".into()); }

    if cfg.memory_commit_mb > 0 { args.push(format!("-m {}", cfg.memory_commit_mb)); }

    if !cfg.performance_counter.trim().is_empty() {
        args.push(format!("-p \"{}\"", cfg.performance_counter));
    }
    if !cfg.perf_counter_threshold.trim().is_empty() {
        args.push(format!("-pl \"{}\"", cfg.perf_counter_threshold));
    }
    if !cfg.exception_filter_include.trim().is_empty() {
        args.push(format!("-f \"{}\"", cfg.exception_filter_include));
    }
    if !cfg.exception_filter_exclude.trim().is_empty() {
        args.push(format!("-fx \"{}\"", cfg.exception_filter_exclude));
    }
    if cfg.wer_integration { args.push("-wer".into()); }
    if cfg.avoid_terminate_timeout > 0 { args.push(format!("-at {}", cfg.avoid_terminate_timeout)); }

    args.push(format!("-n {}", cfg.max_dumps));
    if cfg.wait_for_process { args.push("-w".into()); }

    match cfg.target_type {
        TargetType::Service => args.push(format!("-service {}", cfg.target_name)),
        TargetType::Process => {
            let t = cfg.target_name.clone();
            if !t.trim().is_empty() && !t.to_ascii_lowercase().ends_with(".exe") {
                args.push(format!("{t}.exe"));
            } else {
                args.push(t);
            }
        }
    }

    args.push(format!("\"{}\"", cfg.dump_directory));
    args.join(" ")
}

pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub effective_flags: &'static str,
    apply_fn: fn(&mut Config),
}

impl Preset {
    pub fn all() -> &'static [Preset] { &PRESETS }

    pub fn find(name: &str) -> Option<&'static Preset> {
        PRESETS.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Reset all trigger/operational fields to safe zeroes, then apply.
    /// Preserves: WaitForProcess, ProcDumpPath, DumpDirectory, TargetName, RestartDelay.
    pub fn apply(&self, cfg: &mut Config) {
        cfg.dump_type = "Full".into();
        cfg.dump_on_exception = false;
        cfg.dump_on_terminate = false;
        cfg.use_clone = false;
        cfg.avoid_outage = false;
        cfg.overwrite_existing = false;
        cfg.cpu_per_unit = false;
        cfg.cpu_threshold = 0;
        cfg.cpu_low_threshold = 0;
        cfg.cpu_duration_seconds = 0;
        cfg.memory_commit_mb = 0;
        cfg.hang_window_seconds = 0;
        cfg.max_dumps = 1;
        cfg.wer_integration = false;
        cfg.avoid_terminate_timeout = 0;
        cfg.performance_counter.clear();
        cfg.perf_counter_threshold.clear();
        cfg.exception_filter_include.clear();
        cfg.exception_filter_exclude.clear();
        (self.apply_fn)(cfg);
        cfg.scenario = self.name.into();
    }
}

static PRESETS: [Preset; 5] = [
    Preset {
        name: "Crash capture",
        description: "Captures a full dump when the process throws an unhandled exception or terminates unexpectedly. Uses safe defaults appropriate for production systems. Ideal for post-mortem crash investigation.",
        effective_flags: "-ma -e -t",
        apply_fn: |c| { c.dump_on_exception = true; c.dump_on_terminate = true; },
    },
    Preset {
        name: "Hang capture",
        description: "Captures a full dump when the process window stops responding (hung). Useful for diagnosing UI freezes and deadlocks.",
        effective_flags: "-ma -h",
        apply_fn: |c| { c.hang_window_seconds = 1; },
    },
    Preset {
        name: "High CPU spike capture",
        description: "Captures up to 3 full dumps when CPU usage exceeds 90 % for at least 10 consecutive seconds. Helps identify runaway threads or hot code paths.",
        effective_flags: "-ma -c 90 -s 10 -n 3",
        apply_fn: |c| { c.cpu_threshold = 90; c.cpu_duration_seconds = 10; c.max_dumps = 3; },
    },
    Preset {
        name: "Memory threshold capture",
        description: "Captures up to 3 full dumps when process memory commit exceeds 2048 MB. Useful for investigating memory leaks or unexpected memory growth.",
        effective_flags: "-ma -m 2048 -n 3",
        apply_fn: |c| { c.memory_commit_mb = 2048; c.max_dumps = 3; },
    },
    Preset {
        name: "Low impact full dump",
        description: "A full memory dump equivalent to Task Manager, captured via process cloning (-r) to minimize disruption. The -a flag prevents dump floods; the process is suspended for only milliseconds instead of the full dump duration.",
        effective_flags: "-a -r -ma",
        apply_fn: |c| { c.avoid_outage = true; c.use_clone = true; c.max_dumps = 1; },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, TargetType};

    fn base() -> Config {
        let mut c = Config::default();
        c.target_name = "MyApp".into();
        c.dump_directory = r"C:\Dumps\MyApp".into();
        c
    }

    #[test]
    fn default_crash_args_match_csharp_order() {
        let c = base();
        // defaults: Full, -e, -t, -r, -n 1, -w, target gets .exe appended, quoted dir
        assert_eq!(
            build_args(&c),
            r#"-accepteula -ma -e -t -r -n 1 -w MyApp.exe "C:\Dumps\MyApp""#
        );
    }

    #[test]
    fn service_target_uses_service_flag_and_no_exe_suffix() {
        let mut c = base();
        c.target_type = TargetType::Service;
        assert!(build_args(&c).contains("-w -service MyApp \""));
    }

    #[test]
    fn exe_suffix_not_doubled() {
        let mut c = base();
        c.target_name = "MyApp.EXE".into();
        assert!(build_args(&c).contains("-w MyApp.EXE \""));
    }

    #[test]
    fn all_flags_render_in_order() {
        let mut c = base();
        c.dump_type = "MiniPlus".into();
        c.hang_window_seconds = 1;
        c.avoid_outage = true;
        c.overwrite_existing = true;
        c.cpu_threshold = 90;
        c.cpu_low_threshold = 5;
        c.cpu_duration_seconds = 10;
        c.cpu_per_unit = true;
        c.memory_commit_mb = 2048;
        c.performance_counter = r"\Processor(_Total)\% Processor Time".into();
        c.exception_filter_include = "OutOfMemory".into();
        c.wer_integration = true;
        c.avoid_terminate_timeout = 7;
        c.max_dumps = 3;
        let a = build_args(&c);
        assert_eq!(
            a,
            r#"-accepteula -mp -e -t -h -r -a -o -c 90 -cl 5 -s 10 -u -m 2048 -p "\Processor(_Total)\% Processor Time" -f "OutOfMemory" -wer -at 7 -n 3 -w MyApp.exe "C:\Dumps\MyApp""#
        );
    }

    #[test]
    fn presets_match_readme_flags() {
        let mut c = base();
        Preset::find("High CPU spike capture").unwrap().apply(&mut c);
        assert_eq!(c.cpu_threshold, 90);
        assert_eq!(c.cpu_duration_seconds, 10);
        assert_eq!(c.max_dumps, 3);
        assert!(!c.dump_on_exception, "preset reset must zero triggers");
        // reset preserves paths + wait_for_process
        assert_eq!(c.dump_directory, r"C:\Dumps\MyApp");
        assert!(c.wait_for_process);
        assert_eq!(Preset::all().len(), 5);
        assert_eq!(Preset::all()[0].name, "Crash capture");
        assert_eq!(Preset::find("Low impact full dump").unwrap().effective_flags, "-a -r -ma");
    }
}
