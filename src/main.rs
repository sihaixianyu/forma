#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
#[cfg(target_arch = "wasm32")]
mod browser;
mod canvas;
mod ui;
mod worker;

#[cfg(not(target_arch = "wasm32"))]
use app::App;
#[cfg(not(target_arch = "wasm32"))]
use eframe::egui;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    forma::diagnostics::init();
    #[cfg(feature = "bench-ui")]
    if std::env::args().any(|arg| arg == "--bench-editor") {
        app::benchmark::run();
        return Ok(());
    }
    let viewport = egui::ViewportBuilder::default()
        .with_app_id("forma")
        .with_title("Forma")
        .with_inner_size(if std::env::var_os("FORMA_NARROW").is_some() {
            [900.0, 650.0]
        } else {
            [1280.0, 800.0]
        })
        .with_min_inner_size([800.0, 500.0])
        .with_icon(
            eframe::icon_data::from_png_bytes(include_bytes!("../packaging/icons/forma-256.png"))
                .expect("bundled application icon must be a valid PNG"),
        );
    #[cfg(target_os = "macos")]
    let viewport = viewport
        .with_fullsize_content_view(false)
        .with_titlebar_shown(true)
        .with_title_shown(true);
    eframe::run_native(
        "Forma",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    forma::diagnostics::init();
    if web_sys::window().is_some() {
        browser::start();
    }
}
