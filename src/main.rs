
#[macro_use] extern crate appinstance;
extern crate libc;
extern crate ws_common;

#[cfg(windows)]
extern crate comdrive;

use comdrive::*;

pub struct RenderDevice
{
    
}
impl RenderDevice
{
    AppInstance!(pub static instance: RenderDevice = RenderDevice::new());
    fn new() -> Self
    {
        match VulkanRenderDevice::new()
        {
            Err(e) =>
            {
                println!("Failed to initialize Vulkan backend({:?}). Falling back into DirectX12 backend", e);
                RenderDevice::DirectX12(D3D12RenderDevice::new().expect("Failed to initialize RenderDevice"))
            },
            Ok(vrd) => RenderDevice::Vulkan(vrd)
        }
    }
}

pub struct Application { main_window: NativeWindow }
impl Application
{
    AppInstance!(pub static instance: Application = Application::new());

    const INITIAL_SIZE: (u16, u16) = (960, 960 * 9 / 16);
    fn new() -> Self
    {
        let main_window = NativeWindow::new(Self::INITIAL_SIZE, "DigitalCampus 2017");
        main_window.show();
        Application { main_window }
    }
    fn process_events(&self)
    {
        WindowServer::instance().process_events();
    }
}

fn main()
{
    println!("=== DIGITAL CAMPUS 2017 ===");
    RenderDevice::instance();
    Application::instance().process_events();
}
