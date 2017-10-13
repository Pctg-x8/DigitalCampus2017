
use ferrite as fe;
use Application;
use ws_common::WindowServer;
use ferrite::traits::*;
#[cfg(feature = "debug")] use libc;
use metrics::*;

pub struct LazyData<T>(Option<T>);
impl<T> LazyData<T>
{
    pub const INIT: Self = LazyData(None);

    pub fn load<F: FnOnce() -> T>(&self, loader: F) -> &T
    {
        if self.0.is_none()
        {
            let p = &self.0 as *const _ as *mut _;
            unsafe { *p = loader(); }
        }
        self.0.as_ref().unwrap()
    }
    pub fn load_mut<F: FnOnce() -> T>(&mut self, loader: F) -> &mut T
    {
        if self.0.is_none() { self.0 = Some(loader()); }
        self.0.as_mut().unwrap()
    }
}

pub struct RenderDevice
{
    instance: fe::Instance, adapter: fe::PhysicalDevice, device: fe::Device, graphics_queue: (u32, fe::Queue), transfer_queue: (u32, fe::Queue),
    surface: fe::Surface, swapchain: fe::Swapchain, rt_views: Vec<fe::ImageView>,
    #[cfg(feature = "debug")] debug_report: fe::DebugReportCallback,

    agent_str: LazyData<String>, devprops: LazyData<fe::vk::VkPhysicalDeviceProperties>
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
            agent_str: LazyData::INIT, devprops: LazyData::INIT
        }) }
        #[cfg(not(feature = "debug"))]
        { Ok(RenderDevice
        {
            instance, adapter, device, graphics_queue: gq, transfer_queue: tq, surface, swapchain, rt_views: views,
            agent_str: LazyData::INIT, devprops: LazyData::INIT
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
        self.agent_str.load(||
        {
            use std::ffi::CStr;

            let adapter_properties = self.devprops.load(|| self.adapter.properties());
            format!("Vulkan {:?}", unsafe { CStr::from_ptr(adapter_properties.deviceName.as_ptr()) })
        })
    }
    pub fn minimum_uniform_alignment(&self) -> fe::vk::VkDeviceSize
    {
        self.devprops.load(|| self.adapter.properties()).limits.minUniformBufferOffsetAlignment
    }

    pub fn create_resources(&self, buffer_data: &[super::BufferContent], texture_data: &[super::TextureParam]) -> fe::Result<ResourceBlock>
    {
        #[derive(Debug)]
        struct BufferDataPlacement { offset: fe::vk::VkDeviceSize, bytesize: fe::vk::VkDeviceSize, flags: fe::vk::VkBufferUsageFlags }
        struct TexturePlacement { offset: fe::vk::VkDeviceSize, object: fe::Image }
        fn alignment(p: fe::vk::VkDeviceSize, a: fe::vk::VkDeviceSize) -> fe::vk::VkDeviceSize { (p / a + 1) * a }
        let uf_alignment = |p| alignment(p, self.minimum_uniform_alignment());
        let bdp: Vec<_> = buffer_data.into_iter().scan(0, |current_offset, &super::BufferContent { kind, bytesize }|
        {
            let offset = if kind == super::BufferKind::Constant { uf_alignment(*current_offset) } else { *current_offset };
            *current_offset = offset + bytesize as fe::vk::VkDeviceSize;
            Some(BufferDataPlacement { offset, bytesize: bytesize as _, flags: kind.translate_vk().0 })
        }).collect();
        let buffer_size = bdp.last().map(|b| b.offset + b.bytesize).unwrap_or(0);
        let buffer = fe::BufferDesc::new(buffer_size as _, fe::BufferUsage(bdp.iter().fold(0, |bits, b| bits | b.flags))).create(&self.device)?;
        let mut texture_bytes = 0;
        let tdp: Vec<_> = texture_data.into_iter().scan(0, |current_offset, &super::TextureParam { size, layers, color, render_target }|
        {
            let usage = if render_target { fe::ImageUsage::COLOR_ATTACHMENT } else { fe::ImageUsage::SAMPLED.transfer_dest() };
            Some(fe::ImageDesc::new(fe::Extent2D(size.x(), size.y()), color.translate_vk(), usage, fe::ImageLayout::Preinitialized)
                .array_layers(layers).create(&self.device).map(|o|
                {
                    let req = o.requirements();
                    let offset = alignment(*current_offset, req.alignment);
                    *current_offset = offset + req.size;
                    texture_bytes = *current_offset;
                    TexturePlacement { offset, object: o }
                }))
        }).collect::<fe::Result<_>>()?;
        // let memory = fe::DeviceMemory::allocate(&self.device, texture_bytes + buffer_size)

        println!("BufferPlacement: {:?}", bdp);
        unimplemented!();
    }
}

impl super::BufferKind
{
    fn translate_vk(self) -> fe::BufferUsage
    {
        match self
        {
            super::BufferKind::Vertex => fe::BufferUsage::VERTEX_BUFFER,
            super::BufferKind::Index => fe::BufferUsage::INDEX_BUFFER,
            super::BufferKind::Constant => fe::BufferUsage::UNIFORM_BUFFER
        }
    }
}
impl super::ColorFormat
{
    fn translate_vk(self) -> fe::vk::VkFormat
    {
        match self
        {
            super::ColorFormat::Grayscale => fe::vk::VK_FORMAT_R8_UNORM as _,
            super::ColorFormat::Default => fe::vk::VK_FORMAT_R8G8B8_UNORM as _,
            super::ColorFormat::WithAlpha => fe::vk::VK_FORMAT_R8G8B8A8_UNORM as _
        }
    }
}
pub struct ResourceBlock
{
    memory: fe::DeviceMemory, buffer: fe::Buffer, image: Vec<fe::Image>
}
impl super::ResourceBlock for ResourceBlock {}
