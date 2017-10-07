
#[macro_use] extern crate appinstance;
extern crate libc;
extern crate ferrite;
extern crate ws_common;

use ws_common::{NativeWindow, WindowServer};
use ferrite as fe;
use ferrite::Waitable;
use std::cmp::max;
use std::io::{Result as IOResult, Error as IOError, ErrorKind};
use std::ffi::CStr;

pub struct VulkanRenderDevice
{
    instance: fe::Instance, adapter: fe::PhysicalDevice, device: fe::Device, graphics_queue: (u32, fe::Queue), transfer_queue: (u32, fe::Queue),
    surface: fe::Surface, swapchain: fe::Swapchain, rt_views: Vec<fe::ImageView>,
    #[cfg(debug)] debug_report: fe::DebugReportCallback
}
impl VulkanRenderDevice
{
    #[cfg(feature = "target_x11")] const PLATFORM_SURFACE_EXTENSION: &'static str = "VK_KHR_xcb_surface";
    #[cfg(windows)] const PLATFORM_SURFACE_EXTENSION: &'static str = "VK_KHR_win32_surface";

    fn new() -> fe::Result<Self>
    {
        let mut ibuilder = fe::InstanceBuilder::new("dc2017", (0, 1, 0), "ferrite", (0, 1, 0));
        ibuilder.add_extensions(vec!["VK_KHR_surface", Self::PLATFORM_SURFACE_EXTENSION]);
        #[cfg(debug)] ibuilder.add_extension("VK_EXT_debug_report").add_layer("VK_LAYER_LUNARG_standard_validation");
        let instance = ibuilder.create()?;
        #[cfg(debug)]
        let debug_report = fe::DebugReportCallback::<()>::new(&instance, fe::DebugReportFlags::ERROR.warning().performance_warning(),
            Self::debug_call, None).expect("Failed to create a debug reporter object");
        let adapter = instance.enumerate_physical_devices().expect("PhysicalDevices are not found").remove(0);
        let adapter_properties = adapter.properties();
        println!("RenderDevice: Vulkan 1.0 on {}", unsafe { CStr::from_ptr(adapter_properties.deviceName.as_ptr()) }.to_str().unwrap());
        let queue_families = adapter.queue_family_properties();
        let graphics_qf = queue_families.find_matching_index(fe::QueueFlags::GRAPHICS).expect("Failed to find graphics queue family");
        let transfer_qf = queue_families.find_matching_index(fe::QueueFlags::TRANSFER).unwrap_or(graphics_qf);
        let queues =
            if graphics_qf != transfer_qf { vec![fe::DeviceQueueCreateInfo(graphics_qf, vec![0.0]), fe::DeviceQueueCreateInfo(transfer_qf, vec![0.0])] }
            else { vec![fe::DeviceQueueCreateInfo(graphics_qf, vec![0.0; 2])] };
        let device =
        {
            let devbuilder = fe::DeviceBuilder::new(&adapter)
                .add_extension("VK_KHR_swapchain");
            #[cfg(debug)]
            let devbuilder = devbuilder.add_extension("VK_EXT_debug_report")
                .add_layer("VK_LAYER_LUNARG_standard_validation");
            devbuilder.add_queues(queues).create().expect("Failed to create device")
        };
        let gq = (graphics_qf, device.queue(graphics_qf, 0));
        let tq = (transfer_qf, device.queue(transfer_qf, if graphics_qf == transfer_qf { 1 } else { 0 }));

        let ref target = Application::instance().main_window;
        if !WindowServer::instance().presentation_support(&adapter, graphics_qf)
        {
            panic!("System doesn't have Vulkan Presentation support");
        }
        let surface = WindowServer::instance().new_render_surface(target, &instance).expect("Failed to create Render Surface");
        if !adapter.surface_support(graphics_qf, &surface).expect("Failed to check: PhysicalDevice has Surface Rendering support")
        {
            panic!("PhysicalDevice doesn't have Surface Rendering support");
        }
        let caps = adapter.surface_capabilities(&surface).expect("Failed to get Surface capabilities");
        let formats = adapter.surface_formats(&surface).expect("Failed to get supported Surface Pixel Formats");
        let present_modes = adapter.surface_present_modes(&surface).expect("Failed to get supported Surface Presentation modes");
        
        let present_mode = present_modes.iter().find(|&&x| x == fe::PresentMode::Immediate)
            .or_else(|| present_modes.iter().find(|&&x| x == fe::PresentMode::Mailbox))
            .or_else(|| present_modes.iter().find(|&&x| x == fe::PresentMode::FIFO)).cloned().expect("Surface/PhysicalDevice must have support one of Immediate, Mailbox or FIFO present modes");
        let format = formats.iter().find(|&x| fe::FormatQuery(x.format).eq_bit_width(32).has_components(fe::FormatComponents::RGBA).has_element_of(fe::ElementType::UNORM).passed())
            .cloned().expect("Surface/PhysicalDevice must have support a format which has 32 bit width, components of RGBA and type of UNORM");
        let (width, height) = target.client_size();
        let swapchain = fe::SwapchainBuilder::new(&surface, max(2, caps.minImageCount), format, fe::Extent2D(width as _, height as _), fe::ImageUsage::COLOR_ATTACHMENT)
            .present_mode(present_mode).enable_clip().composite_alpha(fe::CompositeAlpha::PostMultiplied).create(&device).expect("Failed to create a Swapchain");
        let images = swapchain.get_images().expect("Failed to get swapchain buffers");
        let views = images.iter().map(|i| i.create_view(None, None, &fe::ComponentMapping::default(), &fe::ImageSubresourceRange
        {
            aspect_mask: fe::AspectMask::COLOR, mip_levels: 0 .. 1, array_layers: 0 .. 1
        })).collect::<Result<Vec<_>, _>>().expect("Failed to create views to each swapchain buffers");

        #[cfg(debug)]
        { Ok(VulkanRenderDevice { instance, adapter, device, graphics_queue: gq, transfer_queue: tq, surface, swapchain, rt_views: views, debug_report }) }
        #[cfg(not(debug))]
        { Ok(VulkanRenderDevice { instance, adapter, device, graphics_queue: gq, transfer_queue: tq, surface, swapchain, rt_views: views }) }
    }

    #[cfg(debug)]
    #[allow(dead_code)]
    extern "system" fn debug_call(flags: fe::vk::VkDebugReportFlagsEXT, object_type: fe::vk::VkDebugReportObjectTypeEXT,
        object: u64, location: libc::size_t, message_code: i32, layer_prefix: *const libc::c_char, message: *const libc::c_char, user_data: *mut libc::c_void) -> fe::vk::VkBool32
    {
        println!("[debug_call]{:?}", unsafe { CStr::from_ptr(message) }); fe::vk::VK_FALSE
    }
}
impl Drop for VulkanRenderDevice
{
    fn drop(&mut self) { self.device.wait().unwrap(); }
}

#[cfg(windows)]
pub struct D3D12RenderDevice
{

}
#[cfg(windows)]
impl D3D12RenderDevice
{
    fn new() -> IOResult<Self>
    {
        
    }
}
#[cfg(not(windows))] pub struct D3D12RenderDevice;
#[cfg(not(windows))] impl D3D12RenderDevice
{
    fn new() -> IOResult<Self>
    {
        Err(IOError::new(ErrorKind::Other, "Unsupported Platform for DirectX12"))
    }
}

pub enum RenderDevice
{
    Vulkan(VulkanRenderDevice), DirectX12(D3D12RenderDevice)
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
