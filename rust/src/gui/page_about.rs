use crate::config::Config;
use native_windows_gui as nwg;
use std::rc::Rc;

use super::theme;

pub struct AboutPage {
    // Kept alive only for their Drop side effects (Font is fine to drop --
    // labels copy the HFONT internally on set_font -- but Bitmap is not: the
    // ImageFrame's STATIC control just holds a handle to it via
    // STM_SETIMAGE, so dropping `bitmap` here would free the GDI object
    // out from under a control that's still displaying it).
    #[allow(dead_code)]
    name_font: nwg::Font,
    #[allow(dead_code)]
    bitmap: nwg::Bitmap,
    // CONTROL LIFETIME: every static created in build() is stored here.
    // nwg destroys a control's HWND the moment its Rust value drops, so a
    // label kept only as a local in build() would vanish the instant this
    // function returns -- that's the historical bug the redesign fixes.
    #[allow(dead_code)]
    img_logo: nwg::ImageFrame,
    #[allow(dead_code)]
    lbl_name: nwg::Label,
    #[allow(dead_code)]
    lbl_tagline: nwg::Label,
    #[allow(dead_code)]
    lbl_build: nwg::Label,
}

/// Page frame is 680x456 (design-system.md). Unlike the form-grid pages, About
/// has no label/field rows -- per Task 7's brief this is a centered hero
/// block: logo, product name, then muted detail lines below, stacked and
/// centered on both axes within the frame.
pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> AboutPage {
    let bitmap = nwg::Bitmap::from_bin(include_bytes!("../../assets/jci_globe_256.png"))
        .expect("decode embedded jci_globe_256.png");

    // 160x160 logo, centered horizontally in the 680-wide frame: x = (680-160)/2.
    let mut img_logo = nwg::ImageFrame::default();
    nwg::ImageFrame::builder()
        .position((260, 88))
        .size((160, 160))
        .bitmap(Some(&bitmap))
        .parent(parent)
        .build(&mut img_logo)
        .unwrap();

    let name_font = theme::semibold(24);

    let mut lbl_name = nwg::Label::default();
    nwg::Label::builder()
        .text("ProcDump Monitor")
        .position((0, 264))
        .size((680, 36))
        .h_align(nwg::HTextAlign::Center)
        .font(Some(&name_font))
        .parent(parent)
        .build(&mut lbl_name)
        .unwrap();

    let mut lbl_tagline = nwg::Label::default();
    nwg::Label::builder()
        .text("A SWH L3 Production \u{2014} packaged for C\u{2022}CURE deployments.")
        .position((0, 312))
        .size((680, 24))
        .h_align(nwg::HTextAlign::Center)
        .parent(parent)
        .build(&mut lbl_tagline)
        .unwrap();
    theme::register_muted(&lbl_tagline.handle);

    let mut lbl_build = nwg::Label::default();
    nwg::Label::builder()
        .text(&format!("Build {}  \u{b7}  v{}", env!("BUILD_DATE"), env!("CARGO_PKG_VERSION")))
        .position((0, 344))
        .size((680, 24))
        .h_align(nwg::HTextAlign::Center)
        .parent(parent)
        .build(&mut lbl_build)
        .unwrap();
    theme::register_muted(&lbl_build.handle);

    AboutPage { name_font, bitmap, img_logo, lbl_name, lbl_tagline, lbl_build }
}

impl AboutPage {
    /// Nothing on this page depends on the config -- kept only so gui::run's
    /// nav handler can treat every page uniformly (one load/save arm per
    /// page index).
    pub fn load(&self, _cfg: &Config) {}

    pub fn save(&self, _cfg: &mut Config) -> bool {
        true
    }
}
