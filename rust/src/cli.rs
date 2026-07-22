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
    Version,
    Help,
}

pub const USAGE: &str = "\
ProcDumpMonitor.exe                     launch the GUI wizard
ProcDumpMonitor.exe <verb> [--config <path>]
  verbs: monitor | install | uninstall | start | stop | status | version | help
  exit codes: 0 = success, 1 = failure, 2 = bad arguments";

pub fn parse(args: &[String]) -> Result<Verb, String> {
    let verb = args[0].trim_start_matches('-').to_ascii_lowercase();
    let mut config = crate::paths::config_path();
    let mut i = 1;
    while i < args.len() {
        match args[i].trim_start_matches('-') {
            "config" => {
                i += 1;
                let v = args.get(i).ok_or("--config requires a path")?;
                config = PathBuf::from(v);
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
    }
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
}
