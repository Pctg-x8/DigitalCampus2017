//! platform dependent event signaling implementation

#[cfg(unix)] use std::os::unix::prelude::{RawFd, AsRawFd};
#[cfg(windows)] use std::ptr::null;

#[cfg(windows)] pub struct Event(::winapi::um::handleapi::HANDLE);
#[cfg(unix)] pub struct Event(RawFd);
#[cfg(unix)] impl AsRawFd for Event { fn as_raw_fd(&self) -> RawFd { self.0 } }

#[cfg(unix)] extern "system" { fn eventfd(initval: ::libc::c_uint, flags: ::libc::c_int) -> RawFd; }

impl Event
{
	pub fn new() -> Event
	{
		#[cfg(windows)] let h = unsafe { ::winapi::um::handleapi::CreateEventA(null(), false as _, false as _, null()) };
		#[cfg(unix)] let h = unsafe { eventfd(0, 0) };
		Event(h)
	}
	pub fn set(&self)
	{
		#[cfg(windows)] unsafe { ::winapi::um::handleapi::SetEvent(self.0); }
		#[cfg(unix)] unsafe { let inc: u64 = 1; ::libc::write(self.0, ::std::mem::transmute::<_, *const _>(&inc), 8); }
	}
	pub fn wait(&self)
	{
		#[cfg(windows)] unsafe { ::winapi::um::synchapi::WaitForSingleObject(self.0, ::winapi::um::synchapi::INFINITE); }
		#[cfg(unix)] unsafe { let mut inc: [u8; 8] = [0; 8]; ::libc::read(self.0, inc.as_ptr() as _, 8); }
	}
}

impl Drop for Event
{
	fn drop(&mut self)
	{
		#[cfg(windows)] unsafe { ::winapi::um::handleapi::CloseHandle(self.0); }
		#[cfg(unix)] unsafe { ::libc::close(self.0); }
	}
}
