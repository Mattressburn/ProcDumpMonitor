// ponytail: cli::run and its Verb::*{config} consumers only exist behind
// #[cfg(windows)] (this product's real entry points are Windows-only); the
// fields/consts here are genuinely used there. Suppress the Linux-only
// dead-code noise without hiding real dead code on Windows.
#![cfg_attr(not(windows), allow(dead_code))]

use std::path::PathBuf;

#[derive(Debug)]
pub enum Verb {
    Monitor { config: PathBuf },
    Install { config: PathBuf },
    Uninstall { config: PathBuf },
    Start { config: PathBuf },
    Stop { config: PathBuf },
    Status { config: PathBuf },
    Collect { config: PathBuf, out: Option<PathBuf>, workflows: Vec<String> },
    Version,
    Help,
}

pub const USAGE: &str = "\
ProcDumpMonitor.exe                     launch the GUI
ProcDumpMonitor.exe <verb> [--config <path>]
  verbs: monitor | install | uninstall | start | stop | status | collect | version | help
  collect options: [--out <dir>] [--workflows data,install,health,pdm]
                   (default: all workflows, output on the Desktop)
  exit codes: 0 = success, 1 = failure, 2 = bad arguments";

pub const ALL_WORKFLOWS: [&str; 4] = ["data", "install", "health", "pdm"];

pub fn parse(args: &[String]) -> Result<Verb, String> {
    let first = args.first().ok_or("no verb given")?;
    let verb = first.trim_start_matches('-').to_ascii_lowercase();
    let mut config = crate::paths::config_path();
    let mut out: Option<PathBuf> = None;
    let mut workflows: Vec<String> = ALL_WORKFLOWS.iter().map(|s| s.to_string()).collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].trim_start_matches('-') {
            "config" => {
                i += 1;
                let v = args.get(i).ok_or("--config requires a path")?;
                config = PathBuf::from(v);
            }
            "out" => {
                i += 1;
                let v = args.get(i).ok_or("--out requires a path")?;
                out = Some(PathBuf::from(v));
            }
            "workflows" => {
                i += 1;
                let v = args.get(i).ok_or("--workflows requires a list")?;
                workflows = v
                    .split(',')
                    .map(|w| w.trim().to_ascii_lowercase())
                    .filter(|w| !w.is_empty())
                    .collect();
                if workflows.is_empty() {
                    return Err("--workflows requires a non-empty list".into());
                }
                if let Some(bad) = workflows.iter().find(|w| !ALL_WORKFLOWS.contains(&w.as_str())) {
                    return Err(format!("unknown workflow: {bad}"));
                }
            }
            other => return Err(format!("unknown option: {other}")),
        }
        i += 1;
    }
    Ok(match verb.as_str() {
        "monitor" => Verb::Monitor { config },
        "install" => Verb::Install { config },
        "uninstall" => Verb::Uninstall { config },
        "start" => Verb::Start { config },
        "stop" => Verb::Stop { config },
        "status" => Verb::Status { config },
        "collect" => Verb::Collect { config, out, workflows },
        "version" => Verb::Version,
        "help" => Verb::Help,
        other => return Err(format!("unknown verb: {other}")),
    })
}

#[cfg(windows)]
pub fn run(verb: Verb) -> i32 {
    use crate::{config::Config, logger, monitor, paths, task};

    fn load_and_init(config_path: &std::path::Path) -> Config {
        let cfg = Config::load(config_path);
        logger::init(paths::log_path(), cfg.max_log_size_mb, cfg.max_log_files);
        cfg
    }

    fn report(res: Result<(), String>, ok_msg: &str) -> i32 {
        match res {
            Ok(()) => { println!("{ok_msg}"); 0 }
            Err(e) => { eprintln!("ERROR: {e}"); 1 }
        }
    }

    match verb {
        Verb::Version => { println!("{}", env!("CARGO_PKG_VERSION")); 0 }
        Verb::Help => { println!("{USAGE}"); 0 }
        Verb::Monitor { config } => {
            let cfg = load_and_init(&config);
            monitor::run(cfg);
            0
        }
        Verb::Install { config } => {
            let cfg = load_and_init(&config);
            match task::install(&cfg) {
                Ok(existed) => {
                    println!("Task '{}' {}.", task::sanitize_task_name(&cfg.task_name),
                             if existed { "updated" } else { "created" });
                    0
                }
                Err(e) => { eprintln!("ERROR: {e}"); 1 }
            }
        }
        Verb::Uninstall { config } => {
            let cfg = load_and_init(&config);
            report(task::uninstall(&task::sanitize_task_name(&cfg.task_name)), "Task removed.")
        }
        Verb::Start { config } => {
            let cfg = load_and_init(&config);
            report(task::start(&task::sanitize_task_name(&cfg.task_name)), "Task started.")
        }
        Verb::Stop { config } => {
            let cfg = load_and_init(&config);
            report(task::stop(&task::sanitize_task_name(&cfg.task_name)), "Task stopped.")
        }
        Verb::Status { config } => {
            let cfg = load_and_init(&config);
            let st = task::query_status(&task::sanitize_task_name(&cfg.task_name));
            println!("{}", serde_json::to_string_pretty(&st).unwrap_or_default());
            0
        }
        Verb::Collect { config, out, workflows } => {
            let cfg = load_and_init(&config);
            let base = out.unwrap_or_else(default_collect_base);
            match run_collect(&cfg, &config, &base, &workflows) {
                Ok(dir) => { println!("Collection complete: {}", dir.display()); 0 }
                Err(e) => { eprintln!("ERROR: {e}"); 1 }
            }
        }
    }
}

