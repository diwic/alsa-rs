//! MIDI devices I/O and enumeration

use super::error::*;
use super::{poll, Direction};
use crate::alsa;
use ::alloc::ffi::CString;
use ::alloc::string::{String, ToString};
use alsa::snd_ump_rawmidi;
use core::ffi::CStr;
use core::ptr;
use libc::{c_short, c_uint, c_void, pollfd, size_t, timespec};

pub use super::rawmidi::Info;
pub use super::rawmidi::Iter;
pub use super::rawmidi::Params;
pub use super::rawmidi::Rawmidi;
pub use super::rawmidi::Status;

/// [snd_ump_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___raw_midi.html) wrapper
#[derive(Debug)]
pub struct Ump(*mut alsa::snd_ump_t, Rawmidi);

unsafe impl Send for Ump {}

impl Drop for Ump {
    fn drop(&mut self) {
        self.1.0 = ptr::null_mut();
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
        .map(|_| Ump(h, Rawmidi(unsafe { snd_ump_rawmidi(h) })))
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

    pub fn read(&mut self, buf: &mut [u32]) -> Result<usize> {
        acheck!(snd_ump_read(
            self.0,
            buf.as_mut_ptr() as *mut c_void,
            buf.len() as size_t
        ))
        .map(|sz| sz as usize)
    }

    pub fn tread(&mut self, buf: &mut [u32]) -> Result<(timespec, usize)> {
        let mut timestamp: timespec = timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };

        acheck!(snd_ump_tread(
            self.0,
            (&mut timestamp) as *mut timespec,
            buf.as_mut_ptr() as *mut c_void,
            buf.len()
        ))
        .map(|sz| (timestamp, sz as usize))
    }

    pub fn write(&mut self, buf: &[u32]) -> Result<usize> {
        acheck!(snd_ump_write(
            self.0,
            buf.as_ptr() as *const c_void,
            buf.len() as size_t
        ))
        .map(|sz| sz as usize)
    }

    pub fn nonblock(&mut self, nonblock: i32) -> Result<()> {
        acheck!(snd_ump_nonblock(self.0, nonblock)).map(|_| ())
    }

    pub fn rawmidi(&self) -> &Rawmidi {
        &self.1
    }

    pub fn rawmidi_params_current(&self) -> Result<Params> {
        let params = Params::new()?;
        acheck!(snd_ump_rawmidi_params_current(self.0, params.0)).map(|_| params)
    }

    pub fn rawmidi_params(&mut self, params: &Params) -> Result<()> {
        acheck!(snd_ump_rawmidi_params(self.0, params.0)).map(|_| ())
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
