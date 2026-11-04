use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu::{Backends, Dx12SwapchainKind};

pub fn wgpu_configuration() -> WgpuConfiguration {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    if cfg!(windows) {
        setup.instance_descriptor.backends = Backends::DX12;
        setup
            .instance_descriptor
            .backend_options
            .dx12
            .presentation_system = Dx12SwapchainKind::DxgiFromVisual;
    }
    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}
