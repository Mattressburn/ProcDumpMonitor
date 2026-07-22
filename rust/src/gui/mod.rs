#![cfg(windows)]
mod page_target;
// Task 10 adds: mod page_procdump; mod page_task;
// Task 11 adds: mod page_notify; mod page_review; mod page_about;

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

pub fn run() {
    nwg::init().expect("nwg init failed");
    let _ = nwg::Font::set_global_family("Segoe UI");

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
        .size((780, 580))
        .center(true)
        .title("ProcDump Monitor")
        .icon(icon.as_ref())
        .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
        .build(&mut window)
        .expect("window");
    let window_handle = window.handle;

    let mut bold_font = nwg::Font::default();
    let _ = nwg::Font::builder().family("Segoe UI").weight(700).build(&mut bold_font);

    let mut step_label = nwg::Label::default();
    nwg::Label::builder()
        .text(&format!("Step 1 of {} - {}", STEP_TITLES.len(), STEP_TITLES[0]))
        .position((10, 8))
        .size((600, 24))
        .parent(&window)
        .build(&mut step_label)
        .expect("step label");
    step_label.set_font(Some(&bold_font));

    let mut back_btn = nwg::Button::default();
    nwg::Button::builder()
        .text("< Back")
        .position((10, 530))
        .size((90, 30))
        .enabled(false)
        .parent(&window)
        .build(&mut back_btn)
        .expect("back");

    let mut next_btn = nwg::Button::default();
    nwg::Button::builder()
        .text("Next >")
        .position((680, 530))
        .size((90, 30))
        .parent(&window)
        .build(&mut next_btn)
        .expect("next");
    let back_h = back_btn.handle;
    let next_h = next_btn.handle;

    // One frame per page, identical rect; pages build their controls inside.
    // Only the current page's frame carries the VISIBLE flag.
    let mut frames: Vec<nwg::Frame> = Vec::with_capacity(STEP_TITLES.len());
    for i in 0..STEP_TITLES.len() {
        let mut f = nwg::Frame::default();
        nwg::Frame::builder()
            .position((10, 40))
            .size((760, 480))
            .flags(if i == 0 { nwg::FrameFlags::VISIBLE } else { nwg::FrameFlags::NONE })
            .parent(&window)
            .build(&mut f)
            .expect("frame");
        frames.push(f);
    }

    let target_page = page_target::build(&frames[0], state.clone());
    target_page.load(&state.cfg.borrow());
    let cmb_service_h = target_page.cmb_service.handle;
    let btn_refresh_h = target_page.btn_refresh.handle;
    let chk_show_all_h = target_page.chk_show_all.handle;

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
            nwg::Event::OnButtonClick if handle == back_h || handle == next_h => {
                let cur = current.get();
                let next = if handle == next_h { cur + 1 } else { cur.saturating_sub(1) };
                if next > LAST_PAGE {
                    return; // Next on the last page: no-op (Back is disabled on page 1, so
                            // a stray click there just reselects the current page below)
                }

                if cur == 0 {
                    target_page.save(&mut state.cfg.borrow_mut());
                }
                // Task 10/11: save arm for pages 1..=LAST_PAGE

                frames[cur].set_visible(false);
                frames[next].set_visible(true);

                if next == 0 {
                    target_page.load(&state.cfg.borrow());
                }
                // Task 10/11: load arm for pages 1..=LAST_PAGE

                step_label.set_text(&format!(
                    "Step {} of {} - {}",
                    next + 1,
                    STEP_TITLES.len(),
                    STEP_TITLES[next]
                ));
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
