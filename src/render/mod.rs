
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
        let rd = vk::RenderDevice::init();
        let e = match rd
        {
            Ok(vrd) => return RenderDevice::Vulkan(vrd), Err(e) => e
        };
        println!("!! Failed to initialize Vulkan backend({:?}). Falling back into DirectX12 backend", e);
        RenderDevice::DirectX12(d3d12::RenderDevice::init().expect("Failed to initialize RenderDevice"))
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
