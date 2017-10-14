//! platform dependent event signaling implementation

#[cfg(unix)] use std::os::unix::prelude::{RawFd, AsRawFd};
#[cfg(windows)] use std::ptr::null;

#[cfg(windows)] pub struct Event(bool, ::winapi::um::handleapi::HANDLE);
#[cfg(unix)] pub struct Event(bool, RawFd);
#[cfg(unix)] impl AsRawFd for Event { fn as_raw_fd(&self) -> RawFd { self.1 } }

#[cfg(unix)] extern "system" { fn eventfd(initval: ::libc::c_uint, flags: ::libc::c_int) -> RawFd; }

impl Event
{
	pub fn new() -> Event
	{
		#[cfg(windows)] let h = unsafe { ::winapi::um::handleapi::CreateEventA(null(), false as _, false as _, null()) };
		#[cfg(unix)] let h = unsafe { eventfd(0, 0) };
		Event(true, h)
	}
	pub fn share_inner(&self) -> Self { Event(false, self.1) }
	pub fn set(&self)
	{
		#[cfg(windows)] unsafe { ::winapi::um::handleapi::SetEvent(self.1); }
		#[cfg(unix)] unsafe { let inc: u64 = 1; ::libc::write(self.1, ::std::mem::transmute::<_, *const _>(&inc), 8); }
	}
	pub fn wait(&self)
	{
		#[cfg(windows)] unsafe { ::winapi::um::synchapi::WaitForSingleObject(self.1, ::winapi::um::synchapi::INFINITE); }
		#[cfg(unix)] unsafe { let mut inc: [u8; 8] = [0; 8]; ::libc::read(self.1, inc.as_ptr() as _, 8); }
	}

	#[cfg(unix)]
	pub fn wait_any(events: &[&Self]) -> Option<usize>
	{
		let poll = ::mio::Poll::new().expect("Failed to create a polling object");
		for (n, e) in events.iter().enumerate()
		{
			poll.register(&::mio::unix::EventedFd(&e.1), ::mio::Token(n), ::mio::Ready::readable(), ::mio::PollOpt::level())
				.expect("Failed to register an event for polling");
		}
		let mut events = ::mio::Events::with_capacity(1);
		poll.poll(&mut events, None).expect("Failed to wait an event");
		events.get(0).map(|e| e.token().0)
	}
}

impl Drop for Event
{
	fn drop(&mut self)
	{
		if self.0
		{
			#[cfg(windows)] unsafe { ::winapi::um::handleapi::CloseHandle(self.1); }
			#[cfg(unix)] unsafe { ::libc::close(self.1); }
		}
	}
}

unsafe impl Sync for Event {}
unsafe impl Send for Event {}
