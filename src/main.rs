#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;

use eframe::egui::ViewportBuilder;
use komaru::cli::{self, Launch};
use komaru::{app, identity, platform};

fn main() -> eframe::Result {
    match cli::parse(std::env::args_os().skip(1)) {
        Launch::Viewer(files) => run_viewer(files),
        Launch::Install => std::process::exit(cli::install()),
        Launch::Uninstall => std::process::exit(cli::uninstall()),
        Launch::ApplyUpdate { build, signature } => {
            std::process::exit(cli::apply_update(&build, &signature))
        }
    }
}

fn run_viewer(launch_files: Vec<PathBuf>) -> eframe::Result {
    let mut viewport = ViewportBuilder::default()
        .with_title(identity::DISPLAY_NAME)
        .with_app_id(identity::NAME)
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size([550.0, 400.0])
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
