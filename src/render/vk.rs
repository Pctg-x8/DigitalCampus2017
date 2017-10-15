
use ferrite as fe;
use Application;
use ws_common::WindowServer;
use ferrite::traits::*;
#[cfg(feature = "debug")] use libc;
use metrics::*;
use event::*;
use std::sync::Arc;
use std::mem::replace;
use std::error::Error;
use std::borrow::Cow;
use std::ops::Deref;

const APPNAME: &'static str = "dc2017";

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
}

pub struct RenderDeviceCore
{
    instance: fe::Instance, adapter: fe::PhysicalDevice, device: fe::Device,
    #[cfg(feature = "debug")] debug_report: fe::DebugReportCallback,
    graphics_queue: (u32, fe::Queue), transfer_queue: (u32, fe::Queue),

    agent_str: LazyData<String>, devprops: LazyData<fe::vk::VkPhysicalDeviceProperties>, memindices: MemoryIndices
}
impl RenderDeviceCore
{
    AppInstance!(static instance: fe::Result<RenderDeviceCore> = Self::init());
    fn get<'a>() -> &'a Self { Self::instance().as_ref().unwrap() }
    fn init() -> fe::Result<Self>
    {
        #[cfg(feature = "target_x11")] const PLATFORM_SURFACE_EXTENSION: &'static str = "VK_KHR_xcb_surface";
        #[cfg(windows)] const PLATFORM_SURFACE_EXTENSION: &'static str = "VK_KHR_win32_surface";

        let mut ibuilder = fe::InstanceBuilder::new(APPNAME, (0, 1, 0), "Kaede", (0, 1, 0));
        ibuilder.add_extensions(vec!["VK_KHR_surface", PLATFORM_SURFACE_EXTENSION]);
        #[cfg(feature = "debug")] ibuilder.add_extension("VK_EXT_debug_report").add_layer("VK_LAYER_LUNARG_standard_validation");
        let instance = ibuilder.create()?;
        #[cfg(feature = "debug")]
        let debug_report = fe::DebugReportCallback::new::<()>(&instance, fe::DebugReportFlags::ERROR.warning().performance_warning(),
            Self::debug_call, None).expect("Failed to create a debug report object");
        
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

        let memprops = adapter.memory_properties();
        let memindices = MemoryIndices
        {
            devlocal: memprops.find_device_local_index().expect("Unable to find a memory index which is device local"),
            host: memprops.find_host_visible_index().expect("Unable to find a memory index which can be visibled from the host")
        };

        #[cfg(feature = "debug")] {
            Ok(RenderDeviceCore
            {
                instance, adapter, device, debug_report, graphics_queue: gq, transfer_queue: tq, agent_str: LazyData::INIT,
                devprops: LazyData::INIT, memindices
            })
        }
        #[cfg(not(feature = "debug"))] {
            Ok(RenderDeviceCore
            {
                instance, adapter, device, graphics_queue: gq, transfer_queue: tq, agent_str: LazyData::INIT,
                devprops: LazyData::INIT, memindices
            })
        }
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
    fn drop(&mut self) { RenderDeviceCore::get().device.wait().unwrap(); }
}

