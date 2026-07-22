use crate::config::Config;
use native_windows_gui as nwg;
use std::rc::Rc;

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
    #[allow(dead_code)]
    img_logo: nwg::ImageFrame,
    #[allow(dead_code)]
    lbl_name: nwg::Label,
    #[allow(dead_code)]
    lbl_tagline: nwg::Label,
    #[allow(dead_code)]
    lbl_build: nwg::Label,
}

pub fn build(parent: &nwg::Frame, _state: Rc<super::WizardState>) -> AboutPage {
    let bitmap = nwg::Bitmap::from_bin(include_bytes!("../../assets/jci_globe_256.png"))
        .expect("decode embedded jci_globe_256.png");

    let mut img_logo = nwg::ImageFrame::default();
    nwg::ImageFrame::builder()
        .position((300, 10))
        .size((160, 160))
        .bitmap(Some(&bitmap))
        .parent(parent)
        .build(&mut img_logo)
        .unwrap();

    let mut name_font = nwg::Font::default();
    let _ = nwg::Font::builder().family("Segoe UI").weight(700).size(28).build(&mut name_font);

    let mut lbl_name = nwg::Label::default();
    nwg::Label::builder()
        .text("ProcDump Monitor")
        .position((0, 184))
        .size((760, 36))
        .h_align(nwg::HTextAlign::Center)
        .font(Some(&name_font))
        .parent(parent)
        .build(&mut lbl_name)
        .unwrap();

    let mut lbl_tagline = nwg::Label::default();
    nwg::Label::builder()
        .text("A SWH L3 Production \u{2014} packaged for C\u{2022}CURE deployments.")
        .position((0, 226))
        .size((760, 24))
        .h_align(nwg::HTextAlign::Center)
        .parent(parent)
        .build(&mut lbl_tagline)
        .unwrap();

    let mut lbl_build = nwg::Label::default();
    nwg::Label::builder()
        .text(&format!("Build {}  \u{b7}  v{}", env!("BUILD_DATE"), env!("CARGO_PKG_VERSION")))
        .position((0, 256))
        .size((760, 24))
        .h_align(nwg::HTextAlign::Center)
        .parent(parent)
        .build(&mut lbl_build)
        .unwrap();

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
