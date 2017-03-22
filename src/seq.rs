//! MIDI sequencer I/O and enumeration

use libc::{c_uint, c_int, c_short, c_uchar, pollfd};
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

    pub fn get_any_client_info(&self, client: i32) -> Result<ClientInfo> {
        let c = try!(ClientInfo::new());
        acheck!(snd_seq_get_any_client_info(self.0, client, c.0)).map(|_| c)
    }

    pub fn get_any_port_info(&self, client: i32, port: i32) -> Result<PortInfo> {
        let c = try!(PortInfo::new());
        acheck!(snd_seq_get_any_port_info(self.0, client, port, c.0)).map(|_| c)
    }

    pub fn create_port(&self, port: &mut PortInfo) -> Result<()> {
        acheck!(snd_seq_create_port(self.0, port.0)).map(|_| ())
    }

    pub fn set_port_info(&self, port: i32, info: &mut PortInfo) -> Result<()> {
        acheck!(snd_seq_set_port_info(self.0, port, info.0)).map(|_| ())
    }

    pub fn delete_port(&self, port: i32) -> Result<()> {
        acheck!(snd_seq_delete_port(self.0, port as c_int)).map(|_| ())
    }

    pub fn subscribe_port(&self, info: &mut PortSubscribe) -> Result<()> {
        acheck!(snd_seq_subscribe_port(self.0, info.0)).map(|_| ())
    }

    pub fn unsubscribe_port(&self, sender: Addr, dest: Addr) -> Result<()> {
        let z = try!(PortSubscribe::new());
        z.set_sender(sender);
        z.set_dest(dest);
        acheck!(snd_seq_unsubscribe_port(self.0, z.0)).map(|_| ())
    }

}

fn polldir(o: Option<Direction>) -> c_short {
    match o {
        None => poll::POLLIN | poll::POLLOUT,
        Some(Direction::Playback) => poll::POLLOUT,
        Some(Direction::Capture) => poll::POLLIN,
    }.bits()
}

impl<'a> poll::PollDescriptors for (&'a Seq, Option<Direction>) {

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

