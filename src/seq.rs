//! MIDI sequencer I/O and enumeration

use libc::{c_uint, c_int, c_short, pollfd};
use super::error::*;
use alsa;
use super::{Direction, poll};
use std::{ptr, fmt};
use std::ffi::CStr;

// Some constants that are not in alsa-sys
const SND_SEQ_OPEN_OUTPUT: i32 = 1;
const SND_SEQ_OPEN_INPUT: i32 = 2;
const SND_SEQ_OPEN_DUPLEX: i32 = SND_SEQ_OPEN_OUTPUT | SND_SEQ_OPEN_INPUT;
const SND_SEQ_NONBLOCK: i32 = 0x0001;

/// [snd_seq_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___sequencer.html) wrapper
pub struct Seq(*mut alsa::snd_seq_t);

unsafe impl Send for Seq {}

impl Drop for Seq {
    fn drop(&mut self) { unsafe { alsa::snd_seq_close(self.0) }; }
}

impl Seq {
    /// Hint: name should almost always be "default".
    pub fn open(name: &CStr, dir: Option<Direction>, nonblock: bool) -> Result<Seq> {
        let mut h = ptr::null_mut();
        let mode = if nonblock { SND_SEQ_NONBLOCK } else { 0 };
        let streams = match dir {
            None => SND_SEQ_OPEN_DUPLEX,
            Some(Direction::Playback) => SND_SEQ_OPEN_OUTPUT,
            Some(Direction::Capture) => SND_SEQ_OPEN_INPUT,
        };
        acheck!(snd_seq_open(&mut h, name.as_ptr(), streams, mode))
            .map(|_| Seq(h))
    }

    pub fn set_client_name(&self, name: &CStr) -> Result<()> {
        acheck!(snd_seq_set_client_name(self.0, name.as_ptr())).map(|_| ())
    }

    pub fn client_id(&self) -> Result<i32> {
        acheck!(snd_seq_client_id(self.0)).map(|q| q as i32)
    }

    pub fn drain_output(&self) -> Result<i32> {
        acheck!(snd_seq_drain_output(self.0)).map(|q| q as i32)
    }

    pub fn get_any_client_info(&self, cnum: i32) -> Result<ClientInfo> {
        let c = try!(ClientInfo::new());
        acheck!(snd_seq_get_any_client_info(self.0, cnum, c.0)).map(|_| c)
    }

}

fn polldir(o: Option<Direction>) -> c_short {
    match o {
        None => poll::POLLIN | poll::POLLOUT,
        Some(Direction::Playback) => poll::POLLOUT,
        Some(Direction::Capture) => poll::POLLIN,
    }.bits()
}

impl poll::PollDescriptors for (Seq, Option<Direction>) {

    fn count(&self) -> usize {
        unsafe { alsa::snd_seq_poll_descriptors_count((self.0).0, polldir(self.1)) as usize }
    }

    fn fill(&self, p: &mut [pollfd]) -> Result<usize> {
        let z = unsafe { alsa::snd_seq_poll_descriptors((self.0).0, p.as_mut_ptr(), p.len() as c_uint, polldir(self.1)) };
        from_code("snd_seq_poll_descriptors", z).map(|_| z as usize)
    }

    fn revents(&self, p: &[pollfd]) -> Result<poll::PollFlags> {
        let mut r = 0;
        let z = unsafe { alsa::snd_seq_poll_descriptors_revents((self.0).0, p.as_ptr() as *mut pollfd, p.len() as c_uint, &mut r) };
        from_code("snd_seq_poll_descriptors_revents", z).map(|_| poll::PollFlags::from_bits_truncate(r as c_short))
    }
}

/// [snd_seq_client_info_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___seq_client.html) wrapper
pub struct ClientInfo(*mut alsa::snd_seq_client_info_t);

unsafe impl Send for ClientInfo {}

impl Drop for ClientInfo {
    fn drop(&mut self) {
        unsafe { alsa::snd_seq_client_info_free(self.0) };
    }
}

impl ClientInfo {
    fn new() -> Result<Self> {
        let mut p = ptr::null_mut();
        acheck!(snd_seq_client_info_malloc(&mut p)).map(|_| ClientInfo(p))
    }

