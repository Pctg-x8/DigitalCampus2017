
mod vk;
#[cfg(windows)] mod d3d12;
use std::error::Error;
use metrics::*;

pub trait VectorImage {}

pub enum RenderDevice
{
    Vulkan(vk::RenderDevice), #[cfg(windows)] DirectX12(d3d12::RenderDevice)
}
impl RenderDevice
{
    AppInstance!(pub static instance: RenderDevice = RenderDevice::new());
    /// Helping RLS completion
    pub fn get<'a>() -> &'a Self { Self::instance() }

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
    pub fn create_resources(&self, buffer: &[BufferContent], textures: &[TextureParam]) -> Result<Box<ResourceBlock>, Box<Error>>
    {
        match self
        {
            &RenderDevice::Vulkan(ref vrd) => vrd.create_resources(buffer, textures).map(|x| box x as _).map_err(From::from),
            #[cfg(windows)]
            &RenderDevice::DirectX12(ref drd12) => unimplemented!()
        }
    }

    pub fn do_render<F: FnOnce()>(&self, f: F) -> Result<(), Box<Error>>
    {
        match *self
        {
            RenderDevice::Vulkan(_) => unimplemented!(),
            #[cfg(windows)]
            RenderDevice::DirectX12(ref d) =>
            {
                let findex = d.begin_render()?;
                f();
                d.end_render(findex)?; Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferContent { pub kind: BufferKind, pub bytesize: usize }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferKind { Vertex, Index, Constant }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureParam
{
    pub size: Size2U, pub layers: u32, pub color: ColorFormat, pub render_target: bool, pub require_staging: bool
}
impl Default for TextureParam
{
    fn default() -> Self
    {
        TextureParam { size: Size2U(1, 1), layers: 1, color: ColorFormat::WithAlpha, render_target: false, require_staging: false }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFormat { Grayscale, Default, WithAlpha }
pub trait ResourceBlock {}
