
#![feature(box_syntax)]

#[macro_use] extern crate appinstance;
extern crate libc;
extern crate ferrite;
extern crate ws_common;
#[cfg(windows)] extern crate winapi;

extern crate metrics;
#[cfg(windows)] extern crate comdrive;
#[cfg(windows)] extern crate widestring;
#[cfg(windows)] use comdrive::ResultCarrier;
extern crate num;
extern crate mio;

use ws_common::{NativeWindow, WindowServer};

mod render;
use render::{RenderDevice, TextureParam, ColorFormat};
use metrics::*;
mod event;

#[cfg(windows)] mod imaging;
#[cfg(not(windows))] extern crate image;

use image::GenericImage;

pub struct WelcomeSceneRender
{
    
}
impl WelcomeSceneRender
{
    pub fn init() -> Self
    {
        let p_logo_encoded = image::open("assets/logo_ColoredLogo.sdf.png").expect("Failed to load the university logo");
        let (w, h) = p_logo_encoded.dimensions();
        println!("The university logo loaded: size = {}x{} estimatedSize = {} bytes", w, h, w * h);
        let res = RenderDevice::get().create_resources(&[], &[
            TextureParam { size: Size2U(w, h), color: ColorFormat::Grayscale, require_staging: true, .. Default::default() }
        ]).expect("Failed to create some resources");
        RenderDevice::get().update_render_commands(|rec, n|
        {
            rec.prepare_render_targets(&[&*RenderDevice::get().get_primary_render_target(n)]);
            rec.set_render_target(&*RenderDevice::get().get_primary_render_target(n));
        }).expect("Failed to initialize render commands");
        RenderDevice::get().do_render().expect("Failed to kick initial render");
        /*let primary_cmds = RenderDevice::get().new_render_command_buffer(RenderDevice::get().swapchain_buffer_count())
            .expect("Failed to generate render command buffers");
        for n in 0 .. RenderDevice::get().swapchain_buffer_count()
        {
            primary_cmds.begin_recording(n).expect("Failed to begin recording a command buffer")
                .set_primary_render_target(n);
        }*/
        // RenderDevice::get().do_render(|| ()).unwrap();
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
    RenderDevice::init();
    println!("RenderAgent: {}", RenderDevice::get().agent());
    let scene = WelcomeSceneRender::init();
    Application::instance().process_events();
    RenderDevice::uninit();
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
