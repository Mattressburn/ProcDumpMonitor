// `list()` (Windows-only) is consumed by the GUI's Target page (Task 9); on a
// non-Windows `cargo build` (no GUI, no `list()`) that leaves `parse_sc_output`/
// `ServiceInfo` looking unused outside of `cfg(test)`, hence the narrow allow.
#![cfg_attr(not(windows), allow(dead_code))]

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub display: String,
    pub running: bool,
}

/// Parse `sc.exe query type= service state= all` output. Locale caveat:
/// SERVICE_NAME/DISPLAY_NAME/STATE tokens are not localized (verified en-US;
/// C-CURE deployments are en-US).
pub fn parse_sc_output(out: &str) -> Vec<ServiceInfo> {
    let mut result = Vec::new();
    let mut cur: Option<ServiceInfo> = None;
    for line in out.lines() {
        let line = line.trim_end();
        if let Some(name) = line.strip_prefix("SERVICE_NAME: ") {
            if let Some(svc) = cur.take() { result.push(svc); }
            cur = Some(ServiceInfo { name: name.trim().into(), display: String::new(), running: false });
        } else if let Some(disp) = line.trim_start().strip_prefix("DISPLAY_NAME: ") {
            if let Some(svc) = cur.as_mut() { svc.display = disp.trim().into(); }
        } else if line.trim_start().starts_with("STATE") {
            if let Some(svc) = cur.as_mut() {
                svc.running = line.contains(" 4 ") || line.ends_with("RUNNING");
            }
        }
    }
    if let Some(svc) = cur { result.push(svc); }
    result
}

#[cfg(windows)]
pub fn list() -> Vec<ServiceInfo> {
    std::process::Command::new("sc.exe")
        .args(["query", "type=", "service", "state=", "all"])
        .output()
        .map(|o| parse_sc_output(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SC_SAMPLE: &str = "\
SERVICE_NAME: Spooler\r
DISPLAY_NAME: Print Spooler\r
        TYPE               : 110  WIN32_OWN_PROCESS (interactive)\r
        STATE              : 4  RUNNING\r
                                (STOPPABLE, NOT_PAUSABLE, ACCEPTS_SHUTDOWN)\r
\r
SERVICE_NAME: Fax\r
DISPLAY_NAME: Fax\r
        TYPE               : 10  WIN32_OWN_PROCESS\r
        STATE              : 1  STOPPED\r
\r
";

    #[test]
    fn parses_name_display_state() {
        let svcs = parse_sc_output(SC_SAMPLE);
        assert_eq!(svcs.len(), 2);
        assert_eq!(svcs[0].name, "Spooler");
        assert_eq!(svcs[0].display, "Print Spooler");
        assert!(svcs[0].running);
        assert_eq!(svcs[1].name, "Fax");
        assert!(!svcs[1].running);
    }
}