    // Not sure if it's useful for this one to be public.
    fn set_client(&self, client: i32) {
        unsafe { alsa::snd_seq_client_info_set_client(self.0, client as c_int) };
    }
    
    pub fn get_client(&self) -> i32 {
        unsafe { alsa::snd_seq_client_info_get_client(self.0) as i32 }
    }

    pub fn get_name(&self) -> Result<&str> {
        let c = unsafe { alsa::snd_seq_client_info_get_name(self.0) };
        from_const("snd_seq_client_info_get_name", c)
    }
}

impl fmt::Debug for ClientInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ClientInfo({},{:?})", self.get_client(), self.get_name())
    }
}

#[derive(Copy, Clone)]
/// Iterates over clients connected to the seq API (both kernel and userspace clients).
pub struct ClientIter<'a>(&'a Seq, i32);

impl<'a> ClientIter<'a> {
    pub fn new(seq: &'a Seq) -> Self { ClientIter(seq, -1) }
}

impl<'a> Iterator for ClientIter<'a> {
    type Item = ClientInfo;
    fn next(&mut self) -> Option<Self::Item> {
        let z = ClientInfo::new().unwrap();
        z.set_client(self.1);
        let r = unsafe { alsa::snd_seq_query_next_client((self.0).0, z.0) };
        if r < 0 { self.1 = -1; return None };
        self.1 = z.get_client();
        Some(z)
    }
}

/// [snd_seq_port_info_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___seq_port.html) wrapper
pub struct PortInfo(*mut alsa::snd_seq_port_info_t);

unsafe impl Send for PortInfo {}

impl Drop for PortInfo {
    fn drop(&mut self) {
        unsafe { alsa::snd_seq_port_info_free(self.0) };
    }
}

impl PortInfo {
    fn new() -> Result<Self> {
        let mut p = ptr::null_mut();
        acheck!(snd_seq_port_info_malloc(&mut p)).map(|_| PortInfo(p))
    }

    pub fn get_client(&self) -> i32 {
        unsafe { alsa::snd_seq_port_info_get_client(self.0) as i32 }
    }

    pub fn get_port(&self) -> i32 {
        unsafe { alsa::snd_seq_port_info_get_port(self.0) as i32 }
    }

    // Not sure if it's useful for this one to be public.
    fn set_client(&self, client: i32) {
        unsafe { alsa::snd_seq_port_info_set_client(self.0, client as c_int) };
    }

    // Not sure if it's useful for this one to be public.
    fn set_port(&self, port: i32) {
        unsafe { alsa::snd_seq_port_info_set_port(self.0, port as c_int) };
    }

    pub fn get_name(&self) -> Result<&str> {
        let c = unsafe { alsa::snd_seq_port_info_get_name(self.0) };
        from_const("snd_seq_port_info_get_name", c)
    }

}

impl fmt::Debug for PortInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PortInfo({}:{},{:?})", self.get_client(), self.get_port(), self.get_name())
    }
}

#[derive(Copy, Clone)]
/// Iterates over clients connected to the seq API (both kernel and userspace clients).
pub struct PortIter<'a>(&'a Seq, i32, i32);

impl<'a> PortIter<'a> {
    pub fn new(seq: &'a Seq, client: i32) -> Self { PortIter(seq, client, -1) }
}

impl<'a> Iterator for PortIter<'a> {
    type Item = PortInfo;
    fn next(&mut self) -> Option<Self::Item> {
        let z = PortInfo::new().unwrap();
        z.set_client(self.1);
        z.set_port(self.2);
        let r = unsafe { alsa::snd_seq_query_next_port((self.0).0, z.0) };
        if r < 0 { self.2 = -1; return None };
        self.2 = z.get_port();
        Some(z)
    }
}


#[test]
fn print_seqs() {
    use std::ffi::CString;
    let s = super::Seq::open(&CString::new("default").unwrap(), None, false).unwrap();
    s.set_client_name(&CString::new("rust_alsa_API").unwrap()).unwrap();
    let clients: Vec<_> = ClientIter::new(&s).collect();
    for a in &clients {
        let ports: Vec<_> = PortIter::new(&s, a.get_client()).collect();
    println!("{:?}: {:?}", a, ports);
    }
}