/// Blank/default output base = the user's Desktop (PS1 behavior).
#[cfg(windows)]
pub fn default_collect_base() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Desktop"))
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// One run folder, selected workflows in order. Shared by the CLI; the GUI
/// builds its own options from its checkboxes and calls the same modules.
#[cfg(windows)]
pub fn run_collect(
    cfg: &crate::config::Config,
    config_path: &std::path::Path,
    base: &std::path::Path,
    workflows: &[String],
) -> Result<PathBuf, String> {
    use crate::collect::{self, datacoll, discover, installlogs, pdm_bundle, syshealth};
    use crate::{paths, task};

    let mut ctx = collect::RunContext::start(base, Box::new(|s: &str| println!("{s}")))
        .map_err(|e| format!("cannot create run folder under {}: {e}", base.display()))?;

    let pdm_opts = pdm_bundle::Options {
        log_dir: paths::log_dir(),
        health_path: paths::health_path(),
        config_path: config_path.to_path_buf(),
        task_name: task::sanitize_task_name(&cfg.task_name),
        dump_dir: PathBuf::from(&cfg.dump_directory),
        max_dump_bytes: pdm_bundle::DEFAULT_MAX_DUMP_BYTES,
    };

    for wf in workflows {
        match wf.as_str() {
            "data" => {
                let exists = |p: &std::path::Path| p.exists();
                let loc = discover::install_location();
                let (jci, tyco) = discover::vendor_roots(loc.as_deref(), &exists);
                let opts = datacoll::Options {
                    components: discover::log_component_paths(&jci, &tyco),
                    ..Default::default()
                };
                datacoll::run(&mut ctx, &opts);
            }
            "install" => installlogs::run(&mut ctx, &installlogs::Options::default()),
            "health" => syshealth::run(&mut ctx, &syshealth::Options::default()),
            "pdm" => {
                let staging = ctx.run_dir.join("ProcDumpMonitor");
                pdm_bundle::run_into(&mut ctx, &pdm_opts, &staging);
                let zip = ctx.run_dir.join("ProcDumpMonitor.zip");
                collect::zip_dir(&mut ctx, &staging, &zip);
            }
            other => ctx.log(&format!("WARN: unknown workflow '{other}' skipped")),
        }
    }
    Ok(ctx.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> { v.iter().map(|x| x.to_string()).collect() }

    #[test]
    fn parses_verbs_with_and_without_dashes() {
        assert!(matches!(parse(&s(&["--monitor"])).unwrap(), Verb::Monitor { .. }));
        assert!(matches!(parse(&s(&["install"])).unwrap(), Verb::Install { .. }));
        assert!(matches!(parse(&s(&["--status"])).unwrap(), Verb::Status { .. }));
        assert!(matches!(parse(&s(&["--version"])).unwrap(), Verb::Version));
        assert!(matches!(parse(&s(&["help"])).unwrap(), Verb::Help));
    }

    #[test]
    fn config_override() {
        let Verb::Monitor { config } = parse(&s(&["--monitor", "--config", r"C:\x\c.json"])).unwrap()
            else { panic!() };
        assert_eq!(config, std::path::PathBuf::from(r"C:\x\c.json"));
    }

    #[test]
    fn bad_verb_and_missing_config_value_error() {
        assert!(parse(&s(&["--frobnicate"])).is_err());
        assert!(parse(&s(&["install", "--config"])).is_err());
    }

    #[test]
    fn empty_args_is_err_not_panic() {
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn collect_defaults_to_all_workflows() {
        let Verb::Collect { out, workflows, .. } = parse(&s(&["collect"])).unwrap() else {
            panic!()
        };
        assert!(out.is_none());
        assert_eq!(workflows, vec!["data", "install", "health", "pdm"]);
    }

    #[test]
    fn collect_parses_out_and_workflow_list() {
        let Verb::Collect { out, workflows, .. } =
            parse(&s(&["collect", "--out", r"C:\t", "--workflows", "data, pdm"])).unwrap()
        else {
            panic!()
        };
        assert_eq!(out.unwrap(), std::path::PathBuf::from(r"C:\t"));
        assert_eq!(workflows, vec!["data", "pdm"]);
    }

    #[test]
    fn collect_rejects_unknown_and_empty_workflows() {
        assert!(parse(&s(&["collect", "--workflows", "frob"])).is_err());
        assert!(parse(&s(&["collect", "--workflows", " , "])).is_err());
    }
}
