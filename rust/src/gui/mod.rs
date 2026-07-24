#![cfg(windows)]
mod page_about;
mod page_notify;
mod page_procdump;
mod page_review;
mod page_target;
mod page_task;
mod theme;

use crate::config::Config;
use crate::paths;
use native_windows_gui as nwg;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Shared across every wizard page. `Rc`'d so each page's `build()` can hold
/// a clone and read/write the in-progress config without threading it
/// through every call.
pub struct WizardState {
    pub cfg: RefCell<Config>,
    // ponytail: part of the Task 9 interface contract for Task 10's ProcDump
    // page (tracks whether the user hand-edited a field after picking a
    // scenario preset, so the preset combo can drop back to "Custom").
    // Nothing in Task 9 reads or writes it yet.
    #[allow(dead_code)]
    pub dirty_scenario: Cell<bool>,
}

const STEP_TITLES: [&str; 6] = ["Target", "ProcDump", "Task", "Notify", "Review", "About"];
const LAST_PAGE: usize = STEP_TITLES.len() - 1;

// Content-header text per step (owned by the shell; the pages no longer carry
// their own heading). Kept in step order to match STEP_TITLES.
const PAGE_TITLES: [&str; 6] = [
    "Choose what to monitor",
    "Configure ProcDump",
    "Scheduled task",
    "Notifications",
    "Review & install",
    "About",
];
const PAGE_SUBTITLES: [&str; 6] = [
    "Pick a Windows service or type a process name.",
    "Dump triggers, options, and output location.",
    "How the monitor runs in the background.",
    "Get an email or webhook alert when a dump is captured.",
    "Check the summary, then create or manage the scheduled task.",
    "Version and build information.",
];

// Sidebar step-list geometry (logical px; nwg scales for DPI).
const STEP_Y0: i32 = 96;
const STEP_H: i32 = 40;
const STEP_ROW_INSET: i32 = 8; // centers the 24-tall label/bar in the 40 row

