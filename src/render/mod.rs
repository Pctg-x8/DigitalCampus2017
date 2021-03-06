
mod vk;
#[cfg(windows)] mod d3d12;
use std::error::Error;
use metrics::*;

pub trait VectorImage {}

pub enum RenderDevice
{
    Vulkan(vk::RenderDevice), #[cfg(windows)] DirectX12(d3d12::RenderDevice)
}
static mut RD: *const RenderDevice = 0 as *const _;
impl RenderDevice
{
    /// Helping RLS completion
    pub fn get<'a>() -> &'a Self { unsafe { &*RD } }

    pub fn init()
    {
        unsafe { RD = Box::into_raw(box Self::new()); }
    }
    pub fn uninit() { unsafe { drop(Box::from_raw(RD as *mut Self)); RD = 0 as *const _; } }

    #[cfg(windows)]
    fn new() -> Self
    {
        let rd = d3d12::RenderDevice::init();
        let e = match rd
        {
            Ok(vrd) => return RenderDevice::DirectX12(vrd), Err(e) => e
        };
        println!("!! Failed to initialize DirectX12 backend({:?}). Falling back into Vulkan backend", e);
        RenderDevice::Vulkan(vk::RenderDevice::init().expect("Failed to initialize RenderDevice"))
    }
    #[cfg(not(windows))]
    fn new() -> Self
    {
        RenderDevice::Vulkan(vk::RenderDevice::init().expect("Failed to initialize RenderDevice"))
    }

    pub fn agent(&self) -> &str
    {
        match self
        {
            &RenderDevice::Vulkan(ref vrd) => vrd.agent(),
            #[cfg(windows)]
            &RenderDevice::DirectX12(ref drd12) => drd12.agent()
        }
    }
    /*pub fn swapchain_buffer_count(&self) -> usize
    {
        match self
        {
            &RenderDevice::Vulkan(ref vrd) => vrd.swapchain_buffer_count(),
            #[cfg(windows)]
            &RenderDevice::DirectX12(_) => unimplemented!()
        }
    }*/
    pub fn create_resources(&self, buffer: &[BufferContent], textures: &[TextureParam]) -> Result<Box<ResourceBlock>, Box<Error>>
    {
        match self
        {
            &RenderDevice::Vulkan(ref vrd) => vrd.create_resources(buffer, textures).map(|x| box x as _).map_err(From::from),
            #[cfg(windows)]
            &RenderDevice::DirectX12(ref drd12) => unimplemented!()
        }
    }
    pub fn new_render_command_buffer(&self, count: usize) -> Result<Box<RenderCommands>, Box<Error>>
    {
        match self
        {
            &RenderDevice::Vulkan(ref vrd) => vrd.new_render_command_buffer(count).map(|x| box x as _).map_err(From::from),
            #[cfg(windows)]
            &RenderDevice::DirectX12(_) => unimplemented!()
        }
    }
    pub fn new_render_subcommand_buffer(&self, count: usize) -> Result<Box<RenderCommands>, Box<Error>>
    {
        match self
        {
            &RenderDevice::Vulkan(ref vrd) => vrd.new_render_subcommand_buffer(count).map(|x| box x as _).map_err(From::from),
            #[cfg(windows)]
            &RenderDevice::DirectX12(_) => unimplemented!()
        }
    }
    pub fn update_render_commands<'d, F: FnMut(&mut RenderCommandsBasic, usize)>(&self, updater: F) -> Result<(), Box<Error>>
    {
        match self
        {
            &RenderDevice::Vulkan(ref v) => v.update_render_commands(updater).map_err(From::from),
            #[cfg(windows)]
            &RenderDevice::DirectX12(_) => unimplemented!()
        }
    }
    pub fn get_primary_render_target<'d>(&'d self, index: usize) -> Box<RenderTarget + 'd>
    {
        match self
        {
            &RenderDevice::Vulkan(ref v) => box v.get_primary_render_target(index) as _,
            #[cfg(windows)]
            &RenderDevice::DirectX12(_) => unimplemented!()
        }
    }

    pub fn do_render(&self) -> Result<bool, Box<Error>>
    {
        match *self
        {
            RenderDevice::Vulkan(ref d) => d.do_render().map_err(From::from),
            #[cfg(windows)]
            RenderDevice::DirectX12(ref d) => unimplemented!("need to change")
        }
    }
    pub fn wait_render_ready(&self) -> Result<(), Box<Error>>
    {
        match *self
        {
            RenderDevice::Vulkan(ref d) => d.wait_render_ready().map_err(From::from),
            #[cfg(windows)]
            RenderDevice::DirectX12(ref d) => unimplemented!()
        }
    }
    pub fn get_builtin_vertex_array<'d>(&'d self, key: BuiltinResourceKey) -> Result<Box<VertexArray + 'd>, Box<Error>>
    {
        match *self
        {
            RenderDevice::Vulkan(ref d) => d.get_builtin_vertex_array(key).map(|x| box x as _).map_err(From::from),
            #[cfg(windows)]
            RenderDevice::DirectX12(_) => unimplemented!()
        }
    }

    pub(self) fn ensure_vk(&self) -> &vk::RenderDevice
    {
        match self { &RenderDevice::Vulkan(ref v) => v, _ => panic!("unexpected") }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinResourceKey
{
    UnitRect
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferContent { pub kind: BufferKind, pub bytesize: usize }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferKind { Vertex, Index, Constant }
#[derive(Debug, PartialEq, Eq)]
pub enum TextureUsage<'p>
{
    Immutable(&'p [u8]), FrequentlyUpdated, RenderTarget
}
impl<'p> TextureUsage<'p>
{
    pub(self) fn is_immutable(&self) -> bool { match self { &TextureUsage::Immutable(_) => true, _ => false } }
    pub(self) fn initial_pixels(&self) -> Option<&'p [u8]> { match self { &TextureUsage::Immutable(p) => Some(p), _ => None } }
}
#[derive(Debug, PartialEq, Eq)]
pub struct TextureParam<'p>
{
    pub size: Size2U, pub layers: u32, pub color: ColorFormat, pub usage: TextureUsage<'p>
}
impl<'p> Default for TextureParam<'p>
{
    fn default() -> Self
    {
        TextureParam { size: Size2U(1, 1), layers: 1, color: ColorFormat::WithAlpha, usage: TextureUsage::FrequentlyUpdated }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFormat { Grayscale, Default, WithAlpha }
pub trait ResourceBlock {}

pub trait RenderCommands
{
    fn begin_recording<'d>(&'d self, index: usize) -> Result<Box<RenderCommandsBasic + 'd>, Box<Error>>;
}
pub trait RenderCommandsBasic
{
    fn prepare_render_targets(&mut self, targets: &[&RenderTarget]);
    fn set_render_target(&mut self, target: &RenderTarget);
    fn execute_subcommands_into(&mut self, target: &RenderTarget, subcommands: &[&CommandBuffer]);
    fn draw(&mut self, vertices: &VertexArray, instance_count: usize);
}
pub trait RenderTarget {}
pub trait CommandBuffer {}
pub trait VertexArray {}
