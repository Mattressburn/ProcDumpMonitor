#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod paths;
mod config;
mod procdump;
mod task;
mod health;
mod retention;
mod stability;
mod diskguard;
mod notify;
mod secrets;
mod bitness;
mod services;
mod cli;
mod logger;
#[cfg(windows)]
mod monitor;

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("GUI arrives in Task 9");
        return;
    }
    attach_console();
    let code = match cli::parse(&args) {
        Ok(verb) => cli::run(verb),
        Err(e) => {
            eprintln!("ERROR: {e}\n{}", cli::USAGE);
            2
        }
    };
    std::process::exit(code);
}

/// windows_subsystem = "windows" means the OS never populated this process's
/// std handles, so AttachConsole alone is NOT enough to make println!/
/// eprintln! work again (they panic on a broken write, which aborts under
/// panic = "abort") -- CONOUT$ must be reopened and installed explicitly.
/// Skips any handle the caller already redirected (e.g. `> out.txt`, a
/// pipe, or PowerShell capturing `$x = ...`) so we never clobber a handle
/// that already works. No-op when there's no parent console at all
/// (launched by Task Scheduler / Explorer with nothing to attach to).
#[cfg(windows)]
fn attach_console() {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{
        AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_HANDLE, STD_OUTPUT_HANDLE,
    };

    fn is_broken(h: STD_HANDLE) -> bool {
        match unsafe { GetStdHandle(h) } {
            Ok(handle) => handle.is_invalid() || handle.0.is_null(),
            Err(_) => true,
        }
    }

    if !is_broken(STD_OUTPUT_HANDLE) && !is_broken(STD_ERROR_HANDLE) {
        return; // caller already redirected both -- leave them alone
    }
    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_err() {
        return; // no parent console to attach to
    }
    if let Ok(conout) = std::fs::OpenOptions::new().read(true).write(true).open("CONOUT$") {
        let h = HANDLE(conout.as_raw_handle() as _);
        unsafe {
            if is_broken(STD_OUTPUT_HANDLE) {
                let _ = SetStdHandle(STD_OUTPUT_HANDLE, h);
            }
            if is_broken(STD_ERROR_HANDLE) {
                let _ = SetStdHandle(STD_ERROR_HANDLE, h);
            }
        }
        std::mem::forget(conout); // handle now owned by the process's stdio
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("ProcDumpMonitor targets Windows; Linux builds are for `cargo test` only.");
}
