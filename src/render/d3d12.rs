//! wip

use std::io::{Result as IOResult, Error as IOError};

pub struct RenderDevice
{

}
impl RenderDevice
{
    pub fn init() -> IOResult<Self>
    {
        Ok(RenderDevice {})   
    }
    pub fn agent(&self) -> &str { "Direct3D12" }
}
