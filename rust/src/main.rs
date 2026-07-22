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
    native_windows_gui::init().expect("nwg init failed");
    let mut window = Default::default();
    let mut label = Default::default();
    native_windows_gui::Window::builder()
        .size((360, 120))
        .title("PDM Spike")
        .build(&mut window)
        .unwrap();
    native_windows_gui::Label::builder()
        .text("nwg + requireAdministrator OK")
        .size((320, 40))
        .position((20, 30))
        .parent(&window)
        .build(&mut label)
        .unwrap();
    let handler = native_windows_gui::full_bind_event_handler(
        &window.handle,
        move |evt, _data, _handle| {
            if evt == native_windows_gui::Event::OnWindowClose {
                native_windows_gui::stop_thread_dispatch();
            }
        },
    );
    native_windows_gui::dispatch_thread_events();
    native_windows_gui::unbind_event_handler(&handler);
}

#[cfg(not(windows))]
fn main() {
    eprintln!("ProcDumpMonitor targets Windows; Linux builds are for `cargo test` only.");
}
