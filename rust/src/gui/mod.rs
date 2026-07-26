#![cfg(windows)]
//! Mode-based shell (2026-07-25 spec): freely-clickable sidebar with a
//! MONITOR group (the merged Monitor page) and a LOG COLLECTOR group (Data
//! Collection / Install Logs / System Health), plus About. Replaces the
//! linear Back/Next wizard; the Monitor page's actions live in the footer.

mod collect_runner;
mod dlg_advanced;
mod dlg_smtp;
mod page_about;
mod page_datacoll;
mod page_installlogs;
mod page_monitor;
mod page_syshealth;
mod theme;

use crate::config::Config;
use crate::paths;
use native_windows_gui as nwg;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Shared app state (name kept from the wizard era; pages still read/write
/// the in-progress config through it).
pub struct WizardState {
    pub cfg: RefCell<Config>,
    #[allow(dead_code)]
    pub dirty_scenario: Cell<bool>,
}

const PAGE_COUNT: usize = 5;

const PAGE_TITLES: [&str; PAGE_COUNT] = [
    "Monitor",
    "Data Collection",
    "Install Logs",
    "System Health",
    "About",
];
const PAGE_SUBTITLES: [&str; PAGE_COUNT] = [
    "Pick a target, set dump triggers, install the scheduled task \u{2014} all here.",
    "CCURE application && web logs, plus optional extras.",
    "Installer artifacts driven by InstallHistory.xml.",
    "Uptime, process and service snapshots.",
    "Version and build information.",
];
const SIDEBAR_NAMES: [&str; PAGE_COUNT] =
    ["Monitor", "Data Collection", "Install Logs", "System Health", "About"];

// Sidebar geometry (logical px). Two group captions + five clickable rows.
const ROW_YS: [i32; PAGE_COUNT] = [118, 184, 220, 256, 304];
const GROUP_MONITOR_Y: i32 = 96;
const GROUP_COLLECTOR_Y: i32 = 162;

pub fn run() {
    nwg::init().expect("nwg init failed");

    let mut default_font = nwg::Font::default();
    let _ = nwg::Font::builder().family("Segoe UI").size(15).build(&mut default_font);
    nwg::Font::set_global_default(Some(default_font));

    let state = Rc::new(WizardState {
        cfg: RefCell::new(Config::load(&paths::config_path())),
        dirty_scenario: Cell::new(false),
    });

    let embed = nwg::EmbedResource::load(None).ok();
    let icon = embed.as_ref().and_then(|e| e.icon(1, None));

    // Built hidden; shown only after every control is built AND theme-
    // registered (first-paint ordering — see theme.rs).
    let mut window = nwg::Window::default();
    nwg::Window::builder()
        .size((920, 780))
        .center(true)
        .title("ProcDump Monitor")
        .icon(icon.as_ref())
        .flags(nwg::WindowFlags::WINDOW)
        .build(&mut window)
        .expect("window");
    let window_handle = window.handle;
    theme::attach(&window.handle);

    let app_title_font = theme::semibold(18);
    let item_active_font = theme::semibold(15);
    let group_font = theme::semibold(12);

    // ---- Sidebar chrome ----------------------------------------------------
    let mut app_title = nwg::Label::default();
    nwg::Label::builder()
        .text("ProcDump Monitor")
        .position((24, 28))
        .size((200, 26))
        .parent(&window)
        .build(&mut app_title)
        .expect("app title");
    app_title.set_font(Some(&app_title_font));
    theme::register_sidebar_bg(&app_title.handle);

    let mut app_subtitle = nwg::Label::default();
    nwg::Label::builder()
        .text("Monitor && log collection")
        .position((24, 56))
        .size((200, 18))
        .parent(&window)
        .build(&mut app_subtitle)
        .expect("app subtitle");
    app_subtitle.set_font(Some(theme::subtitle_font()));
    theme::register_sidebar_bg(&app_subtitle.handle);
    theme::register_muted(&app_subtitle.handle);

    let mut group_labels: Vec<nwg::Label> = Vec::new();
    for (text, y) in [("MONITOR", GROUP_MONITOR_Y), ("LOG COLLECTOR", GROUP_COLLECTOR_Y)] {
        let mut l = nwg::Label::default();
        nwg::Label::builder()
            .text(text)
            .position((24, y))
            .size((200, 16))
            .parent(&window)
            .build(&mut l)
            .expect("group label");
        l.set_font(Some(&group_font));
        theme::register_sidebar_bg(&l.handle);
        theme::register_muted(&l.handle);
        group_labels.push(l);
    }

    let mut accent_bar = nwg::Label::default();
    nwg::Label::builder()
        .text("")
        .position((0, ROW_YS[0] - 1))
        .size((3, 24))
        .parent(&window)
        .build(&mut accent_bar)
        .expect("accent bar");
    theme::register_accent_bar(&accent_bar.handle);

    let mut item_labels: Vec<nwg::Label> = Vec::with_capacity(PAGE_COUNT);
    for (i, name) in SIDEBAR_NAMES.iter().enumerate() {
        let mut lbl = nwg::Label::default();
        nwg::Label::builder()
            .text(name)
            .position((24, ROW_YS[i]))
            .size((200, 22))
            .parent(&window)
            .build(&mut lbl)
            .expect("sidebar item");
        lbl.set_font(Some(if i == 0 { &item_active_font } else { theme::body_font() }));
        theme::register_sidebar_bg(&lbl.handle);
        theme::register_muted(&lbl.handle);
        item_labels.push(lbl);
    }
    theme::set_active_step(&item_labels[0].handle);
    let item_handles: Vec<nwg::ControlHandle> = item_labels.iter().map(|l| l.handle).collect();

    let mut version_label = nwg::Label::default();
    nwg::Label::builder()
        .text(&format!("Version {}", env!("CARGO_PKG_VERSION")))
        .position((24, 744))
        .size((200, 18))
        .parent(&window)
        .build(&mut version_label)
        .expect("version label");
    version_label.set_font(Some(theme::subtitle_font()));
    theme::register_sidebar_bg(&version_label.handle);
    theme::register_muted(&version_label.handle);

    // ---- Content header ------------------------------------------------------
    let mut content_title = nwg::Label::default();
    nwg::Label::builder()
        .text(PAGE_TITLES[0])
        .position((272, 32))
        .size((520, 34))
        .parent(&window)
        .build(&mut content_title)
        .expect("content title");
    content_title.set_font(Some(theme::title_font()));

    let mut content_subtitle = nwg::Label::default();
    nwg::Label::builder()
        .text(PAGE_SUBTITLES[0])
        .position((272, 68))
        .size((520, 20))
        .parent(&window)
        .build(&mut content_subtitle)
        .expect("content subtitle");
    content_subtitle.set_font(Some(theme::subtitle_font()));
    theme::register_muted(&content_subtitle.handle);

    // Always-elevated badge (the release manifest requires Administrator).
    let mut admin_pill = nwg::Label::default();
    nwg::Label::builder()
        .text("\u{25CF} Administrator")
        .position((800, 40))
        .size((110, 18))
        .parent(&window)
        .build(&mut admin_pill)
        .expect("admin pill");
    admin_pill.set_font(Some(theme::subtitle_font()));
    theme::register_accent(&admin_pill.handle);

    // ---- Page frames -----------------------------------------------------------
    let mut frames: Vec<nwg::Frame> = Vec::with_capacity(PAGE_COUNT);
    for i in 0..PAGE_COUNT {
        let mut f = nwg::Frame::default();
        nwg::Frame::builder()
            .position((240, 100))
            .size((680, 596))
            .flags(if i == 0 { nwg::FrameFlags::VISIBLE } else { nwg::FrameFlags::NONE })
            .parent(&window)
            .build(&mut f)
            .expect("frame");
        theme::attach(&f.handle);
        frames.push(f);
    }

    // ---- Footer action buttons (Monitor page only) -------------------------------
    fn footer_btn(window: &nwg::Window, text: &str, x: i32, y: i32, w: i32, h: i32) -> nwg::Button {
        let mut b = nwg::Button::default();
        nwg::Button::builder()
            .text(text)
            .position((x, y))
            .size((w, h))
            .parent(window)
            .build(&mut b)
            .expect("footer button");
        b
    }
    let btn_create = footer_btn(&window, "Create Task", 264, 712, 140, 32);
    let btn_run = footer_btn(&window, "Run Now", 412, 712, 110, 32);
    let btn_stop = footer_btn(&window, "Stop", 530, 712, 100, 32);
    let btn_remove = footer_btn(&window, "Remove Task", 638, 712, 130, 32);
    let btn_save_cfg = footer_btn(&window, "Save Config", 264, 748, 108, 26);
    let btn_open_dumps = footer_btn(&window, "Open Dumps", 380, 748, 112, 26);
    let btn_view_logs = footer_btn(&window, "View Logs", 500, 748, 96, 26);
    let btn_copy_args = footer_btn(&window, "Copy Args", 604, 748, 92, 26);
    let btn_taskschd = footer_btn(&window, "Task Scheduler", 704, 748, 118, 26);

    // ---- Pages -------------------------------------------------------------------
    let monitor_page = Rc::new(page_monitor::build(&frames[0], state.clone()));
    let datacoll_page = page_datacoll::build(&frames[1], state.clone());
    let installlogs_page = page_installlogs::build(&frames[2], state.clone());
    let syshealth_page = page_syshealth::build(&frames[3], state.clone());
    let about_page = page_about::build(&frames[4], state.clone());

    monitor_page.load(&state.cfg.borrow());
    monitor_page.refresh_preview(&state);
    monitor_page.refresh_status(&state);

    // ---- Dialogs (owned, reusable; nwg hides them on WM_CLOSE) --------------------
    let window = Rc::new(window);
    let adv_dlg = Rc::new(dlg_advanced::build(&window));
    let smtp_dlg = Rc::new(dlg_smtp::build(&window));

    // ---- Status poll timer ----------------------------------------------------------
    let mut status_timer = nwg::AnimationTimer::default();
    nwg::AnimationTimer::builder()
        .parent(&*window)
        .interval(std::time::Duration::from_millis(3000))
        .active(true)
        .build(&mut status_timer)
        .expect("status timer");
    let timer_h = status_timer.handle;

    // Handles captured for the dispatcher.
    let m = &monitor_page;
    let cmb_target_h = m.cmb_target.handle;
    let btn_refresh_h = m.btn_refresh.handle;
    let chk_show_all_h = m.chk_show_all.handle;
    let cmb_scenario_h = m.cmb_scenario.handle;
    let btn_browse_pd_h = m.btn_browse_pd.handle;
    let btn_browse_dir_h = m.btn_browse_dir.handle;
    let chk_email_h = m.chk_email.handle;
    let chk_webhook_h = m.chk_webhook.handle;
    let btn_advanced_h = m.btn_advanced.handle;
    let btn_smtp_h = m.btn_smtp.handle;

    let dc_browse_h = datacoll_page.btn_browse.handle;
    let dc_all_h = datacoll_page.btn_all.handle;
    let dc_none_h = datacoll_page.btn_none.handle;
    let dc_start_h = datacoll_page.btn_start.handle;
    let dc_open_h = datacoll_page.btn_open.handle;
    let dc_notice_h = datacoll_page.runner.notice.handle;

    let il_browse_h = installlogs_page.btn_browse.handle;
    let il_start_h = installlogs_page.btn_start.handle;
    let il_open_h = installlogs_page.btn_open.handle;
    let il_notice_h = installlogs_page.runner.notice.handle;

    let sh_start_h = syshealth_page.btn_start.handle;
    let sh_open_h = syshealth_page.btn_open.handle;
    let sh_notice_h = syshealth_page.runner.notice.handle;

    let (create_h, run_h, stop_h, remove_h) =
        (btn_create.handle, btn_run.handle, btn_stop.handle, btn_remove.handle);
    let (save_cfg_h, open_dumps_h, view_logs_h, copy_args_h, taskschd_h) = (
        btn_save_cfg.handle,
        btn_open_dumps.handle,
        btn_view_logs.handle,
        btn_copy_args.handle,
        btn_taskschd.handle,
    );

    let current = Cell::new(0usize);

    // ---- Main dispatcher --------------------------------------------------------------
    let handler = {
        let state = state.clone();
        let monitor_page = monitor_page.clone();
        let adv_dlg = adv_dlg.clone();
        let smtp_dlg = smtp_dlg.clone();
        let window_rc = window.clone();
        nwg::full_bind_event_handler(&window_handle, move |evt, _data, handle| {
            match evt {
                nwg::Event::OnWindowClose if handle == window_handle => {
                    nwg::stop_thread_dispatch();
                }

                // ---- Sidebar navigation ----
                nwg::Event::OnLabelClick => {
                    let Some(next) = item_handles.iter().position(|h| *h == handle) else {
                        return;
                    };
                    let cur = current.get();
                    if next == cur {
                        return;
                    }
                    // Save the outgoing page (never blocks — validation
                    // happens on Create Task).
                    match cur {
                        0 => {
                            monitor_page.save(&mut state.cfg.borrow_mut());
                        }
                        4 => {
                            about_page.save(&mut state.cfg.borrow_mut());
                        }
                        _ => {}
                    }
                    if next == 0 {
                        monitor_page.load(&state.cfg.borrow());
                        monitor_page.refresh_preview(&state);
                        monitor_page.refresh_status(&state);
                    } else if next == 4 {
                        about_page.load(&state.cfg.borrow());
                    }

                    frames[cur].set_visible(false);
                    frames[next].set_visible(true);
                    content_title.set_text(PAGE_TITLES[next]);
                    content_subtitle.set_text(PAGE_SUBTITLES[next]);

                    theme::set_active_step(&item_labels[next].handle);
                    for (i, lbl) in item_labels.iter().enumerate() {
                        lbl.set_font(Some(if i == next {
                            &item_active_font
                        } else {
                            theme::body_font()
                        }));
                    }
                    accent_bar.set_position(0, ROW_YS[next] - 1);

                    for b in [
                        &btn_create,
                        &btn_run,
                        &btn_stop,
                        &btn_remove,
                        &btn_save_cfg,
                        &btn_open_dumps,
                        &btn_view_logs,
                        &btn_copy_args,
                        &btn_taskschd,
                    ] {
                        b.set_visible(next == 0);
                    }
                    current.set(next);
                }

                // ---- Live status poll ----
                nwg::Event::OnTimerTick if handle == timer_h => {
                    if current.get() == 0 {
                        monitor_page.refresh_status(&state);
                    }
                }

                // ---- Monitor page ----
                nwg::Event::OnButtonClick if handle == btn_refresh_h || handle == chk_show_all_h => {
                    monitor_page.refresh_targets();
                }
                nwg::Event::OnComboxBoxSelection if handle == cmb_target_h => {
                    monitor_page.on_target_picked(&state);
                }
                nwg::Event::OnComboxBoxSelection if handle == cmb_scenario_h => {
                    monitor_page.on_scenario_selected(&state);
                }
                nwg::Event::OnButtonClick if handle == btn_browse_pd_h => {
                    monitor_page.browse_procdump_path(window_handle);
                }
                nwg::Event::OnButtonClick if handle == btn_browse_dir_h => {
                    monitor_page.browse_dump_dir(window_handle);
                }
                nwg::Event::OnButtonClick if handle == chk_email_h => {
                    monitor_page.on_email_toggled();
                }
                nwg::Event::OnButtonClick if handle == chk_webhook_h => {
                    monitor_page.on_webhook_toggled();
                }
                nwg::Event::OnTextInput | nwg::Event::OnButtonClick | nwg::Event::OnComboxBoxSelection
                    if monitor_page.is_option_control(handle) =>
                {
                    monitor_page.on_option_changed(&state);
                }
                nwg::Event::OnButtonClick if handle == btn_advanced_h => {
                    monitor_page.save(&mut state.cfg.borrow_mut());
                    adv_dlg.open(&state.cfg.borrow(), &monitor_page.manual_target.borrow());
                    window_rc.set_enabled(false);
                }
                nwg::Event::OnButtonClick if handle == btn_smtp_h => {
                    monitor_page.save(&mut state.cfg.borrow_mut());
                    smtp_dlg.open(&state.cfg.borrow());
                    window_rc.set_enabled(false);
                }

                // ---- Footer actions ----
                nwg::Event::OnButtonClick if handle == create_h => {
                    monitor_page.create_task(&state);
                }
                nwg::Event::OnButtonClick if handle == run_h => {
                    monitor_page.run_task(&state);
                }
                nwg::Event::OnButtonClick if handle == stop_h => {
                    monitor_page.stop_task(&state);
                }
                nwg::Event::OnButtonClick if handle == remove_h => {
                    monitor_page.remove_task(&state);
                }
                nwg::Event::OnButtonClick if handle == save_cfg_h => {
                    monitor_page.save_config_only(&state);
                }
                nwg::Event::OnButtonClick if handle == open_dumps_h => {
                    monitor_page.open_dump_folder(&state);
                }
                nwg::Event::OnButtonClick if handle == view_logs_h => {
                    monitor_page.view_logs();
                }
                nwg::Event::OnButtonClick if handle == copy_args_h => {
                    monitor_page.copy_args(&state);
                }
                nwg::Event::OnButtonClick if handle == taskschd_h => {
                    monitor_page.open_task_scheduler();
                }

                // ---- Data Collection page ----
                nwg::Event::OnButtonClick if handle == dc_browse_h => {
                    datacoll_page.browse_save_path(window_handle);
                }
                nwg::Event::OnButtonClick if handle == dc_all_h => {
                    datacoll_page.set_components(true);
                }
                nwg::Event::OnButtonClick if handle == dc_none_h => {
                    datacoll_page.set_components(false);
                }
                nwg::Event::OnButtonClick if handle == dc_start_h => {
                    datacoll_page.start(&state);
                }
                nwg::Event::OnButtonClick if handle == dc_open_h => {
                    datacoll_page.runner.open_last_output();
                }
                nwg::Event::OnNotice if handle == dc_notice_h => {
                    datacoll_page.on_notice();
                }

                // ---- Install Logs page ----
                nwg::Event::OnButtonClick if handle == il_browse_h => {
                    installlogs_page.browse_history(window_handle);
                }
                nwg::Event::OnButtonClick if handle == il_start_h => {
                    installlogs_page.start();
                }
                nwg::Event::OnButtonClick if handle == il_open_h => {
                    installlogs_page.runner.open_last_output();
                }
                nwg::Event::OnNotice if handle == il_notice_h => {
                    installlogs_page.on_notice();
                }

                // ---- System Health page ----
                nwg::Event::OnButtonClick if handle == sh_start_h => {
                    syshealth_page.start();
                }
                nwg::Event::OnButtonClick if handle == sh_open_h => {
                    syshealth_page.runner.open_last_output();
                }
                nwg::Event::OnNotice if handle == sh_notice_h => {
                    syshealth_page.on_notice();
                }

                _ => {}
            }
        })
    };

    // ---- Advanced dialog dispatcher -----------------------------------------------------
    let adv_handler = {
        let state = state.clone();
        let monitor_page = monitor_page.clone();
        let adv = adv_dlg.clone();
        let window_rc = window.clone();
        let adv_window_h = adv_dlg.window.handle;
        let adv_close_h = adv_dlg.btn_close.handle;
        nwg::full_bind_event_handler(&adv_window_h, move |evt, _data, handle| {
            let finalize = || {
                let manual = adv.save(&mut state.cfg.borrow_mut());
                *monitor_page.manual_target.borrow_mut() = manual;
                window_rc.set_enabled(true);
                window_rc.set_focus();
                if adv.dirty.get() {
                    monitor_page.on_advanced_changed(&state);
                }
            };
            match evt {
                nwg::Event::OnButtonClick if handle == adv_close_h => {
                    adv.window.set_visible(false);
                    finalize();
                }
                nwg::Event::OnWindowClose if handle == adv_window_h => {
                    finalize();
                }
                nwg::Event::OnTextInput | nwg::Event::OnButtonClick => {
                    adv.dirty.set(true);
                }
                _ => {}
            }
        })
    };

    // ---- SMTP dialog dispatcher ----------------------------------------------------------
    let smtp_handler = {
        let state = state.clone();
        let smtp = smtp_dlg.clone();
        let window_rc = window.clone();
        let smtp_window_h = smtp_dlg.window.handle;
        let smtp_close_h = smtp_dlg.btn_close.handle;
        let smtp_validate_h = smtp_dlg.btn_validate.handle;
        let smtp_test_h = smtp_dlg.btn_test.handle;
        nwg::full_bind_event_handler(&smtp_window_h, move |evt, _data, handle| {
            let finalize = || {
                smtp.save(&mut state.cfg.borrow_mut());
                smtp.txt_password.set_text("");
                smtp.txt_password.set_placeholder_text(
                    if state.cfg.borrow().encrypted_password_blob.is_empty() {
                        None
                    } else {
                        Some("(unchanged)")
                    },
                );
                window_rc.set_enabled(true);
                window_rc.set_focus();
            };
            match evt {
                nwg::Event::OnButtonClick if handle == smtp_validate_h => {
                    smtp.validate_smtp();
                }
                nwg::Event::OnButtonClick if handle == smtp_test_h => {
                    let snapshot = state.cfg.borrow().clone();
                    smtp.send_test_email(&snapshot);
                }
                nwg::Event::OnButtonClick if handle == smtp_close_h => {
                    smtp.window.set_visible(false);
                    finalize();
                }
                nwg::Event::OnWindowClose if handle == smtp_window_h => {
                    finalize();
                }
                _ => {}
            }
        })
    };

    window.set_visible(true);
    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&smtp_handler);
    nwg::unbind_event_handler(&adv_handler);
    nwg::unbind_event_handler(&handler);
    drop(status_timer);
}
