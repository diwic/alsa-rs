//! Tiny poll ffi
//!
//! This should probably have been part of the libc crate instead,
//! but since it isn't, here's what you need to use the poll system call
//! with ALSA.

use libc;
use super::error::*;
use std::os::unix::io::{RawFd, AsRawFd};
use std::io;

bitflags! {
    pub flags PollFlags: ::libc::c_short {
        const POLLIN  = 0x001,
        const POLLPRI = 0x002,
        const POLLOUT = 0x004,
        const POLLERR = 0x008,
        const POLLHUP = 0x010,
        const POLLNVAL = 0x020,
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct PollFd {
    fd: libc::c_int,
    events: libc::c_short,
    revents: libc::c_short,
}

impl AsRawFd for PollFd {
    fn as_raw_fd(&self) -> RawFd { self.fd }
}

impl PollFd {
    pub fn new(fd: RawFd, f: PollFlags) -> PollFd { PollFd { fd: fd, events: f.bits(), revents: 0 }}
    pub fn get_events(&self) -> PollFlags { PollFlags::from_bits_truncate(self.events) }
    pub fn set_events(&mut self, f: PollFlags) { self.events = f.bits(); }
    pub fn get_revents(&self) -> PollFlags { PollFlags::from_bits_truncate(self.revents) }
}

pub trait PollDescriptors {
    fn count(&self) -> usize;
    fn fill(&self, &mut [PollFd]) -> Result<usize>;
    fn revents(&self, &[PollFd]) -> Result<PollFlags>;

    /// Wrapper around count and fill - returns an array of PollFds
    fn get(&self) -> Result<Vec<PollFd>> {
        let mut v = vec![PollFd { fd: 0, events: 0, revents: 0 }; self.count()];
        if try!(self.fill(&mut v)) != v.len() { Err(Error::new(Some("did not fill the poll descriptors array".into()), 0)) }
        else { Ok(v) }
    }
}

impl PollDescriptors for PollFd {
    fn count(&self) -> usize { 1 }
    fn fill(&self, a: &mut [PollFd]) -> Result<usize> { a[0] = self.clone(); Ok(1) }
    fn revents(&self, a: &[PollFd]) -> Result<PollFlags> { Ok(a[0].get_revents()) }
}

mod ffi {
    use libc;
    extern "C" { pub fn poll(fds: *mut super::PollFd, nfds: libc::c_ulong, timeout: libc::c_int) -> libc::c_int; }
}

/// Wrapper around the libc poll call.
pub fn poll(fds: &mut[PollFd], timeout: i32) -> Result<usize> {
    let r = unsafe { ffi::poll(fds.as_mut_ptr(), fds.len() as libc::c_ulong, timeout as libc::c_int) };
    if r >= 0 { Ok(r as usize) } else {
         from_code("poll", -io::Error::last_os_error().raw_os_error().unwrap()).map(|_| unreachable!())
    }
}

/// Builds a pollfd array, polls it, and returns the poll descriptors which have non-zero revents.
pub fn poll_all<'a>(desc: &[&'a PollDescriptors], timeout: i32) -> Result<Vec<(&'a PollDescriptors, PollFlags)>> {

    let mut pollfds: Vec<PollFd> = vec!();
    let mut indices = vec!();
    for v2 in desc.iter().map(|q| q.get()) {
        let v = try!(v2);
        indices.push((pollfds.len()..pollfds.len()+v.len()));
        pollfds.extend(v);
    };

    try!(poll(&mut pollfds, timeout));

    let mut res = vec!();
    for (i, r) in indices.into_iter().enumerate() {
        let z = try!(desc[i].revents(&pollfds[r]));
        if !z.is_empty() { res.push((desc[i], z)); }
    }
    Ok(res)
}
