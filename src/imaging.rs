//! Image Loader Backend

use comdrive::*;
use std::io::Result as IOResult;
use std::ffi::OsStr;
use widestring::WideCString;
use metrics::Size2U;
use winapi::um::wincodec::GUID_WICPixelFormat32bppPBGRA;

pub struct Bitmap { size: Size2U, source: imaging::FormatConverter }

pub struct ImageLoader(imaging::Factory);
impl ImageLoader
{
    AppInstance!(pub static instance: ImageLoader = ImageLoader::new());
    /// Helping RLS completion
    pub fn get<'a>() -> &'a Self { Self::instance() }

    fn new() -> Self
    {
        let factory = imaging::Factory::new().expect("Failed to initialize a WIC Factory");
        ImageLoader(factory)
    }

    pub fn load<P: AsRef<OsStr>>(&self, path: &P) -> IOResult<Bitmap>
    {
        let frame = self.0.new_decoder_from_file(&WideCString::from_str(path).unwrap())?.frame(0)?;
        let fconv = self.0.new_format_converter()?;
        fconv.initialize(&frame, &GUID_WICPixelFormat32bppPBGRA)?;
        Ok(Bitmap { size: fconv.size()?, source: fconv })
    }
}
