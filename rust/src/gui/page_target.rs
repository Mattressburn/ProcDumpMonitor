use crate::config::{Config, TargetType};
use crate::services;
use native_windows_gui as nwg;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TargetPage {
    pub txt_process: nwg::TextInput,
    pub cmb_service: nwg::ComboBox<String>,
    pub chk_show_all: nwg::CheckBox,
    pub btn_refresh: nwg::Button,
    /// Parallel to `cmb_service`'s items.
    pub services: RefCell<Vec<services::ServiceInfo>>,
    pub picked_service: RefCell<Option<String>>,
}

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> TargetPage {
    let mut lbl = nwg::Label::default();
    nwg::Label::builder()
        .text("Process Name (no .exe):")
        .position((10, 20))
        .size((150, 22))
        .parent(parent)
        .build(&mut lbl)
        .unwrap();
    let mut txt_process = nwg::TextInput::default();
    nwg::TextInput::builder()
        .position((170, 18))
        .size((380, 24))
        .parent(parent)
        .build(&mut txt_process)
        .unwrap();

    let mut lbl2 = nwg::Label::default();
    nwg::Label::builder()
        .text("Select Service:")
        .position((10, 60))
        .size((150, 22))
        .parent(parent)
        .build(&mut lbl2)
        .unwrap();
    let mut cmb_service = nwg::ComboBox::default();
    nwg::ComboBox::builder()
        .position((170, 58))
        .size((380, 26))
        .parent(parent)
        .build(&mut cmb_service)
        .unwrap();

    let mut chk_show_all = nwg::CheckBox::default();
    nwg::CheckBox::builder()
        .text("Show all services")
        .position((170, 95))
        .size((140, 24))
        .parent(parent)
        .build(&mut chk_show_all)
        .unwrap();
    let mut btn_refresh = nwg::Button::default();
    nwg::Button::builder()
        .text("Refresh Services")
        .position((320, 92))
        .size((120, 26))
        .parent(parent)
        .build(&mut btn_refresh)
        .unwrap();

    let mut hint = nwg::Label::default();
    nwg::Label::builder()
        .text("Picking a service fills the name and targets it as a service; typing targets a process.")
        .position((10, 140))
        .size((740, 22))
        .parent(parent)
        .build(&mut hint)
        .unwrap();

    let page = TargetPage {
        txt_process,
        cmb_service,
        chk_show_all,
        btn_refresh,
        services: RefCell::new(Vec::new()),
        picked_service: RefCell::new(None),
    };
    page.refresh_services();
    page
}

impl TargetPage {
    pub fn refresh_services(&self) {
        let all = services::list();
        let show_all = self.chk_show_all.check_state() == nwg::CheckBoxState::Checked;
        let filtered: Vec<services::ServiceInfo> =
            all.into_iter().filter(|s| show_all || s.running).collect();
        let labels = filtered.iter().map(|s| format!("{} ({})", s.display, s.name)).collect();
        self.cmb_service.set_collection(labels);
        *self.services.borrow_mut() = filtered;
    }

    /// Wired into gui::run's event handler:
    /// - OnComboxBoxSelection on cmb_service -> on_service_picked()
    /// - OnButtonClick on btn_refresh and on chk_show_all -> refresh_services()
    pub fn on_service_picked(&self) {
        if let Some(i) = self.cmb_service.selection() {
            if let Some(svc) = self.services.borrow().get(i) {
                self.txt_process.set_text(&svc.name);
                *self.picked_service.borrow_mut() = Some(svc.name.clone());
            }
        }
    }

    pub fn load(&self, cfg: &Config) {
        self.txt_process.set_text(&cfg.target_name);
        *self.picked_service.borrow_mut() = match cfg.target_type {
            TargetType::Service => Some(cfg.target_name.clone()),
            TargetType::Process => None,
        };
    }

    /// Always succeeds -- Target never blocks navigation. Returns `bool` to
    /// match the page contract every page shares (only Notify's `save` ever
    /// returns `false`, on invalid email settings).
    pub fn save(&self, cfg: &mut Config) -> bool {
        let typed = self.txt_process.text().trim().to_string();
        // If the text still equals the last-picked service name, the user
        // hasn't retyped over it -> still targeting that service. Decision
        // lives in config::infer_target_type so it's unit-tested on Linux
        // (this file only compiles on Windows).
        let picked = self.picked_service.borrow();
        cfg.target_type = crate::config::infer_target_type(&typed, picked.as_deref());
        // Auto task name follows target when the user hasn't customized it
        // (Task 10's Task page re-checks this same condition).
        if cfg.task_name == crate::task::auto_task_name(&cfg.target_name)
            || cfg.task_name == "ProcDump Monitor"
        {
            cfg.task_name = crate::task::auto_task_name(&typed);
        }
        cfg.target_name = typed;
        true
    }
}
