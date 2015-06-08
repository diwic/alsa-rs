//use libc::c_int;
use alsa;
use std::ffi::{CStr, CString};
use super::error::*;
use std::ptr;
use super::Card;

pub struct Ctl(*mut alsa::snd_ctl_t);

impl Ctl {
    // Does not offer async mode (it's not very Rustic anyway)
    pub fn new(c: &CStr, nonblock: bool) -> Result<Ctl> {
        let mut r = ptr::null_mut();
        let flags = if nonblock { 1 } else { 0 }; // FIXME: alsa::SND_CTL_NONBLOCK does not exist in alsa-sys
        check("snd_ctl_open", unsafe { alsa::snd_ctl_open(&mut r, c.as_ptr(), flags) })
            .map(|_| Ctl(r))
    }

    pub fn from_card(c: &Card, nonblock: bool) -> Result<Ctl> {
        let s = format!("hw:{}", **c);
        Ctl::new(&CString::new(s).unwrap(), nonblock)
    }

    pub unsafe fn handle(&self) -> *mut alsa::snd_ctl_t { self.0 } 
}

impl Drop for Ctl {
    fn drop(&mut self) {  unsafe { alsa::snd_ctl_close(self.0) }; }
}
