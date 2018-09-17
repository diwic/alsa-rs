//! Tiny poll ffi
//!
//! A tiny wrapper around libc's poll system call.

use libc;
use super::error::*;
use std::io;
use libc::pollfd;


bitflags! {
    pub struct PollFlags: ::libc::c_short {
        const POLLIN  = ::libc::POLLIN;
        const POLLPRI = ::libc::POLLPRI;
        const POLLOUT = ::libc::POLLOUT;
        const POLLERR = ::libc::POLLERR;
        const POLLHUP = ::libc::POLLHUP;
        const POLLNVAL = ::libc::POLLNVAL;
    }
}

pub trait PollDescriptors {
    fn count(&self) -> usize;
    fn fill(&self, &mut [pollfd]) -> Result<usize>;
    fn revents(&self, &[pollfd]) -> Result<PollFlags>;

    /// Wrapper around count and fill - returns an array of pollfds
    fn get(&self) -> Result<Vec<pollfd>> {
        let mut v = vec![pollfd { fd: 0, events: 0, revents: 0 }; self.count()];
        if try!(self.fill(&mut v)) != v.len() { Err(Error::unsupported("did not fill the poll descriptors array")) }
        else { Ok(v) }
    }
}

impl PollDescriptors for pollfd {
    fn count(&self) -> usize { 1 }
    fn fill(&self, a: &mut [pollfd]) -> Result<usize> { a[0] = self.clone(); Ok(1) }
    fn revents(&self, a: &[pollfd]) -> Result<PollFlags> { Ok(PollFlags::from_bits_truncate(a[0].revents)) }
}

/// Wrapper around the libc poll call.
pub fn poll(fds: &mut[pollfd], timeout: i32) -> Result<usize> {
    let r = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::c_ulong, timeout as libc::c_int) };
    if r >= 0 { Ok(r as usize) } else {
         from_code("poll", -io::Error::last_os_error().raw_os_error().unwrap()).map(|_| unreachable!())
    }
}

/// Builds a pollfd array, polls it, and returns the poll descriptors which have non-zero revents.
pub fn poll_all<'a>(desc: &[&'a PollDescriptors], timeout: i32) -> Result<Vec<(&'a PollDescriptors, PollFlags)>> {

    let mut pollfds: Vec<pollfd> = vec!();
    let mut indices = vec!();
    for v2 in desc.iter().map(|q| q.get()) {
        let v = try!(v2);
        indices.push(pollfds.len() .. pollfds.len()+v.len());
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
