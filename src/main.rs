
#[macro_use] extern crate appinstance;
extern crate libc;
extern crate ferrite;
extern crate ws_common;
#[cfg(windows)] extern crate winapi;
extern crate flate2;
extern crate svgparser;

#[cfg(windows)] extern crate metrics;
#[cfg(windows)] extern crate comdrive;
#[cfg(windows)] extern crate widestring;
#[cfg(windows)] use comdrive::ResultCarrier;

use ws_common::{NativeWindow, WindowServer};

mod render;
use render::RenderDevice;

#[cfg(windows)] mod imaging;

use std::io::prelude::*;
use svgparser::Tokenize;
pub struct WelcomeSceneRender
{
    
}
impl WelcomeSceneRender
{
    pub fn new() -> Self
    {
        let mut fp = std::fs::File::open("assets/logo_ColoredLogo.svgz").and_then(flate2::read::GzDecoder::new)
            .expect("Failed to load the university logo");
        let mut content = String::with_capacity(fp.get_mut().metadata().unwrap().len() as _); fp.read_to_string(&mut content).unwrap();
        for t in &mut svgparser::svg::Tokenizer::from_str(&content).tokens()
        {
            match t
            {
                n => println!("? {:?}", n)
            }
        }
        /*let logo_svg = SVGLoader::load("assets/logo_ColoredLogo.svgz").expect("Failed to load the university logo");
        let path_groups = logo_svg.descendants().find(|n| n.id() == Some("枠")).unwrap().children()[0]
            .children().iter().filter(|n| n.match_name("g"));
        let mut paths = path_groups.flat_map(|g| g.children().iter().filter(|n| n.match_name("path")));
        let iter = paths.map(|p|
        {
            if let Some(d) = p.path_data() { d.iter() } else { unreachable!(); }
        });
        let logo = RenderDevice::get().realize_svg_segments(iter).expect("Failed to realize the svg");*/
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
