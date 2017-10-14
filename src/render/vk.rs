
use ferrite as fe;
use Application;
use ws_common::WindowServer;
use ferrite::traits::*;
#[cfg(feature = "debug")] use libc;
use metrics::*;
use event::*;
use std::sync::Arc;
use std::mem::replace;

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

pub struct MemoryIndices { devlocal: u32, host: u32 }
pub struct RenderDevice
{
    instance: fe::Instance, adapter: fe::PhysicalDevice, device: fe::Device, graphics_queue: (u32, fe::Queue), transfer_queue: (u32, fe::Queue),
    surface: fe::Surface, swapchain: fe::Swapchain, rt_views: Vec<fe::ImageView>,
    #[cfg(feature = "debug")] debug_report: fe::DebugReportCallback,

    agent_str: LazyData<String>, devprops: LazyData<fe::vk::VkPhysicalDeviceProperties>, memindices: MemoryIndices, render_control: RenderControl
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
        let (gq_count, queues) =
            if graphics_qf != transfer_qf { (1, vec![fe::DeviceQueueCreateInfo(graphics_qf, vec![0.0]), fe::DeviceQueueCreateInfo(transfer_qf, vec![0.0])]) }
            else
            {
                let c = ::std::cmp::min(2, queue_families.queue_count(graphics_qf));
                (c, vec![fe::DeviceQueueCreateInfo(graphics_qf, vec![0.0; c as usize])])
            };
        let device =
        {
            let devbuilder = fe::DeviceBuilder::new(&adapter)
                .add_extension("VK_KHR_swapchain");
            #[cfg(feature = "debug")]
            let devbuilder = devbuilder.add_layer("VK_LAYER_LUNARG_standard_validation");
            devbuilder.add_queues(queues).create().expect("Failed to create device")
        };
        let gq = (graphics_qf, device.queue(graphics_qf, 0));
        let tq = (transfer_qf, device.queue(transfer_qf, if graphics_qf == transfer_qf { ::std::cmp::min(1, gq_count - 1) } else { 0 }));

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

        let memprops = adapter.memory_properties();
        let memindices = MemoryIndices
        {
            devlocal: memprops.find_device_local_index().expect("Unable to find a memory index which is device local"),
            host: memprops.find_host_visible_index().expect("Unable to find a memory index which can be visibled from the host")
        };