pub fn run() {
    nwg::init().expect("nwg init failed");

    // Global default font (Segoe UI 15px) so any control that doesn't set its
    // own font still matches the wizard body size. Replaces the old bare
    // `set_global_family` call.
    let mut default_font = nwg::Font::default();
    let _ = nwg::Font::builder().family("Segoe UI").size(15).build(&mut default_font);
    nwg::Font::set_global_default(Some(default_font));

    let state = Rc::new(WizardState {
        cfg: RefCell::new(Config::load(&paths::config_path())),
        dirty_scenario: Cell::new(false),
    });

    // Window icon: pull icon id 1 (winresource's default application icon
    // id, see build.rs) out of this exe's own resources -- no second copy
    // of the .ico needs to ship or be loaded from disk at runtime.
    let embed = nwg::EmbedResource::load(None).ok();
    let icon = embed.as_ref().and_then(|e| e.icon(1, None));

    let mut window = nwg::Window::default();
    nwg::Window::builder()
        .size((920, 640))
        .center(true)
        .title("ProcDump Monitor")
        .icon(icon.as_ref())
        .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
        .build(&mut window)
        .expect("window");
    let window_handle = window.handle;

    // White content canvas + gray sidebar + footer divider, and the text
    // coloring the sidebar/header labels rely on.
    theme::attach(&window.handle);

    // Fonts kept alive for the window's lifetime (nwg::Font never frees its
    // HFONT, but building once avoids re-creating it on every nav).
    let app_title_font = theme::semibold(18);
    let step_active_font = theme::semibold(15);

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
        .text("Setup wizard")
        .position((24, 56))
        .size((200, 18))
        .parent(&window)
        .build(&mut app_subtitle)
        .expect("app subtitle");
    app_subtitle.set_font(Some(theme::subtitle_font()));
    theme::register_sidebar_bg(&app_subtitle.handle);
    theme::register_muted(&app_subtitle.handle);

    // Active-step accent bar (3x24) — starts on step 0; moved on nav.
    let mut accent_bar = nwg::Label::default();
    nwg::Label::builder()
        .text("")
        .position((0, STEP_Y0 + STEP_ROW_INSET))
        .size((3, 24))
        .parent(&window)
        .build(&mut accent_bar)
        .expect("accent bar");
    theme::register_accent_bar(&accent_bar.handle);

    // Six step rows.
    let mut step_labels: Vec<nwg::Label> = Vec::with_capacity(STEP_TITLES.len());
    for (i, name) in STEP_TITLES.iter().enumerate() {
        let mut lbl = nwg::Label::default();
        nwg::Label::builder()
            .text(&format!("{}  {}", i + 1, name))
            .position((24, STEP_Y0 + (i as i32) * STEP_H + STEP_ROW_INSET))
            .size((200, 24))
            .parent(&window)
            .build(&mut lbl)
            .expect("step label");
        lbl.set_font(Some(if i == 0 { &step_active_font } else { theme::body_font() }));
        theme::register_sidebar_bg(&lbl.handle);
        theme::register_muted(&lbl.handle);
        step_labels.push(lbl);
    }
    theme::set_active_step(&step_labels[0].handle);

    let mut version_label = nwg::Label::default();
    nwg::Label::builder()
        .text(&format!("Version {}", env!("CARGO_PKG_VERSION")))
        .position((24, 604))
        .size((200, 18))
        .parent(&window)
        .build(&mut version_label)
        .expect("version label");
    version_label.set_font(Some(theme::subtitle_font()));
    theme::register_sidebar_bg(&version_label.handle);
    theme::register_muted(&version_label.handle);

    // ---- Content header (per-page title/subtitle, updated on nav) ----------
    let mut content_title = nwg::Label::default();
    nwg::Label::builder()
        .text(PAGE_TITLES[0])
        .position((272, 32))
        .size((624, 34))
        .parent(&window)
        .build(&mut content_title)
        .expect("content title");
    content_title.set_font(Some(theme::title_font()));

    let mut content_subtitle = nwg::Label::default();
    nwg::Label::builder()
        .text(PAGE_SUBTITLES[0])
        .position((272, 68))
        .size((624, 20))
        .parent(&window)
        .build(&mut content_subtitle)
        .expect("content subtitle");
    content_subtitle.set_font(Some(theme::subtitle_font()));
    theme::register_muted(&content_subtitle.handle);

    // ---- Footer nav --------------------------------------------------------
    let mut back_btn = nwg::Button::default();
    nwg::Button::builder()
        .text("< Back")
        .position((696, 596))
        .size((96, 32))
        .enabled(false)
        .parent(&window)
        .build(&mut back_btn)
        .expect("back");

    let mut next_btn = nwg::Button::default();
    nwg::Button::builder()
        .text("Next >")
        .position((800, 596))
        .size((96, 32))
        .parent(&window)
        .build(&mut next_btn)
        .expect("next");
    let back_h = back_btn.handle;
    let next_h = next_btn.handle;

    // One frame per page, identical rect; pages build their controls inside.
    // Only the current page's frame carries the VISIBLE flag. `VISIBLE` is
    // WS_VISIBLE with no border, so the frame draws no dark box on the canvas.
    let mut frames: Vec<nwg::Frame> = Vec::with_capacity(STEP_TITLES.len());
    for i in 0..STEP_TITLES.len() {
        let mut f = nwg::Frame::default();
        nwg::Frame::builder()
            .position((240, 100))
            .size((680, 456))
            .flags(if i == 0 { nwg::FrameFlags::VISIBLE } else { nwg::FrameFlags::NONE })
            .parent(&window)
            .build(&mut f)
            .expect("frame");
        // Each frame paints its own white body + white label backgrounds.
        theme::attach(&f.handle);
        frames.push(f);
    }

    let target_page = page_target::build(&frames[0], state.clone());
    target_page.load(&state.cfg.borrow());
    let cmb_service_h = target_page.cmb_service.handle;
    let btn_refresh_h = target_page.btn_refresh.handle;
    let chk_show_all_h = target_page.chk_show_all.handle;

    // Pages 1/2 aren't visible at startup -- populated on first nav via the
    // load arm below, same as page 0 above but eagerly.
    let procdump_page = page_procdump::build(&frames[1], state.clone());
    let cmb_scenario_h = procdump_page.cmb_scenario.handle;
    let btn_browse_pd_h = procdump_page.btn_browse_pd.handle;
    let btn_browse_dir_h = procdump_page.btn_browse_dir.handle;

    let task_page = page_task::build(&frames[2], state.clone());
    let btn_reset_auto_h = task_page.btn_reset_auto.handle;
    let btn_copy_cmd_h = task_page.btn_copy_cmd.handle;

    let notify_page = page_notify::build(&frames[3], state.clone());
    let chk_email_h = notify_page.chk_email.handle;
    let chk_webhook_h = notify_page.chk_webhook.handle;
    let btn_validate_h = notify_page.btn_validate.handle;
    let btn_test_email_h = notify_page.btn_test_email.handle;

    let review_page = page_review::build(&frames[4], state.clone());
    let btn_create_h = review_page.btn_create.handle;
    let btn_run_h = review_page.btn_run.handle;
    let btn_stop_h = review_page.btn_stop.handle;
    let btn_remove_h = review_page.btn_remove.handle;
    let btn_save_only_h = review_page.btn_save_only.handle;
    let btn_open_dumps_h = review_page.btn_open_dumps.handle;
    let btn_view_logs_h = review_page.btn_view_logs.handle;
    let btn_copy_args_h = review_page.btn_copy_args.handle;
    let btn_taskschd_h = review_page.btn_taskschd.handle;

    let about_page = page_about::build(&frames[5], state.clone());

    let current = Cell::new(0usize);

    // Single subclass hook for the whole window: `full_bind_event_handler`
    // already walks all children recursively, so wizard nav (Back/Next/
    // Close) and each page's own control events (service refresh, combo
    // pick, ...) live in one dispatcher instead of one hook per control.
    // Task 10/11 add one match arm per new page/control here.
    let handler = nwg::full_bind_event_handler(&window_handle, move |evt, _data, handle| {
        match evt {
            nwg::Event::OnWindowClose if handle == window_handle => {
                nwg::stop_thread_dispatch();
            }
            nwg::Event::OnButtonClick if handle == btn_refresh_h || handle == chk_show_all_h => {
                target_page.refresh_services();
            }
            nwg::Event::OnComboxBoxSelection if handle == cmb_service_h => {
                target_page.on_service_picked();
            }
            nwg::Event::OnComboxBoxSelection if handle == cmb_scenario_h => {
                procdump_page.on_scenario_selected(&state);
            }
            nwg::Event::OnButtonClick if handle == btn_browse_pd_h => {
                procdump_page.browse_procdump_path(window_handle);
            }
            nwg::Event::OnButtonClick if handle == btn_browse_dir_h => {
                procdump_page.browse_dump_dir(window_handle);
            }
            nwg::Event::OnButtonClick if handle == btn_reset_auto_h => {
                task_page.reset_to_auto(&state);
            }
            nwg::Event::OnButtonClick if handle == btn_copy_cmd_h => {
                task_page.copy_command();
            }
            nwg::Event::OnTextInput | nwg::Event::OnButtonClick | nwg::Event::OnComboxBoxSelection
                if procdump_page.is_option_control(handle) =>
            {
                procdump_page.on_option_changed(&state);
            }
            nwg::Event::OnButtonClick if handle == chk_email_h => {
                notify_page.on_email_toggled();
            }
            nwg::Event::OnButtonClick if handle == chk_webhook_h => {
                notify_page.on_webhook_toggled();
            }
            nwg::Event::OnButtonClick if handle == btn_validate_h => {
                notify_page.validate_smtp();
            }
            nwg::Event::OnButtonClick if handle == btn_test_email_h => {
                notify_page.send_test_email(&state);
            }
            nwg::Event::OnButtonClick if handle == btn_create_h => {
                // Mirrors a forward wizard nav's save step across every page
                // (Review itself has no editable fields) so Create/Update
                // Task always installs whatever is currently on-screen, even
                // if the user jumped here without visiting every page this
                // session. Aborts -- without installing -- if Notify's
                // validation fails; it has already shown the error dialog.
                let ok = {
                    let mut cfg = state.cfg.borrow_mut();
                    target_page.save(&mut cfg)
                        && procdump_page.save(&mut cfg)
                        && task_page.save(&mut cfg)
                        && notify_page.save(&mut cfg)
                };
                if ok {
                    review_page.create_task(&state);
                }
            }
            nwg::Event::OnButtonClick if handle == btn_run_h => {
                review_page.run_task();
            }
            nwg::Event::OnButtonClick if handle == btn_stop_h => {
                review_page.stop_task();
            }
            nwg::Event::OnButtonClick if handle == btn_remove_h => {
                review_page.remove_task(&state);
            }
            nwg::Event::OnButtonClick if handle == btn_save_only_h => {
                review_page.save_config_only(&state);
            }
            nwg::Event::OnButtonClick if handle == btn_open_dumps_h => {
                review_page.open_dump_folder(&state);
            }
            nwg::Event::OnButtonClick if handle == btn_view_logs_h => {
                review_page.view_logs();
            }
            nwg::Event::OnButtonClick if handle == btn_copy_args_h => {
                review_page.copy_args(&state);
            }
            nwg::Event::OnButtonClick if handle == btn_taskschd_h => {
                review_page.open_task_scheduler();
            }
            nwg::Event::OnButtonClick if handle == back_h || handle == next_h => {
                let cur = current.get();
                let next = if handle == next_h { cur + 1 } else { cur.saturating_sub(1) };
                if next > LAST_PAGE {
                    return; // Next on the last page: no-op (Back is disabled on page 1, so
                            // a stray click there just reselects the current page below)
                }

                // Every page's save() now returns bool (only Notify's ever
                // returns false, on invalid email settings). A false abort
                // leaves `current`, the frames, and the sidebar/header state
                // untouched -- the user stays put with the error dialog Notify
                // already showed, and no partial state change happened.
                let save_ok = match cur {
                    0 => target_page.save(&mut state.cfg.borrow_mut()),
                    1 => procdump_page.save(&mut state.cfg.borrow_mut()),
                    2 => task_page.save(&mut state.cfg.borrow_mut()),
                    3 => notify_page.save(&mut state.cfg.borrow_mut()),
                    4 => review_page.save(&mut state.cfg.borrow_mut()),
                    5 => about_page.save(&mut state.cfg.borrow_mut()),
                    _ => true,
                };
                if !save_ok {
                    return;
                }

                // Load-before-show: a frame is never made visible before its
                // controls are repopulated for the config it's about to
                // display (fixed from Task 9's save -> toggle -> load order).
                match next {
                    0 => target_page.load(&state.cfg.borrow()),
                    1 => {
                        procdump_page.load(&state.cfg.borrow());
                        procdump_page.refresh_preview(&state);
                    }
                    2 => task_page.load(&state.cfg.borrow()),
                    3 => notify_page.load(&state.cfg.borrow()),
                    4 => review_page.load(&state.cfg.borrow()),
                    5 => about_page.load(&state.cfg.borrow()),
                    _ => {}
                }

                frames[cur].set_visible(false);
                frames[next].set_visible(true);

                // Content header text for the new page.
                content_title.set_text(PAGE_TITLES[next]);
                content_subtitle.set_text(PAGE_SUBTITLES[next]);

                // Sidebar active-step indicator: set the active row first so the
                // font-driven repaints below pick up the new accent/muted text
                // colors, then move the accent bar to the active row.
                theme::set_active_step(&step_labels[next].handle);
                for (i, lbl) in step_labels.iter().enumerate() {
                    lbl.set_font(Some(if i == next { &step_active_font } else { theme::body_font() }));
                }
                accent_bar.set_position(0, STEP_Y0 + (next as i32) * STEP_H + STEP_ROW_INSET);

                back_btn.set_enabled(next > 0);
                next_btn.set_enabled(next < LAST_PAGE);

                current.set(next);
            }
            _ => {}
        }
    });

    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
}