pub struct MemoryIndices { devlocal: u32, host: u32 }
pub struct RenderDevice
{
    surface: fe::Surface, swapchain: fe::Swapchain, rt_views: Vec<fe::ImageView>,
    render_control: RenderControl, primary_rt_pass: fe::RenderPass, rtsc: Vec<fe::Framebuffer>,
    rtcp: fe::CommandPool, rtcmds: Vec<fe::CommandBuffer>, buffer_ready: fe::Semaphore, present_ready: fe::Semaphore
}
impl RenderDevice
{
    pub fn init() -> Result<Self, &'static fe::VkResultBox>
    {
        let ref core = RenderDeviceCore::instance().as_ref()?;

        let ref target = Application::instance().main_window;
        if !WindowServer::instance().presentation_support(&core.adapter, core.graphics_queue.0)
        {
            panic!("System doesn't have Vulkan Presentation support");
        }
        let surface = WindowServer::instance().new_render_surface(target, &core.instance).expect("Failed to create Render Surface");
        if !core.adapter.surface_support(core.graphics_queue.0, &surface).expect("Failed to check: PhysicalDevice has Surface Rendering support")
        {
            panic!("PhysicalDevice doesn't have Surface Rendering support");
        }
        let caps = core.adapter.surface_capabilities(&surface).expect("Failed to get Surface capabilities");
        let formats = core.adapter.surface_formats(&surface).expect("Failed to get supported Surface Pixel Formats");
        let present_modes = core.adapter.surface_present_modes(&surface).expect("Failed to get supported Surface Presentation modes");
        
        let present_mode = present_modes.iter().find(|&&x| x == fe::PresentMode::Immediate)
            .or_else(|| present_modes.iter().find(|&&x| x == fe::PresentMode::Mailbox))
            .or_else(|| present_modes.iter().find(|&&x| x == fe::PresentMode::FIFO)).cloned().expect("Surface/PhysicalDevice must have support one of Immediate, Mailbox or FIFO present modes");
        let format = formats.iter().find(|&x| fe::FormatQuery(x.format).eq_bit_width(32).has_components(fe::FormatComponents::RGBA).has_element_of(fe::ElementType::UNORM).passed())
            .cloned().expect("Surface/PhysicalDevice must have support a format which has 32 bit width, components of RGBA and type of UNORM");
        let fmt = format.format;
        let (width, height) = target.client_size();
        let swapchain = fe::SwapchainBuilder::new(&surface, ::std::cmp::max(2, caps.minImageCount), format,
            fe::Extent2D(width as _, height as _), fe::ImageUsage::COLOR_ATTACHMENT)
            .present_mode(present_mode).enable_clip().composite_alpha(fe::CompositeAlpha::Opaque)
            .pre_transform(fe::SurfaceTransform::Identity).create(&core.device).expect("Failed to create a Swapchain");
        let images = swapchain.get_images().expect("Failed to get swapchain buffers");
        let views = images.iter().map(|i| i.create_view(None, None, &fe::ComponentMapping::default(), &fe::ImageSubresourceRange
        {
            aspect_mask: fe::AspectMask::COLOR, mip_levels: 0 .. 1, array_layers: 0 .. 1
        })).collect::<Result<Vec<_>, _>>().expect("Failed to create views to each swapchain buffers");
        let primary_rt_pass = fe::RenderPassBuilder::new()
            .add_attachment(fe::vk::VkAttachmentDescription
            {
                loadOp: fe::vk::VK_ATTACHMENT_LOAD_OP_CLEAR, storeOp: fe::vk::VK_ATTACHMENT_STORE_OP_STORE,
                format: fmt, initialLayout: fe::ImageLayout::ColorAttachmentOpt as _, finalLayout: fe::ImageLayout::PresentSrc as _,
                samples: 1, flags: 0, stencilLoadOp: fe::vk::VK_ATTACHMENT_LOAD_OP_DONT_CARE, stencilStoreOp: fe::vk::VK_ATTACHMENT_STORE_OP_DONT_CARE
            })
            .add_subpass(fe::SubpassDescription::new().add_color_output(0, fe::ImageLayout::ColorAttachmentOpt, None))
            .create(&core.device).expect("Failed to create a render pass object for primary render targets");
        let rtsc: Vec<_> = views.iter().map(|v| fe::Framebuffer::new(&primary_rt_pass, &[v], v.size(), 1))
            .collect::<Result<_, _>>().expect("Failed to create render targets of each swapchain buffers");
        let rtcp = fe::CommandPool::new(&core.device, core.graphics_queue.0, false, false).expect("Failed to create a CommandPool");
        let rtcmds = rtcp.alloc(rtsc.len() as _, true).expect("Failed to allocate command buffers for rendering to swapchain buffers");

        #[cfg(feature = "debug")]
        Ok(RenderDevice
        {
            render_control: RenderControl::init(&core.device, &swapchain), surface, swapchain, rt_views: views, primary_rt_pass, rtsc,
            rtcp, rtcmds, buffer_ready: fe::Semaphore::new(&core.device).expect("Failed to create a semaphore(buffer_ready)"),
            present_ready: fe::Semaphore::new(&core.device).expect("Failed to create a semaphore(present_ready)")
        })
    }
}

