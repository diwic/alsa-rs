//! MIDI devices I/O and enumeration

use super::error::*;
use super::{poll, Direction};
use crate::alsa;
use ::alloc::ffi::CString;
use ::alloc::string::{String, ToString};
use core::ffi::CStr;
use core::ptr;
use libc::{c_short, c_uint, c_void, pollfd, size_t, timespec};

pub use super::rawmidi::Info;
pub use super::rawmidi::Iter;
pub use super::rawmidi::Status;

/// [snd_ump_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___raw_midi.html) wrapper
#[derive(Debug)]
pub struct Ump(*mut alsa::snd_ump_t);

unsafe impl Send for Ump {}

impl Drop for Ump {
    fn drop(&mut self) {
        unsafe { alsa::snd_ump_close(self.0) };
    }
}

impl Ump {
    /// Wrapper around open that takes a &str instead of a &CStr
    pub fn new(name: &str, dir: Direction, nonblock: bool) -> Result<Self> {
        Self::open(&CString::new(name).unwrap(), dir, nonblock)
    }

    pub fn open(name: &CStr, dir: Direction, nonblock: bool) -> Result<Ump> {
        let mut h = ptr::null_mut();
        let flags = if nonblock { 2 } else { 0 }; // FIXME: alsa::SND_RAWMIDI_NONBLOCK does not exist in alsa-sys
        acheck!(snd_ump_open(
            if dir == Direction::Capture {
                &mut h
            } else {
                ptr::null_mut()
            },
            if dir == Direction::Playback {
                &mut h
            } else {
                ptr::null_mut()
            },
            name.as_ptr(),
            flags
        ))
        .map(|_| Ump(h))
    }

    pub fn rawmidi_info(&self) -> Result<Info> {
        Info::new().and_then(|i| acheck!(snd_ump_rawmidi_info(self.0, i.0)).map(|_| i))
    }

    pub fn rawmidi_status(&self) -> Result<Status> {
        Status::new().and_then(|i| acheck!(snd_ump_rawmidi_status(self.0, i.0)).map(|_| i))
    }

    pub fn drop(&self) -> Result<()> {
        acheck!(snd_ump_drop(self.0)).map(|_| ())
    }

    pub fn drain(&self) -> Result<()> {
        acheck!(snd_ump_drain(self.0)).map(|_| ())
    }

    pub fn name(&self) -> Result<String> {
        let c = unsafe { alsa::snd_ump_name(self.0) };
        from_const("snd_ump_name", c).map(|s| s.to_string())
    }

    #[cfg(feature = "std")]
    pub fn read(&mut self, buf: &mut [u32]) -> std::io::Result<usize> {
        let r = unsafe {
            alsa::snd_ump_read(self.0, buf.as_mut_ptr() as *mut c_void, buf.len() as size_t)
        };
        if r < 0 {
            Err(std::io::Error::from_raw_os_error(r as i32))
        } else {
            Ok(r as usize)
        }
    }

    #[cfg(feature = "std")]
    pub fn tread(&mut self, buf: &mut [u32]) -> std::io::Result<(timespec, usize)> {
        let mut timestamp: timespec = timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };

        let r = unsafe {
            alsa::snd_ump_tread(
                self.0,
                &mut timestamp as *mut timespec,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as size_t,
            )
        };
        if r < 0 {
            Err(std::io::Error::from_raw_os_error(r as i32))
        } else {
            Ok((timestamp, r as usize))
        }
    }

    #[cfg(feature = "std")]
    pub fn write(&mut self, buf: &[u32]) -> std::io::Result<usize> {
        let r = unsafe {
            alsa::snd_ump_write(self.0, buf.as_ptr() as *const c_void, buf.len() as size_t)
        };
        if r < 0 {
            Err(std::io::Error::from_raw_os_error(r as i32))
        } else {
            Ok(r as usize)
        }
    }

    pub fn nonblock(&mut self, nonblock: i32) -> Result<()> {
        acheck!(snd_ump_nonblock(self.0, nonblock)).map(|_| ())
    }
}

impl poll::Descriptors for Ump {
    fn count(&self) -> usize {
        unsafe { alsa::snd_ump_poll_descriptors_count(self.0) as usize }
    }

    fn fill(&self, p: &mut [pollfd]) -> Result<usize> {
        let z =
            unsafe { alsa::snd_ump_poll_descriptors(self.0, p.as_mut_ptr(), p.len() as c_uint) };
        from_code("snd_ump_poll_descriptors", z).map(|_| z as usize)
    }

    fn revents(&self, p: &[pollfd]) -> Result<poll::Flags> {
        let mut r = 0;
        let z = unsafe {
            alsa::snd_ump_poll_descriptors_revents(
                self.0,
                p.as_ptr() as *mut pollfd,
                p.len() as c_uint,
                &mut r,
            )
        };
        from_code("snd_ump_poll_descriptors_revents", z)
            .map(|_| poll::Flags::from_bits_truncate(r as c_short))
    }
}