        #[cfg(feature = "debug")]
        { Ok(RenderDevice
        {
            render_control: RenderControl::init(&device),
            instance, adapter, device, graphics_queue: gq, transfer_queue: tq, surface, swapchain, rt_views: views, debug_report,
            agent_str: LazyData::INIT, devprops: LazyData::INIT, memindices
        }) }
        #[cfg(not(feature = "debug"))]
        { Ok(RenderDevice
        {
            render_control: RenderControl::init(&device),
            instance, adapter, device, graphics_queue: gq, transfer_queue: tq, surface, swapchain, rt_views: views,
            agent_str: LazyData::INIT, devprops: LazyData::INIT, memindices
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
        fn alignment(p: fe::vk::VkDeviceSize, a: fe::vk::VkDeviceSize) -> fe::vk::VkDeviceSize { (p / a + 1) * a }
        let uf_alignment = |p| alignment(p, self.minimum_uniform_alignment());
        let bdp: Vec<_> = buffer_data.into_iter().scan(0, |current_offset, &super::BufferContent { kind, bytesize }|
        {
            let offset = if kind == super::BufferKind::Constant { uf_alignment(*current_offset) } else { *current_offset };
            *current_offset = offset + bytesize as fe::vk::VkDeviceSize;
            Some(BufferDataPlacement { offset, bytesize: bytesize as _, flags: kind.translate_vk().0 })
        }).collect();
        let buffer_size = bdp.last().map(|b| b.offset + b.bytesize).unwrap_or(0);
        let (buffer, sbuffer) = if buffer_size > 0
        {
            let buffer_usage = fe::BufferUsage(bdp.iter().fold(0, |bits, b| bits | b.flags));
            let buffer = fe::BufferDesc::new(buffer_size as _, buffer_usage).create(&self.device)?;
            let sbuffer = fe::BufferDesc::new(buffer_size as _, buffer_usage).create(&self.device)?;
            (Some(buffer), Some(sbuffer))
        }
        else { (None, None) };
        let (bufalloc, sbufalloc) = (buffer.as_ref().map(MemoryBound::requirements), sbuffer.as_ref().map(MemoryBound::requirements));
        let mut texture_bytes = 0;
        let tdp: Vec<_> = texture_data.into_iter().scan(0, |current_offset, &super::TextureParam { size, layers, color, render_target, require_staging }|
        {
            let mut usage = if render_target { fe::ImageUsage::COLOR_ATTACHMENT } else { fe::ImageUsage::SAMPLED };
            if require_staging { usage = usage.transfer_dest(); }
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
        let mut stexture_bytes = 0;
        let tdps: Vec<_> = texture_data.into_iter().filter(|p| p.require_staging && !p.render_target).scan(0, |current_offset, &super::TextureParam { size, layers, color, .. }|
        {
            Some(fe::ImageDesc::new(fe::Extent2D(size.x(), size.y()), color.translate_vk(), fe::ImageUsage::TRANSFER_SRC, fe::ImageLayout::Preinitialized)
                .use_linear_tiling().array_layers(layers).create(&self.device).map(|o|
                {
                    let req = o.requirements();
                    let offset = alignment(*current_offset, req.alignment);
                    *current_offset = offset + req.size;
                    stexture_bytes = *current_offset;
                    TexturePlacement { offset, object: o }
                }))
        }).collect::<fe::Result<_>>()?;
        let buffer_base = alignment(texture_bytes, bufalloc.map(|x| x.alignment).unwrap_or(1));
        let sbuffer_base = alignment(stexture_bytes, sbufalloc.map(|x| x.alignment).unwrap_or(1));
        let memory = fe::DeviceMemory::allocate(&self.device, (buffer_base + buffer_size) as _, self.memindices.devlocal)?;
        let smemory = fe::DeviceMemory::allocate(&self.device, (sbuffer_base + buffer_size) as _, self.memindices.host)?;
        if let Some(ref b) = buffer.as_ref() { b.bind(&memory, buffer_base as _)?; }
        if let Some(ref b) = sbuffer.as_ref() { b.bind(&smemory, sbuffer_base as _)?; }
        let mut image = Vec::with_capacity(tdp.len());
        for TexturePlacement { object, offset } in tdp
        {
            object.bind(&memory, offset as _)?; image.push(object);
        }
        for &TexturePlacement { ref object, offset } in &tdps { object.bind(&smemory, offset as _)?; }

        Ok(ResourceBlock { memory, smemory, buffer, sbuffer, image, simage: tdps })
    }

    pub fn begin_render(&self, wait: bool) -> fe::Result<Option<()>>
    {
        let exec_render = if wait { self.render_control.wait_last_render_completion()?; true }
        else { self.render_control.check_last_render_completion()? };
        if !exec_render { Ok(None) }
        else
        {
            // TODO: impl here
        }
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
pub struct TexturePlacement { offset: fe::vk::VkDeviceSize, object: fe::Image }
pub struct ResourceBlock
{
    memory: fe::DeviceMemory, smemory: fe::DeviceMemory, buffer: Option<fe::Buffer>, sbuffer: Option<fe::Buffer>,
    image: Vec<fe::Image>, simage: Vec<TexturePlacement>
}
impl super::ResourceBlock for ResourceBlock {}

pub struct RenderControl
{
    th: Option<::std::thread::JoinHandle<()>>, fence: Arc<fe::Fence>,
    ev_queue_render: Event, ev_render_ready: Event, ev_thread_exit: Event
}
impl RenderControl
{
    fn init(device: &fe::Device) -> Self
    {
        let fence = Arc::new(fe::Fence::new(device, false).expect("Failed to create a fence"));
        let fence_th = fence.clone();
        let (ev_queue_render, ev_render_ready, ev_thread_exit) = (Event::new(), Event::new(), Event::new());
        let (eqr_s, err_s, ete_s) = (ev_queue_render.share_inner(), ev_render_ready.share_inner(), ev_thread_exit.share_inner());
        // let ev_queue_render = box 
        RenderControl
        {
            th: Some(::std::thread::Builder::new().name("RenderControl Fence Observer".into()).spawn(move ||
            {
                let (ev_queue_render, ev_render_ready, ev_thread_exit) = (eqr_s, err_s, ete_s);

                'mlp: loop
                {
                    loop
                    {
                        match Event::wait_any(&[&ev_queue_render, &ev_thread_exit])
                        {
                            Some(1) => break 'mlp, Some(0) => break, _ => ()
                        }
                    }
                    fence_th.wait() .expect("Failure while waiting a fence to be signaled");
                    fence_th.reset().expect("Failed to reset a fence");
                    ev_render_ready.set();
                }
            }).expect("Failed to spawn an observer thread")), fence,
            ev_queue_render, ev_render_ready, ev_thread_exit
        }
    }

    pub fn check_last_render_completion(&self) -> fe::Result<bool>
    {
        if !self.fence.status()?
        {
            self.ev_queue_render.set();
            Ok(false)
        }
        else { Ok(true) }
    }
    pub fn wait_last_render_completion(&self) -> fe::Result<()>
    {
        if !self.check_last_render_completion()? { self.ev_render_ready.wait(); }
        Ok(())
    }
}
impl Drop for RenderControl
{
    fn drop(&mut self) { self.ev_thread_exit.set(); replace(&mut self.th, None).unwrap().join().unwrap(); }
}
