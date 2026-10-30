fn main() {
    println!("cargo:rerun-if-changed=packaging/icons/forma.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("packaging/icons/forma.ico")
            .set("ProductName", "Forma")
            .set("FileDescription", "Forma")
            .compile()
            .expect("could not embed the Windows application icon");
    }
}
