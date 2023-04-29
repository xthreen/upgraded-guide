#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

fn load_icon(path: &str) -> eframe::IconData {
    let (icon_rgba, icon_width, icon_height) = {
        let icon = image::open(path).expect("Failed to load icon").into_rgba8();
        let (width, height) = icon.dimensions();
        let rgba = icon.into_raw();
        (rgba, width, height)
    };

    eframe::IconData {
        rgba: icon_rgba,
        width: icon_width,
        height: icon_height,
    }
}

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions {
        icon_data: Some(load_icon("assets/tesseract-logo-houndstoothed-alpha.ico")),
        initial_window_size: Some([960.0, 480.0].into()),
        min_window_size: Some([768.0, 480.0].into()),
        transparent: true,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Functional Rust UI Demo",
        native_options,
        Box::new(|cc| Box::new(functional_rust_ui_demo::TemplateApp::new(cc))),
    )
}

// when compiling to web using trunk.
#[cfg(target_arch = "wasm32")]
fn main() {
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    // Redirect tracing to console.log and friends:
    tracing_wasm::set_as_global_default();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::start_web(
            "the_canvas_id", // hardcode it
            web_options,
            Box::new(|cc| Box::new(functional_rust_ui_demo::TemplateApp::new(cc))),
        )
        .await
        .expect("failed to start eframe");
    });
}
