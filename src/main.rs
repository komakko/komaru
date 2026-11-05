use std::path::PathBuf;

use eframe::egui::ViewportBuilder;
use komaru::{app, identity, platform};

fn main() -> eframe::Result {
    let launch_files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let mut viewport = ViewportBuilder::default()
        .with_title(identity::DISPLAY_NAME)
        .with_app_id(identity::NAME)
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size([960.0, 640.0])
        .with_min_inner_size([360.0, 240.0]);
    if let Some(icon) = platform::window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: platform::wgpu_configuration(),
        clipboard_shortcuts: false,
        event_loop_builder: platform::event_loop_hook(),
        ..Default::default()
    };
    eframe::run_native(
        identity::DISPLAY_NAME,
        options,
        Box::new(|creation_context| {
            Ok(Box::new(app::KomaruApp::new(
                creation_context,
                launch_files,
            )))
        }),
    )
}
