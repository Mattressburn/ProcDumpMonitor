fn main() {
    println!(
        "cargo:rustc-env=BUILD_DATE={}",
        chrono::Local::now().format("%m.%d.%y")
    );
    // Automation builds set PDM_TEST_MANIFEST=1 to embed the asInvoker manifest
    // so the exe launches without a UAC prompt (release keeps requireAdministrator).
    println!("cargo:rerun-if-env-changed=PDM_TEST_MANIFEST");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        #[cfg(windows)]
        {
            let manifest = if std::env::var("PDM_TEST_MANIFEST").as_deref() == Ok("1") {
                // A release exe must never ship asInvoker: it would silently drop
                // the requireAdministrator elevation the app depends on.
                if std::env::var("PROFILE").as_deref() == Ok("release") {
                    panic!("PDM_TEST_MANIFEST=1 is not allowed for release builds; unset it");
                }
                "app.test.manifest"
            } else {
                "app.manifest"
            };
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/jci_globe.ico");
            res.set_manifest_file(manifest);
            res.compile().expect("resource compile failed");
        }
    }
}
