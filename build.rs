fn main() {
    println!("cargo:rerun-if-changed=packaging/icons/json-lens.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("packaging/icons/json-lens.ico")
            .set("ProductName", "JSON Lens")
            .set("FileDescription", "JSON Lens")
            .compile()
            .expect("could not embed the Windows application icon");
    }
}
