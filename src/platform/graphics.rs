use eframe::egui_wgpu::{SurfaceConfig, WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu::{Backends, Dx12SwapchainKind, PresentMode};

pub fn wgpu_configuration() -> WgpuConfiguration {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    let mut present_mode = PresentMode::AutoVsync;
    if cfg!(windows) {
        setup.instance_descriptor.backends = Backends::DX12;
        setup
            .instance_descriptor
            .backend_options
            .dx12
            .presentation_system = Dx12SwapchainKind::DxgiFromVisual;
        present_mode = PresentMode::AutoNoVsync;
    }
    WgpuConfiguration {
        surface: SurfaceConfig {
            present_mode,
            ..SurfaceConfig::HIGH_THROUGHPUT
        },
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}
