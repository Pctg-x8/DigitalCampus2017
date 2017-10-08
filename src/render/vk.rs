
use ferrite as fe;
use Application;
use ws_common::WindowServer;
use ferrite::traits::*;
#[cfg(feature = "debug")] use libc;

pub struct RenderDevice
{
    instance: fe::Instance, adapter: fe::PhysicalDevice, device: fe::Device, graphics_queue: (u32, fe::Queue), transfer_queue: (u32, fe::Queue),
    surface: fe::Surface, swapchain: fe::Swapchain, rt_views: Vec<fe::ImageView>,
    #[cfg(feature = "debug")] debug_report: fe::DebugReportCallback,

    agent_str: Option<String>
}
impl RenderDevice
{
    #[cfg(feature = "target_x11")] const PLATFORM_SURFACE_EXTENSION: &'static str = "VK_KHR_xcb_surface";
    #[cfg(windows)] const PLATFORM_SURFACE_EXTENSION: &'static str = "VK_KHR_win32_surface";

    pub fn init() -> fe::Result<Self>
    {
        use std::cmp::max;

        let mut ibuilder = fe::InstanceBuilder::new("dc2017", (0, 1, 0), "ferrite", (0, 1, 0));
        ibuilder.add_extensions(vec!["VK_KHR_surface", Self::PLATFORM_SURFACE_EXTENSION]);
        #[cfg(feature = "debug")] ibuilder.add_extension("VK_EXT_debug_report").add_layer("VK_LAYER_LUNARG_standard_validation");
        let instance = ibuilder.create()?;
        #[cfg(feature = "debug")]
        let debug_report = fe::DebugReportCallback::new::<()>(&instance, fe::DebugReportFlags::ERROR.warning().performance_warning(),
            Self::debug_call, None).expect("Failed to create a debug reporter object");
        let adapter = instance.enumerate_physical_devices().expect("PhysicalDevices are not found").remove(0);
        let queue_families = adapter.queue_family_properties();
        let graphics_qf = queue_families.find_matching_index(fe::QueueFlags::GRAPHICS).expect("Failed to find graphics queue family");
        let transfer_qf = queue_families.find_another_matching_index(fe::QueueFlags::TRANSFER, graphics_qf).unwrap_or(graphics_qf);
        let queues =
            if graphics_qf != transfer_qf { vec![fe::DeviceQueueCreateInfo(graphics_qf, vec![0.0]), fe::DeviceQueueCreateInfo(transfer_qf, vec![0.0])] }
            else { vec![fe::DeviceQueueCreateInfo(graphics_qf, vec![0.0; 2])] };
        let device =
        {
            let devbuilder = fe::DeviceBuilder::new(&adapter)
                .add_extension("VK_KHR_swapchain");
            #[cfg(feature = "debug")]
            let devbuilder = devbuilder.add_layer("VK_LAYER_LUNARG_standard_validation");
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
            .present_mode(present_mode).enable_clip().composite_alpha(fe::CompositeAlpha::Opaque)
            .pre_transform(fe::SurfaceTransform::Identity).create(&device).expect("Failed to create a Swapchain");
        let images = swapchain.get_images().expect("Failed to get swapchain buffers");
        let views = images.iter().map(|i| i.create_view(None, None, &fe::ComponentMapping::default(), &fe::ImageSubresourceRange
        {
            aspect_mask: fe::AspectMask::COLOR, mip_levels: 0 .. 1, array_layers: 0 .. 1
        })).collect::<Result<Vec<_>, _>>().expect("Failed to create views to each swapchain buffers");

        #[cfg(feature = "debug")]
        { Ok(RenderDevice
        {
            instance, adapter, device, graphics_queue: gq, transfer_queue: tq, surface, swapchain, rt_views: views, debug_report,
            agent_str: None
        }) }
        #[cfg(not(feature = "debug"))]
        { Ok(RenderDevice
        {
            instance, adapter, device, graphics_queue: gq, transfer_queue: tq, surface, swapchain, rt_views: views,
            agent_str: None
        }) }
    }
    #[cfg(feature = "debug")]
    #[allow(dead_code)]
    extern "system" fn debug_call(flags: fe::vk::VkDebugReportFlagsEXT, object_type: fe::vk::VkDebugReportObjectTypeEXT,
        object: u64, location: libc::size_t, message_code: i32, layer_prefix: *const libc::c_char, message: *const libc::c_char, user_data: *mut libc::c_void) -> fe::vk::VkBool32
    {
        use std::ffi::CStr;

        println!("[debug_call]{:?}", unsafe { CStr::from_ptr(message) }); fe::vk::VK_FALSE
    }
}
impl Drop for RenderDevice
{
    fn drop(&mut self) { self.device.wait().unwrap(); }
}

impl RenderDevice
{
    pub fn agent(&self) -> &str
    {
        if self.agent_str.is_none()
        {
            use std::ffi::CStr;

            let p = &self.agent_str as *const _ as *mut _;
            let adapter_properties = self.adapter.properties();
            let device_name = unsafe { CStr::from_ptr(adapter_properties.deviceName.as_ptr()).to_str().unwrap() };
            unsafe { *p = Some(format!("Vulkan {}", device_name)); }
        }
        self.agent_str.as_ref().unwrap()
    }
}
