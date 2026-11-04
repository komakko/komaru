#[cfg(all(unix, not(target_os = "macos")))]
pub fn event_loop_hook() -> Option<eframe::EventLoopBuilderHook> {
    use winit::platform::x11::EventLoopBuilderExtX11;

    std::env::var_os("DISPLAY")?;
    Some(Box::new(|builder| {
        builder.with_x11();
    }))
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn event_loop_hook() -> Option<eframe::EventLoopBuilderHook> {
    None
}
