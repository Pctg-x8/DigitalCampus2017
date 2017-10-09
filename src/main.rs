
#[macro_use] extern crate appinstance;
extern crate libc;
extern crate ferrite;
extern crate ws_common;
#[cfg(windows)] extern crate winapi;
extern crate flate2;
extern crate svgdom;

#[cfg(windows)] extern crate metrics;
#[cfg(windows)] extern crate comdrive;
#[cfg(windows)] extern crate widestring;
#[cfg(windows)] use comdrive::ResultCarrier;

use ws_common::{NativeWindow, WindowServer};

mod render;
use render::RenderDevice;

#[cfg(windows)] mod imaging;

#[derive(Debug)]
pub enum SVGLoaderError { IO(std::io::Error), DOM(svgdom::Error) }
impl From<std::io::Error> for SVGLoaderError { fn from(e: std::io::Error) -> Self { SVGLoaderError::IO(e) } }
impl From<svgdom::Error> for SVGLoaderError { fn from(e: svgdom::Error) -> Self { SVGLoaderError::DOM(e) } }

use std::io::prelude::*;
pub struct SVGLoader {}
impl SVGLoader
{
    pub fn load<P: AsRef<std::path::Path> + ?Sized>(path: &P) -> Result<svgdom::Document, SVGLoaderError>
    {
        let fp = std::fs::File::open(path)?;
        let mut raw_data = String::new();
        if let Ok(mut gzr) = flate2::read::GzDecoder::new(fp)
        {
            gzr.read_to_string(&mut raw_data)?;
        }
        else { unimplemented!("read svg"); }
        svgdom::Document::from_str(&raw_data).map_err(From::from)
    }
}

pub struct WelcomeSceneRender
{
    
}
impl WelcomeSceneRender
{
    pub fn new() -> Self
    {
        let logo_svg = SVGLoader::load("assets/logo_ColoredLogo.svgz").expect("Failed to load the university logo");
        let path_groups = logo_svg.descendants().find(|n| *n.id() == "枠").unwrap().first_child().unwrap()
            .children().filter(|n| n.tag_id() == Some(svgdom::ElementId::G));
        for p in path_groups.flat_map(|g| g.children().filter(|n| n.tag_id() == Some(svgdom::ElementId::Path)))
        {
            println!("- {:?}", p);
        }
        WelcomeSceneRender {}
    }
}

pub struct Application { pub main_window: NativeWindow }
impl Application
{
    AppInstance!(pub static instance: Application = Application::new());
    /// Helping RLS completion
    pub fn get<'a>() -> &'a Self { Self::instance() }

    const INITIAL_SIZE: (u16, u16) = (960, 960 * 9 / 16);
    fn new() -> Self
    {
        let main_window = NativeWindow::new(Self::INITIAL_SIZE, "DigitalCampus 2017", true);
        main_window.show();
        Application { main_window }
    }
    fn process_events(&self)
    {
        WindowServer::instance().process_events();
    }
}

fn main()
{
    #[cfg(windows)] unsafe
    {
        CoInitializeEx(std::ptr::null_mut(), winapi::um::objbase::COINIT_MULTITHREADED).to_result(()).unwrap();
        extern "C" fn uninit() { unsafe { CoUninitialize(); } }
        libc::atexit(uninit);
    }
    println!("=== DIGITAL CAMPUS 2017 ===");
    println!("RenderAgent: {}", RenderDevice::get().agent());
    let scene = WelcomeSceneRender::new();
    Application::instance().process_events();
}

#[cfg(windows)]
use winapi::shared::minwindef::{DWORD, LPVOID};
#[cfg(windows)]
use winapi::shared::winerror::HRESULT;
#[cfg(windows)]
#[link(name = "ole32")] extern "system"
{
    pub fn CoInitializeEx(pvReserved: LPVOID, dwCoInit: DWORD) -> HRESULT;
    pub fn CoUninitialize();
}