    pub fn empty() -> Result<Self> {
        let z = try!(Self::new());
        unsafe { ptr::write_bytes(z.0 as *mut u8, 0, alsa::snd_seq_port_info_sizeof()) };
        Ok(z)
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

    pub fn set_name(&mut self, name: &CStr) {
        // Note: get_name returns an interior reference, so this one must take &mut self
        unsafe { alsa::snd_seq_port_info_set_name(self.0, name.as_ptr()) };
    }

    pub fn get_capability(&self) -> PortCaps {
        PortCaps::from_bits_truncate(unsafe { alsa::snd_seq_port_info_get_capability(self.0) as u32 })
    }

    pub fn get_type(&self) -> PortType {
        PortType::from_bits_truncate(unsafe { alsa::snd_seq_port_info_get_type(self.0) as u32 })
    }

    pub fn set_capability(&self, c: PortCaps) {
        unsafe { alsa::snd_seq_port_info_set_capability(self.0, c.bits() as c_uint) }
    }

    pub fn set_type(&self, c: PortType) {
        unsafe { alsa::snd_seq_port_info_set_type(self.0, c.bits() as c_uint) }
    }

    pub fn get_midi_channels(&self) -> i32 { unsafe { alsa::snd_seq_port_info_get_midi_channels(self.0) as i32 } }
    pub fn get_midi_voices(&self) -> i32 { unsafe { alsa::snd_seq_port_info_get_midi_voices(self.0) as i32 } }
    pub fn get_synth_voices(&self) -> i32 { unsafe { alsa::snd_seq_port_info_get_synth_voices(self.0) as i32 } }
    pub fn get_read_use(&self) -> i32 { unsafe { alsa::snd_seq_port_info_get_read_use(self.0) as i32 } }
    pub fn get_write_use(&self) -> i32 { unsafe { alsa::snd_seq_port_info_get_write_use(self.0) as i32 } }
    pub fn get_port_specified(&self) -> bool { unsafe { alsa::snd_seq_port_info_get_port_specified(self.0) == 1 } }
    pub fn get_timestamping(&self) -> bool { unsafe { alsa::snd_seq_port_info_get_timestamping(self.0) == 1 } }
    pub fn get_timestamp_real(&self) -> bool { unsafe { alsa::snd_seq_port_info_get_timestamp_real(self.0) == 1 } }
    pub fn get_timestamp_queue(&self) -> i32 { unsafe { alsa::snd_seq_port_info_get_timestamp_queue(self.0) as i32 } }

    pub fn set_midi_channels(&self, value: i32) { unsafe { alsa::snd_seq_port_info_set_midi_channels(self.0, value as c_int) } }
    pub fn set_midi_voices(&self, value: i32) { unsafe { alsa::snd_seq_port_info_set_midi_voices(self.0, value as c_int) } }
    pub fn set_synth_voices(&self, value: i32) { unsafe { alsa::snd_seq_port_info_set_synth_voices(self.0, value as c_int) } }
    pub fn set_port_specified(&self, value: bool) { unsafe { alsa::snd_seq_port_info_set_port_specified(self.0, if value { 1 } else { 0 } ) } }
    pub fn set_timestamping(&self, value: bool) { unsafe { alsa::snd_seq_port_info_set_timestamping(self.0, if value { 1 } else { 0 } ) } }
    pub fn set_timestamp_real(&self, value: bool) { unsafe { alsa::snd_seq_port_info_set_timestamp_real(self.0, if value { 1 } else { 0 } ) } }
    pub fn set_timestamp_queue(&self, value: i32) { unsafe { alsa::snd_seq_port_info_set_timestamp_queue(self.0, value as c_int) } }
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

bitflags! {
    pub flags PortCaps: u32 {
        const READ = 1<<0,
        const WRITE = 1<<1,
        const SYNC_READ = 1<<2,
        const SYNC_WRITE = 1<<3,
        const DUPLEX = 1<<4,
        const SUBS_READ = 1<<5,
        const SUBS_WRITE = 1<<6,
        const NO_EXPORT = 1<<7,
   }
}

bitflags! {
    pub flags PortType: u32 {
        const SPECIFIC = (1<<0),
        const MIDI_GENERIC = (1<<1),
        const MIDI_GM = (1<<2),
        const MIDI_GS = (1<<3),
        const MIDI_XG = (1<<4),
        const MIDI_MT32 = (1<<5),
        const MIDI_GM2 = (1<<6),
        const SYNTH = (1<<10),
        const DIRECT_SAMPLE = (1<<11),
        const SAMPLE = (1<<12),
        const HARDWARE = (1<<16),
        const SOFTWARE = (1<<17),
        const SYNTHESIZER = (1<<18),
        const PORT = (1<<19),
        const APPLICATION = (1<<20),
    }
}


/// [snd_seq_addr_t](http://www.alsa-project.org/alsa-doc/alsa-lib/structsnd__seq__addr__t.html) wrapper
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Addr {
   pub client: i32,
   pub port: i32,
}

/// [snd_seq_port_subscribe_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___seq_subscribe.html) wrapper
pub struct PortSubscribe(*mut alsa::snd_seq_port_subscribe_t);

unsafe impl Send for PortSubscribe {}

impl Drop for PortSubscribe {
    fn drop(&mut self) { unsafe { alsa::snd_seq_port_subscribe_free(self.0) }; }
}

impl PortSubscribe {
    fn new() -> Result<Self> {
        let mut p = ptr::null_mut();
        acheck!(snd_seq_port_subscribe_malloc(&mut p)).map(|_| PortSubscribe(p))
    }

    pub fn empty() -> Result<Self> {
        let z = try!(Self::new());
        unsafe { ptr::write_bytes(z.0 as *mut u8, 0, alsa::snd_seq_port_subscribe_sizeof()) };
        Ok(z)
    }

    pub fn get_sender(&self) -> Addr { unsafe {
        let z = alsa::snd_seq_port_subscribe_get_sender(self.0);
        Addr { client: (*z).client as i32, port: (*z).port as i32 }
    } }

    pub fn get_dest(&self) -> Addr { unsafe {
        let z = alsa::snd_seq_port_subscribe_get_dest(self.0);
        Addr { client: (*z).client as i32, port: (*z).port as i32 }
    } }

    pub fn get_queue(&self) -> i32 { unsafe { alsa::snd_seq_port_subscribe_get_queue(self.0) as i32 } }
    pub fn get_exclusive(&self) -> bool { unsafe { alsa::snd_seq_port_subscribe_get_exclusive(self.0) == 1 } }
    pub fn get_time_update(&self) -> bool { unsafe { alsa::snd_seq_port_subscribe_get_time_update(self.0) == 1 } }
    pub fn get_time_real(&self) -> bool { unsafe { alsa::snd_seq_port_subscribe_get_time_real(self.0) == 1 } }

    pub fn set_sender(&self, value: Addr) {
        let z = alsa::snd_seq_addr_t { client: value.client as c_uchar, port: value.port as c_uchar };
        unsafe { alsa::snd_seq_port_subscribe_set_sender(self.0, &z) };
    }

    pub fn set_dest(&self, value: Addr) {
        let z = alsa::snd_seq_addr_t { client: value.client as c_uchar, port: value.port as c_uchar };
        unsafe { alsa::snd_seq_port_subscribe_set_dest(self.0, &z) };
    }

    pub fn set_queue(&self, value: i32) { unsafe { alsa::snd_seq_port_subscribe_set_queue(self.0, value as c_int) } }
    pub fn set_exclusive(&self, value: bool) { unsafe { alsa::snd_seq_port_subscribe_set_exclusive(self.0, if value { 1 } else { 0 } ) } }
    pub fn set_time_update(&self, value: bool) { unsafe { alsa::snd_seq_port_subscribe_set_time_update(self.0, if value { 1 } else { 0 } ) } }
    pub fn set_time_real(&self, value: bool) { unsafe { alsa::snd_seq_port_subscribe_set_time_real(self.0, if value { 1 } else { 0 } ) } }

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
