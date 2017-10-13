
mod vk;
#[cfg(windows)] mod d3d12;
use std::error::Error;

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
