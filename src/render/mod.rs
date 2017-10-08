
mod vk;
#[cfg(windows)] mod d3d12;

pub enum RenderDevice
{
    Vulkan(vk::RenderDevice), DirectX12(d3d12::RenderDevice)
}
impl RenderDevice
{
    AppInstance!(pub static instance: RenderDevice = RenderDevice::new());
    /// Helping RLS completion
    pub fn get<'a>() -> &'a Self { Self::instance() }

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

    pub fn agent(&self) -> &str
    {
        match self
        {
            &RenderDevice::Vulkan(ref vrd) => vrd.agent(),
            &RenderDevice::DirectX12(ref drd12) => drd12.agent()
        }
    }
}
