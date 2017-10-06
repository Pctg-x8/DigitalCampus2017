
#[macro_use] extern crate appinstance;
extern crate libc;
extern crate ferrite;
extern crate ws_common;

use ws_common::{NativeWindow, WindowServer};

pub struct Application { main_window: NativeWindow }
impl Application
{
    AppInstance!(pub static instance: Application = Application::new());

    const INITIAL_SIZE: (u16, u16) = (960, 960 * 9 / 16);
    fn new() -> Self
    {
        let main_window = NativeWindow::new(Self::INITIAL_SIZE, "DigitalCampus 2017");
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
    println!("=== DIGITAL CAMPUS 2017 ===");
    Application::instance().process_events();
}
