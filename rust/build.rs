fn main() {
    println!(
        "cargo:rustc-env=BUILD_DATE={}",
        chrono::Local::now().format("%m.%d.%y")
    );
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        #[cfg(windows)]
        {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/jci_globe.ico");
            res.set_manifest_file("app.manifest");
            res.compile().expect("resource compile failed");
        }
    }
}
