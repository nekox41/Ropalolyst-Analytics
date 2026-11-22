fn main() {
    println!("cargo:rerun-if-changed=../../logo.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winres::WindowsResource::new();
        resource.set_icon("../../logo.ico");
        resource
            .compile()
            .expect("failed to embed the Ropalolyst Analytics application icon");
    }
}