impl RenderDevice
{
    pub fn agent(&self) -> &str
    {
        RenderDeviceCore::get().agent_str.load(||
        {
            use std::ffi::CStr;

            let adapter_properties = RenderDeviceCore::get().devprops.load(|| RenderDeviceCore::get().adapter.properties());
            format!("Vulkan {:?}", unsafe { CStr::from_ptr(adapter_properties.deviceName.as_ptr()) })
        })
    }
    pub fn minimum_uniform_alignment(&self) -> fe::vk::VkDeviceSize
    {
        RenderDeviceCore::get().devprops.load(|| RenderDeviceCore::get().adapter.properties()).limits.minUniformBufferOffsetAlignment
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
            let buffer = fe::BufferDesc::new(buffer_size as _, buffer_usage).create(&RenderDeviceCore::get().device)?;
            let sbuffer = fe::BufferDesc::new(buffer_size as _, buffer_usage).create(&RenderDeviceCore::get().device)?;
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
                .array_layers(layers).create(&RenderDeviceCore::get().device).map(|o|
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
                .use_linear_tiling().array_layers(layers).create(&RenderDeviceCore::get().device).map(|o|
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
        let memory = fe::DeviceMemory::allocate(&RenderDeviceCore::get().device, (buffer_base + buffer_size) as _, RenderDeviceCore::get().memindices.devlocal)?;
        let smemory = fe::DeviceMemory::allocate(&RenderDeviceCore::get().device, (sbuffer_base + buffer_size) as _, RenderDeviceCore::get().memindices.host)?;
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

    pub fn update_render_commands<F: FnMut(&mut super::RenderCommandsBasic, usize)>(&self, mut updater: F) -> fe::Result<()>
    {
        self.rtcp.reset(true)?;
        for (n, c) in self.rtcmds.iter().enumerate()
        {
            let mut rec = CommandRecorder { rec: c.begin()?, in_render_pass: false };
            updater(&mut rec, n);
        }
        Ok(())
    }
    pub fn get_primary_render_target(&self, index: usize) -> RenderTarget { RenderTarget::PrimaryRT(index) }

    /*pub fn new_render_target(&self, res: &fe::ImageView, optimized_clear: Option<Color>, after_usage: ResourceAfterUsage) -> fe::Result<RenderTarget>
    {
        let rp = fe::RenderPassBuilder::new()
            .add_attachment(fe::vk::VkAttachmentDescription
            {
                loadOp: if optimized_clear.is_some() { fe::vk::VK_ATTACHMENT_LOAD_OP_CLEAR } else { fe::vk::VK_ATTACHMENT_LOAD_OP_LOAD },
                storeOp: fe::vk::VK_ATTACHMENT_STORE_OP_STORE,
                format: res.format(), initialLayout: fe::ImageLayout::ColorAttachmentOpt as _, finalLayout: after_usage.translate_vk() as _,
                samples: 1, flags: 0, stencilLoadOp: fe::vk::VK_ATTACHMENT_LOAD_OP_DONT_CARE, stencilStoreOp: fe::vk::VK_ATTACHMENT_STORE_OP_DONT_CARE
            })
            .add_subpass(fe::SubpassDescription::new().add_color_output(0, fe::ImageLayout::ColorAttachmentOpt, None))
            .create(&self.device)?;
        let fb = fe::Framebuffer::new(&rp, &[res], res.size(), 1)?;
        Ok(RenderTarget(rp, fb, optimized_clear))
    }*/

    // pub fn swapchain_buffer_count(&self) -> usize { self.rtsc.len() }
    pub fn do_render(&self) -> fe::Result<bool>
    {
        if let Some(next) = self.render_control.check_ready_next()?
        {
            RenderDeviceCore::get().graphics_queue.1.submit(&[fe::SubmissionBatch
            {
                command_buffers: Cow::Borrowed(&[&self.rtcmds[next as usize]]),
                signal_semaphores: Cow::Borrowed(&[(&self.present_ready)]),
                .. Default::default()
            }], None)?;
            RenderDeviceCore::get().graphics_queue.1.present(&[(&self.swapchain, next)], &[&self.present_ready])?;
            self.render_control.begin_acquire_next();
            Ok(true)
        }
        else
        {
            Ok(false)
        }
    }
    pub fn wait_render_ready(&self) -> fe::Result<()>
    {
        self.render_control.wait_last_render_completion().map(drop)
    }

    pub fn new_render_command_buffer(&self, count: usize) -> fe::Result<RenderCommands>
    {
        let cp = fe::CommandPool::new(&RenderDeviceCore::get().device, RenderDeviceCore::get().graphics_queue.0, false, false)?;
        let commands = cp.alloc(count as _, true)?;
        Ok(RenderCommands(cp, commands))
    }
    pub fn new_render_subcommand_buffer(&self, count: usize) -> fe::Result<RenderCommands>
    {
        let cp = fe::CommandPool::new(&RenderDeviceCore::get().device, RenderDeviceCore::get().graphics_queue.0, false, false)?;
        let commands = cp.alloc(count as _, false)?;
        Ok(RenderCommands(cp, commands))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)] pub enum ResourceAfterUsage
{
    Displayed, ShaderRead, TargetedForRender, CopySource
}
#[repr(C)] #[derive(Debug, Clone, PartialEq)]
pub struct Color(pub f32, pub f32, pub f32, pub f32);
impl AsRef<[f32; 4]> for Color { fn as_ref(&self) -> &[f32; 4] { unsafe { ::std::mem::transmute(self) } } }
impl ResourceAfterUsage
{
    fn translate_vk(self) -> fe::ImageLayout
    {
        match self
        {
            ResourceAfterUsage::Displayed => fe::ImageLayout::PresentSrc,
            ResourceAfterUsage::ShaderRead => fe::ImageLayout::ShaderReadOnlyOpt,
            ResourceAfterUsage::TargetedForRender => fe::ImageLayout::ColorAttachmentOpt,
            ResourceAfterUsage::CopySource => fe::ImageLayout::TransferSrcOpt
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
            super::ColorFormat::Grayscale => fe::vk::VK_FORMAT_R8_UNORM,
            super::ColorFormat::Default => fe::vk::VK_FORMAT_R8G8B8_UNORM,
            super::ColorFormat::WithAlpha => fe::vk::VK_FORMAT_R8G8B8A8_UNORM
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

pub enum RenderTarget
{
    Owned(fe::RenderPass, fe::Framebuffer, Option<Color>),
    PrimaryRT(usize)
}
impl super::RenderTarget for RenderTarget {}
impl RenderTarget
{
    fn pass(&self) -> &fe::RenderPass
    {
        match *self
        {
            RenderTarget::Owned(ref r, _, _) => r,
            RenderTarget::PrimaryRT(_) => &super::RenderDevice::get().ensure_vk().primary_rt_pass
        }
    }
    fn fb(&self) -> &fe::Framebuffer
    {
        match *self
        {
            RenderTarget::Owned(_, ref f, _) => f,
            RenderTarget::PrimaryRT(n) => &super::RenderDevice::get().ensure_vk().rtsc[n]
        }
    }
    fn opt_clear(&self) -> Option<&Color>
    {
        match *self
        {
            RenderTarget::Owned(_, _, ref o) => o.as_ref(),
            RenderTarget::PrimaryRT(_) => Some(&Color(0.0, 0.0, 0.0, 0.5))
        }
    }
}

use std::sync::atomic::{Ordering, AtomicUsize, AtomicBool};
pub struct RenderControl
{
    th: Option<::std::thread::JoinHandle<()>>,
    next_index: Arc<AtomicUsize>, render_ready_flag: Arc<AtomicBool>,
    ev_acquire_next: Event, ev_render_ready: Event, ev_thread_exit: Event
}
impl RenderControl
{
    fn acquire_next_image_sync(sc: Option<&fe::Swapchain>, fence: &fe::Fence) -> fe::Result<u32>
    {
        let sc = sc.unwrap_or_else(|| &super::RenderDevice::get().ensure_vk().swapchain);
        let next = sc.acquire_next(None, None, Some(fence))?;
        fence.wait()?; fence.reset()?;
        Ok(next)
    }

    fn init(device: &fe::Device, swapchain: &fe::Swapchain) -> Self
    {
        let render_ready = fe::Fence::new(device, false).expect("Failed to create a fence");
        let render_ready_flag = Arc::new(AtomicBool::new(true));
        let rrf_th = render_ready_flag.clone();
        let (ev_acquire_next, ev_render_ready, ev_thread_exit) = (Event::new(), Event::new(), Event::new());
        let (ean_s, err_s, ete_s) = (ev_acquire_next.share_inner(), ev_render_ready.share_inner(), ev_thread_exit.share_inner());
        let next_index = Arc::new(AtomicUsize::new(Self::acquire_next_image_sync(Some(swapchain), &render_ready)
            .expect("Failure while acquiring initial index of buffer") as _));
        let ni_th = next_index.clone();
        #[cfg(feature = "debug")] println!("Initial Index: {}", next_index.load(Ordering::Acquire));
        RenderControl
        {
            th: Some(::std::thread::Builder::new().name("RenderControl Fence Observer".into()).spawn(move ||
            {
                let (ev_acquire_next, ev_render_ready, ev_thread_exit) = (ean_s, err_s, ete_s);
                let render_ready_flag = rrf_th;

                'mlp: loop
                {
                    #[cfg(feature = "debug")] println!("[RenderControl] waiting to begin acquire_next...");
                    loop
                    {
                        if Event::wait_any(&[&ev_acquire_next, &ev_thread_exit]) == Some(0) { ev_acquire_next.reset(); break; }
                        else { ev_thread_exit.reset(); break 'mlp; }
                    }
                    #[cfg(feature = "debug")] println!("[RenderControl] acquiring next index...");
                    let next = Self::acquire_next_image_sync(None, &render_ready).expect("Failure while acquiring next index of buffer");
                    #[cfg(feature = "debug")] println!("[RenderControl] acquired!");
                    ni_th.store(next as _, Ordering::Release);
                    render_ready_flag.store(true, Ordering::Release);
                    ev_render_ready.set();
                }
            }).expect("Failed to spawn an observer thread")), next_index,
            ev_acquire_next, ev_render_ready, ev_thread_exit, render_ready_flag
        }
    }

    pub fn check_ready_next(&self) -> fe::Result<Option<u32>>
    {
        if !self.render_ready_flag.load(Ordering::Acquire) { Ok(None) }
        else { Ok(Some(self.next_index.load(Ordering::Acquire) as _)) }
    }
    pub fn wait_last_render_completion(&self) -> fe::Result<u32>
    {
        if let Some(n) = self.check_ready_next()? { Ok(n) }
        else { self.ev_render_ready.wait(); self.wait_last_render_completion() }
    }
    pub fn begin_acquire_next(&self)
    {
        self.render_ready_flag.store(false, Ordering::Release);
        self.ev_acquire_next.set();
    }
}
impl Drop for RenderControl
{
    fn drop(&mut self)
    {
        self.ev_thread_exit.set(); replace(&mut self.th, None).unwrap().join().unwrap();
    }
}

pub struct RenderCommands(fe::CommandPool, Vec<fe::CommandBuffer>);
pub struct CommandRecorder<'d> { rec: fe::CmdRecord<'d>, in_render_pass: bool }

impl<'d> Drop for CommandRecorder<'d>
{
    fn drop(&mut self) { if self.in_render_pass { self.rec.end_render_pass(); } }
}

impl super::CommandBuffer for fe::CommandBuffer {}
impl super::RenderCommands for RenderCommands
{
    fn begin_recording<'s>(&'s self, index: usize) -> Result<Box<super::RenderCommandsBasic + 's>, Box<Error>>
    {
        Ok(box CommandRecorder { rec: self.1[index].begin()?, in_render_pass: false })
    }
}
impl<'d> super::RenderCommandsBasic for CommandRecorder<'d>
{
    fn prepare_render_targets(&mut self, target: &[&super::RenderTarget])
    {
        let targets: Vec<_> = target.iter().map(|&rt| unsafe { &*(rt as *const _ as *const RenderTarget) }.fb().resources()[0].deref().native_ptr()).collect();
        self.rec.pipeline_barrier(fe::PipelineStageFlags::ALL_COMMANDS, fe::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, false,
            &[], &[], &targets.iter().map(|&r| fe::vk::VkImageMemoryBarrier
            {
                srcAccessMask: fe::vk::VK_ACCESS_MEMORY_READ_BIT, dstAccessMask: fe::vk::VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT,
                oldLayout: fe::ImageLayout::PresentSrc as _, newLayout: fe::ImageLayout::ColorAttachmentOpt as _, image: r,
                subresourceRange: fe::vk::VkImageSubresourceRange { aspectMask: fe::AspectMask::COLOR.0, .. Default::default() },
                .. Default::default()
            }).collect::<Vec<_>>());
    }
    fn set_render_target(&mut self, target: &super::RenderTarget)
    {
        let target = unsafe { &*(target as *const _ as *const RenderTarget) };
        if self.in_render_pass { self.rec.end_render_pass(); }
        if let Some(ref c) = target.opt_clear()
        {
            self.rec.begin_render_pass(target.pass(), target.fb(), target.fb().size().clone().into(), &[fe::ClearValue::Color(c.as_ref().clone())], false);
        }
        else { self.rec.begin_render_pass(target.pass(), target.fb(), target.fb().size().clone().into(), &[], false); }
        self.in_render_pass = true;
    }
    fn execute_subcommands_into(&mut self, target: &super::RenderTarget, subcommands: &[&super::CommandBuffer])
    {
        let target = unsafe { &*(target as *const _ as *const RenderTarget) };
        if self.in_render_pass { self.rec.end_render_pass(); }
        if let Some(ref c) = target.opt_clear()
        {
            self.rec.begin_render_pass(target.pass(), target.fb(), target.fb().size().clone().into(), &[fe::ClearValue::Color(c.as_ref().clone())], false);
        }
        else { self.rec.begin_render_pass(target.pass(), target.fb(), target.fb().size().clone().into(), &[], false); }
        unsafe { self.rec
            .execute_commands(&subcommands.into_iter().map(|&sc| unsafe { &*(sc as *const _ as *const fe::CommandBuffer) }.native_ptr()).collect::<Vec<_>>())
            .end_render_pass(); }
        self.in_render_pass = false;
    }
}
