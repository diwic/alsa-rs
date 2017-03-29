//! MIDI sequencer I/O and enumeration

use libc::{c_uint, c_int, c_short, c_uchar, c_void, c_long, size_t, pollfd};
use super::error::*;
use alsa;
use super::{Direction, poll};
use std::{ptr, fmt, mem, slice, time};
use std::ffi::CStr;

// Some constants that are not in alsa-sys
const SND_SEQ_OPEN_OUTPUT: i32 = 1;
const SND_SEQ_OPEN_INPUT: i32 = 2;
const SND_SEQ_OPEN_DUPLEX: i32 = SND_SEQ_OPEN_OUTPUT | SND_SEQ_OPEN_INPUT;
const SND_SEQ_NONBLOCK: i32 = 0x0001;
const SND_SEQ_ADDRESS_BROADCAST: u8 = 255;
const SND_SEQ_ADDRESS_SUBSCRIBERS: u8 = 254;
const SND_SEQ_ADDRESS_UNKNOWN: u8 = 253;
const SND_SEQ_QUEUE_DIRECT: u8 = 253;
const SND_SEQ_TIME_MODE_MASK: u8 = 1<<1;
const SND_SEQ_TIME_STAMP_MASK: u8 = 1<<0;
const SND_SEQ_TIME_MODE_REL: u8 = (1<<1);
const SND_SEQ_TIME_STAMP_REAL: u8 = (1<<0);
const SND_SEQ_TIME_STAMP_TICK: u8 = (0<<0);
const SND_SEQ_TIME_MODE_ABS: u8 = (0<<1);
const SND_SEQ_CLIENT_SYSTEM: u8 = 0;
const SND_SEQ_PORT_SYSTEM_TIMER: u8 = 0;
const SND_SEQ_PORT_SYSTEM_ANNOUNCE: u8 = 1;
const SND_SEQ_PRIORITY_HIGH: u8 = 1<<4;

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

    pub fn set_client_event_filter(&self, event_type: i32) -> Result<()> {
        acheck!(snd_seq_set_client_event_filter(self.0, event_type as c_int)).map(|_| ())
    }

    pub fn set_client_pool_output(&self, size: u32) -> Result<()> {
        acheck!(snd_seq_set_client_pool_output(self.0, size as size_t)).map(|_| ())
    }

    pub fn set_client_pool_input(&self, size: u32) -> Result<()> {
        acheck!(snd_seq_set_client_pool_input(self.0, size as size_t)).map(|_| ())
    }

    pub fn set_client_pool_output_room(&self, size: u32) -> Result<()> {
        acheck!(snd_seq_set_client_pool_output_room(self.0, size as size_t)).map(|_| ())
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

    pub fn get_any_port_info(&self, a: Addr) -> Result<PortInfo> {
        let c = try!(PortInfo::new());
        acheck!(snd_seq_get_any_port_info(self.0, a.client as c_int, a.port as c_int, c.0)).map(|_| c)
    }

    pub fn create_port(&self, port: &PortInfo) -> Result<()> {
        acheck!(snd_seq_create_port(self.0, port.0)).map(|_| ())
    }

    pub fn create_simple_port(&self, name: &CStr, caps: PortCaps, t: PortType) -> Result<i32> {
        acheck!(snd_seq_create_simple_port(self.0, name.as_ptr(), caps.bits() as c_uint, t.bits() as c_uint)).map(|q| q as i32)
    }

    pub fn set_port_info(&self, port: i32, info: &mut PortInfo) -> Result<()> {
        acheck!(snd_seq_set_port_info(self.0, port, info.0)).map(|_| ())
    }

    pub fn delete_port(&self, port: i32) -> Result<()> {
        acheck!(snd_seq_delete_port(self.0, port as c_int)).map(|_| ())
    }

    pub fn subscribe_port(&self, info: &PortSubscribe) -> Result<()> {
        acheck!(snd_seq_subscribe_port(self.0, info.0)).map(|_| ())
    }

    pub fn unsubscribe_port(&self, sender: Addr, dest: Addr) -> Result<()> {
        let z = try!(PortSubscribe::new());
        z.set_sender(sender);
        z.set_dest(dest);
        acheck!(snd_seq_unsubscribe_port(self.0, z.0)).map(|_| ())
    }

    pub fn event_output(&self, e: &mut Event) -> Result<u32> { acheck!(snd_seq_event_output(self.0, &mut e.0)).map(|q| q as u32) }
    pub fn event_output_buffer(&self, e: &mut Event) -> Result<u32> { acheck!(snd_seq_event_output_buffer(self.0, &mut e.0)).map(|q| q as u32) }
    pub fn event_output_direct(&self, e: &mut Event) -> Result<u32> { acheck!(snd_seq_event_output_direct(self.0, &mut e.0)).map(|q| q as u32) }

    pub fn event_input(&self) -> Result<Event> {
        let mut z = ptr::null_mut();
        try!(acheck!(snd_seq_event_input(self.0, &mut z)));
        unsafe { Event::extract(&mut *z, "snd_seq_event_input") }
    }
    pub fn event_input_pending(&self, fetch_sequencer: bool) -> Result<u32> {
        acheck!(snd_seq_event_input_pending(self.0, if fetch_sequencer {1} else {0})).map(|q| q as u32)
    }

    pub fn get_queue_tempo(&self, q: i32) -> Result<QueueTempo> {
        let value = try!(QueueTempo::new());
        acheck!(snd_seq_get_queue_tempo(self.0, q as c_int, value.0)).map(|_| value)
    }

    pub fn set_queue_tempo(&self, q: i32, value: &QueueTempo) -> Result<()> {
        acheck!(snd_seq_set_queue_tempo(self.0, q as c_int, value.0)).map(|_| ())
    }

    pub fn free_queue(&self, q: i32) -> Result<()> { acheck!(snd_seq_free_queue(self.0, q)).map(|_| ()) }
    pub fn alloc_queue(&self) -> Result<i32> { acheck!(snd_seq_alloc_queue(self.0)).map(|q| q as i32) }
    pub fn alloc_named_queue(&self, n: &CStr) -> Result<i32> {
        acheck!(snd_seq_alloc_named_queue(self.0, n.as_ptr())).map(|q| q as i32)
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
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Addr {
    pub client: i32,
    pub port: i32,
}

impl Addr {
    pub fn system_timer() -> Addr { Addr { client: SND_SEQ_CLIENT_SYSTEM as i32, port: SND_SEQ_PORT_SYSTEM_TIMER as i32 } }
    pub fn system_announce() -> Addr { Addr { client: SND_SEQ_CLIENT_SYSTEM as i32, port: SND_SEQ_PORT_SYSTEM_ANNOUNCE as i32 } }
    pub fn broadcast() -> Addr { Addr { client: SND_SEQ_ADDRESS_BROADCAST as i32, port: SND_SEQ_ADDRESS_BROADCAST as i32 } }
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

pub struct Event(alsa::snd_seq_event_t, EventType, Option<Vec<u8>>);

unsafe impl Send for Event {}

impl Event {
    pub fn new<D: EventData>(t: EventType, data: &D) -> Self {
        let mut z = Event(unsafe { mem::zeroed() }, EventType::None, None);
        z.1 = t;
        (z.0)._type = t as c_uchar;
        debug_assert!(D::has_data(t));
        data.set_data(&mut z);
        z
    }

    // Extracts EventType and Data into Event's own buffer. This requires the event data to
    // be valid, hence the unsafety.
    unsafe fn extract(z: &mut alsa::snd_seq_event_t, func: &'static str) -> Result<Event> {
        let t = try!(EventType::from_c_int((*z)._type as c_int, func));
        let v = if Vec::<u8>::has_data(t) {
            let zz = (*z).data.ext();
            Some(slice::from_raw_parts((*zz).ptr as *mut u8, (*zz).len as usize).to_vec())
        } else { None };
        Ok(Event(ptr::read(z), t, v))
    }


    #[inline]
    pub fn get_type(&self) -> EventType { self.1 }

    pub fn get_data<D: EventData>(&self) -> Option<D> { if D::has_data(self.1) { Some(D::get_data(self)) } else { None } }

    pub fn set_subs(&mut self) {
        self.0.dest.client = SND_SEQ_ADDRESS_SUBSCRIBERS;
        self.0.dest.port = SND_SEQ_ADDRESS_UNKNOWN;
    }

    pub fn set_source(&mut self, p: i32) { self.0.source.port = p as u8 }

    pub fn schedule_real(&mut self, queue: i32, relative: bool, rtime: time::Duration) {
        self.0.flags &= !(SND_SEQ_TIME_STAMP_MASK | SND_SEQ_TIME_MODE_MASK);
        self.0.flags |= SND_SEQ_TIME_STAMP_REAL | (if relative { SND_SEQ_TIME_MODE_REL } else { SND_SEQ_TIME_MODE_ABS });
        self.0.queue = queue as u8;
        let t = unsafe { &mut (*self.0.time.time()) };
        t.tv_sec = rtime.as_secs() as c_uint;
        t.tv_nsec = rtime.subsec_nanos() as c_uint;
    }

    pub fn schedule_tick(&mut self, queue: i32, relative: bool, ttime: u32) {
        self.0.flags &= !(SND_SEQ_TIME_STAMP_MASK | SND_SEQ_TIME_MODE_MASK);
        self.0.flags |= SND_SEQ_TIME_STAMP_TICK | (if relative { SND_SEQ_TIME_MODE_REL } else { SND_SEQ_TIME_MODE_ABS });
        self.0.queue = queue as u8;
        let t = unsafe { &mut (*self.0.time.tick()) };
        *t = ttime as c_uint;
    }

    pub fn set_direct(&mut self) { self.0.queue = SND_SEQ_QUEUE_DIRECT }

    pub fn get_relative(&self) -> bool { (self.0.flags & SND_SEQ_TIME_MODE_REL) != 0 }

    pub fn get_time(&self) -> Option<time::Duration> {
        if (self.0.flags & SND_SEQ_TIME_STAMP_REAL) != 0 {
            let mut d = alsa::snd_seq_timestamp_t { data: self.0.time.data };
            let t = unsafe { &(*d.time()) };
            Some(time::Duration::new(t.tv_sec as u64, t.tv_nsec as u32))
        } else { None } 
    }

    pub fn get_tick(&self) -> Option<u32> {
        if (self.0.flags & SND_SEQ_TIME_STAMP_REAL) == 0 {
            let mut d = alsa::snd_seq_timestamp_t { data: self.0.time.data };
            let t = unsafe { &(*d.tick()) };
            Some(*t)
        } else { None }
    }

    pub fn get_priority(&self) -> bool { (self.0.flags & SND_SEQ_PRIORITY_HIGH) != 0 }

    pub fn set_priority(&mut self, is_high_prio: bool) {
        if is_high_prio { self.0.flags |= SND_SEQ_PRIORITY_HIGH; }
        else { self.0.flags &= !SND_SEQ_PRIORITY_HIGH; }
    }
}

impl Clone for Event {
    fn clone(&self) -> Self { Event(unsafe { ptr::read(&self.0) }, self.1, self.2.clone()) }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut x = f.debug_tuple("Event");
        x.field(&self.1);
        if let Some(z) = self.get_data::<EvNote>() { x.field(&z); }
        if let Some(z) = self.get_data::<EvCtrl>() { x.field(&z); }
        if let Some(z) = self.get_data::<Addr>() { x.field(&z); }
        if let Some(z) = self.get_data::<Connect>() { x.field(&z); }
        if let Some(z) = self.get_data::<EvQueueControl<()>>() { x.field(&z); }
        if let Some(z) = self.get_data::<EvQueueControl<i32>>() { x.field(&z); }
        if let Some(z) = self.get_data::<EvQueueControl<u32>>() { x.field(&z); }
        if let Some(z) = self.get_data::<EvResult>() { x.field(&z); }
        if let Some(z) = self.get_data::<Vec<u8>>() { x.field(&z); }
        x.finish()
    }
}

/// Low level methods to set/get data on an Event. Don't use these directly, use generic methods on Event instead.
pub trait EventData {
    fn has_data(e: EventType) -> bool;
    fn set_data(&self, ev: &mut Event);
    fn get_data(ev: &Event) -> Self;
}

impl EventData for () {
    fn has_data(e: EventType) -> bool { !EvNote::has_data(e) && !EvCtrl::has_data(e) && !Addr::has_data(e) && !Vec::<u8>::has_data(e)}
    fn set_data(&self, _: &mut Event) {}
    fn get_data(_: &Event) -> Self {}
}

impl EventData for Vec<u8> {
    fn has_data(e: EventType) -> bool {
        match e {
            EventType::Sysex => true,
            EventType::Bounce => true,
            EventType::UsrVar0 => true,
            EventType::UsrVar1 => true,
            EventType::UsrVar2 => true,
            EventType::UsrVar3 => true,
            EventType::UsrVar4 => true,
             _ => false,
        }
    }
    fn set_data(&self, e: &mut Event) {
        e.2 = Some(self.clone());
        let z: &mut alsa::snd_seq_ev_ext_t = unsafe { &mut *(&mut e.0.data as *mut alsa::Union_Unnamed10 as *mut _) };
        z.len = e.2.as_ref().unwrap().len() as c_uint;
        z.ptr = e.2.as_mut().unwrap().as_mut_ptr() as *mut c_void;
    }
    fn get_data(e: &Event) -> Self { e.2.as_ref().unwrap_or(&Vec::new()).clone() }
}


#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Default)]
pub struct EvNote {
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
    pub off_velocity: u8,
    pub duration: u32,
}

impl EventData for EvNote {
    fn has_data(e: EventType) -> bool {
         match e {
             EventType::Note => true,
             EventType::Noteon => true,
             EventType::Noteoff => true,
             EventType::Keypress => true,
             _ => false,
         }
    }
    fn get_data(ev: &Event) -> Self {
         let z: &alsa::snd_seq_ev_note_t = unsafe { &*(&ev.0.data as *const alsa::Union_Unnamed10 as *const _) };
         EvNote { channel: z.channel as u8, note: z.note as u8, velocity: z.velocity as u8, off_velocity: z.off_velocity as u8, duration: z.duration as u32 }
    }
    fn set_data(&self, ev: &mut Event) {
         let z: &mut alsa::snd_seq_ev_note_t = unsafe { &mut *(&mut ev.0.data as *mut alsa::Union_Unnamed10 as *mut _) };
         z.channel = self.channel as c_uchar;
         z.note = self.note as c_uchar;
         z.velocity = self.velocity as c_uchar;
         z.off_velocity = self.off_velocity as c_uchar;
         z.duration = self.duration as c_uint;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Default)]
pub struct EvCtrl {
    pub channel: u8,
    pub param: u32,
    pub value: i32,
}

impl EventData for EvCtrl {
    fn has_data(e: EventType) -> bool {
         match e {
             EventType::Controller => true,
             EventType::Pgmchange => true,
             EventType::Chanpress => true,
             EventType::Pitchbend => true,
             EventType::Control14 => true,
             EventType::Nonregparam => true,
             EventType::Regparam => true,
             EventType::Songpos => true,
             EventType::Songsel => true,
             EventType::Qframe => true,
             EventType::Timesign => true,
             EventType::Keysign => true,
             _ => false,
         }
    }
    fn get_data(ev: &Event) -> Self {
         let z: &alsa::snd_seq_ev_ctrl_t = unsafe { &*(&ev.0.data as *const alsa::Union_Unnamed10 as *const _) };
         EvCtrl { channel: z.channel as u8, param: z.param as u32, value: z.value as i32 }
    }
    fn set_data(&self, ev: &mut Event) {
         let z: &mut alsa::snd_seq_ev_ctrl_t = unsafe { &mut *(&mut ev.0.data as *mut alsa::Union_Unnamed10 as *mut _) };
         z.channel = self.channel as c_uchar;
         z.param = self.param as c_uint;
         z.value = self.value as c_int;
    }
}

impl EventData for Addr {
    fn has_data(e: EventType) -> bool {
         match e {
             EventType::ClientStart => true,
             EventType::ClientExit => true,
             EventType::ClientChange => true,
             EventType::PortStart => true,
             EventType::PortExit => true,
             EventType::PortChange => true,
             _ => false,
         }
    }
    fn get_data(ev: &Event) -> Self {
         let z: &alsa::snd_seq_addr_t = unsafe { &*(&ev.0.data as *const alsa::Union_Unnamed10 as *const _) };
         Addr { client: z.client as i32, port: z.port as i32 }
    }
    fn set_data(&self, ev: &mut Event) {
         let z: &mut alsa::snd_seq_addr_t = unsafe { &mut *(&mut ev.0.data as *mut alsa::Union_Unnamed10 as *mut _) };
         z.client = self.client as c_uchar;
         z.port = self.port as c_uchar;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Default)]
pub struct Connect {
    pub sender: Addr,
    pub dest: Addr,
}

impl EventData for Connect {
    fn has_data(e: EventType) -> bool {
         match e {
             EventType::PortSubscribed => true,
             EventType::PortUnsubscribed => true,
             _ => false,
         }
    }
    fn get_data(ev: &Event) -> Self {
         let mut d = unsafe { ptr::read(&ev.0.data) };
         let z = unsafe { &*d.connect() };
         Connect {
             sender: Addr { client: z.sender.client as i32, port: z.sender.port as i32 },
             dest: Addr { client: z.dest.client as i32, port: z.dest.port as i32 }
         }
    }
    fn set_data(&self, ev: &mut Event) {
         let z = unsafe { &mut *ev.0.data.connect() };
         z.sender.client = self.sender.client as c_uchar;
         z.sender.port = self.sender.port as c_uchar;
         z.dest.client = self.dest.client as c_uchar;
         z.dest.port = self.dest.port as c_uchar;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Default)]
/// Note: What types of T are required for the different EvQueueControl messages is not documented in alsa-lib. Improvement patches welcome.
pub struct EvQueueControl<T> {
    queue: i32,
    value: T,
}

impl EventData for EvQueueControl<()> {
    fn has_data(e: EventType) -> bool {
         match e {
             EventType::Start => true,
             EventType::Continue => true,
             EventType::Stop => true,
             EventType::SetposTick => true,
             EventType::SetposTime => true,
             EventType::Clock => true,
             EventType::Tick => true,
             EventType::QueueSkew => true,
             EventType::SyncPos => true,
             _ => false,
         }
    }
    fn get_data(ev: &Event) -> Self {
         let mut d = unsafe { ptr::read(&ev.0.data) };
         let z = unsafe { &*d.queue() };
         EvQueueControl { queue: z.queue as i32, value: () }
    }
    fn set_data(&self, ev: &mut Event) {
         let z = unsafe { &mut *ev.0.data.queue() };
         z.queue = self.queue as c_uchar;
    }
}

impl EventData for EvQueueControl<i32> {
    fn has_data(e: EventType) -> bool {
         match e {
             EventType::Tempo => true,
             _ => false,
         }
    }
    fn get_data(ev: &Event) -> Self { unsafe {
         let mut d = ptr::read(&ev.0.data);
         let z = &mut *d.queue();
         EvQueueControl { queue: z.queue as i32, value: *z.param.value() as i32 }
    } }
    fn set_data(&self, ev: &mut Event) { unsafe {
         let z = &mut *ev.0.data.queue();
         z.queue = self.queue as c_uchar;
         *z.param.value() = self.value as c_int;
    } }
}

impl EventData for EvQueueControl<u32> {
    fn has_data(e: EventType) -> bool {
         match e {
             EventType::SyncPos => true,
             _ => false,
         }
    }
    fn get_data(ev: &Event) -> Self { unsafe {
         let mut d = ptr::read(&ev.0.data);
         let z = &mut *d.queue();
         EvQueueControl { queue: z.queue as i32, value: *z.param.position() as u32 }
    } }
    fn set_data(&self, ev: &mut Event) { unsafe {
         let z = &mut *ev.0.data.queue();
         z.queue = self.queue as c_uchar;
         *z.param.position() = self.value as c_uint;
    } }
}


#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Default)]
/// It's called EvResult instead of Result in order to not be confused with Rust's Result type.
pub struct EvResult {
    event: i32,
    result: i32,
}

impl EventData for EvResult {
    fn has_data(e: EventType) -> bool {
         match e {
             EventType::System => true,
             EventType::Result => true,
             _ => false,
         }
    }
    fn get_data(ev: &Event) -> Self {
         let mut d = unsafe { ptr::read(&ev.0.data) };
         let z = unsafe { &*d.result() };
         EvResult { event: z.event as i32, result: z.result as i32 }
    }
    fn set_data(&self, ev: &mut Event) {
         let z = unsafe { &mut *ev.0.data.result() };
         z.event = self.event as c_int;
         z.result = self.result as c_int;
    }
}



alsa_enum!(
    /// [SND_SEQ_EVENT_xxx](http://www.alsa-project.org/alsa-doc/alsa-lib/group___seq_events.html) constants

    EventType, ALL_EVENT_TYPES[59],

    Bounce = SND_SEQ_EVENT_BOUNCE,
    Chanpress = SND_SEQ_EVENT_CHANPRESS,
    ClientChange = SND_SEQ_EVENT_CLIENT_CHANGE,
    ClientExit = SND_SEQ_EVENT_CLIENT_EXIT,
    ClientStart = SND_SEQ_EVENT_CLIENT_START,
    Clock = SND_SEQ_EVENT_CLOCK,
    Continue = SND_SEQ_EVENT_CONTINUE,
    Control14 = SND_SEQ_EVENT_CONTROL14,
    Controller = SND_SEQ_EVENT_CONTROLLER,
    Echo = SND_SEQ_EVENT_ECHO,
    Keypress = SND_SEQ_EVENT_KEYPRESS, 	
    Keysign = SND_SEQ_EVENT_KEYSIGN,	
    None = SND_SEQ_EVENT_NONE,	
    Nonregparam = SND_SEQ_EVENT_NONREGPARAM,
    Note = SND_SEQ_EVENT_NOTE,
    Noteoff = SND_SEQ_EVENT_NOTEOFF,
    Noteon = SND_SEQ_EVENT_NOTEON,	
    Oss = SND_SEQ_EVENT_OSS,
    Pgmchange = SND_SEQ_EVENT_PGMCHANGE,
    Pitchbend = SND_SEQ_EVENT_PITCHBEND, 	
    PortChange = SND_SEQ_EVENT_PORT_CHANGE,
    PortExit = SND_SEQ_EVENT_PORT_EXIT,	
    PortStart = SND_SEQ_EVENT_PORT_START,
    PortSubscribed = SND_SEQ_EVENT_PORT_SUBSCRIBED,
    PortUnsubscribed = SND_SEQ_EVENT_PORT_UNSUBSCRIBED,
    Qframe = SND_SEQ_EVENT_QFRAME,
    QueueSkew = SND_SEQ_EVENT_QUEUE_SKEW,
    Regparam = SND_SEQ_EVENT_REGPARAM,	
    Reset = SND_SEQ_EVENT_RESET,	
    Result = SND_SEQ_EVENT_RESULT,
    Sensing = SND_SEQ_EVENT_SENSING,
    SetposTick = SND_SEQ_EVENT_SETPOS_TICK,
    SetposTime = SND_SEQ_EVENT_SETPOS_TIME, 	
    Songpos = SND_SEQ_EVENT_SONGPOS,
    Songsel = SND_SEQ_EVENT_SONGSEL, 	
    Start = SND_SEQ_EVENT_START,	
    Stop = SND_SEQ_EVENT_STOP,	
    SyncPos = SND_SEQ_EVENT_SYNC_POS,
    Sysex = SND_SEQ_EVENT_SYSEX,	
    System = SND_SEQ_EVENT_SYSTEM,
    Tempo = SND_SEQ_EVENT_TEMPO,	
    Tick = SND_SEQ_EVENT_TICK,	
    Timesign = SND_SEQ_EVENT_TIMESIGN,
    TuneRequest = SND_SEQ_EVENT_TUNE_REQUEST,
    Usr0 = SND_SEQ_EVENT_USR0,
    Usr1 = SND_SEQ_EVENT_USR1, 	
    Usr2 = SND_SEQ_EVENT_USR2, 	
    Usr3 = SND_SEQ_EVENT_USR3, 	
    Usr4 = SND_SEQ_EVENT_USR4, 	
    Usr5 = SND_SEQ_EVENT_USR5, 	
    Usr6 = SND_SEQ_EVENT_USR6, 	
    Usr7 = SND_SEQ_EVENT_USR7, 	
    Usr8 = SND_SEQ_EVENT_USR8, 	
    Usr9 = SND_SEQ_EVENT_USR9, 	
    UsrVar0 = SND_SEQ_EVENT_USR_VAR0,
    UsrVar1 = SND_SEQ_EVENT_USR_VAR1,
    UsrVar2 = SND_SEQ_EVENT_USR_VAR2, 	
    UsrVar3 = SND_SEQ_EVENT_USR_VAR3, 	
    UsrVar4 = SND_SEQ_EVENT_USR_VAR4,
);


pub struct QueueTempo(*mut alsa::snd_seq_queue_tempo_t);

unsafe impl Send for QueueTempo {}

impl Drop for QueueTempo {
    fn drop(&mut self) { unsafe { alsa::snd_seq_queue_tempo_free(self.0) } }
}

impl QueueTempo {
    fn new() -> Result<Self> {
        let mut q = ptr::null_mut();
        acheck!(snd_seq_queue_tempo_malloc(&mut q)).map(|_| QueueTempo(q))
    }

    pub fn empty() -> Result<Self> {
        let q = try!(QueueTempo::new());
        unsafe { ptr::write_bytes(q.0 as *mut u8, 0, alsa::snd_seq_queue_tempo_sizeof()) };
        Ok(q)
    }

    pub fn get_queue(&self) -> i32 { unsafe { alsa::snd_seq_queue_tempo_get_queue(self.0) as i32 } }
    pub fn get_tempo(&self) -> u32 { unsafe { alsa::snd_seq_queue_tempo_get_tempo(self.0) as u32 } }
    pub fn get_ppq(&self) -> i32 { unsafe { alsa::snd_seq_queue_tempo_get_ppq(self.0) as i32 } }
    pub fn get_skew(&self) -> u32 { unsafe { alsa::snd_seq_queue_tempo_get_skew(self.0) as u32 } }
    pub fn get_skew_base(&self) -> u32 { unsafe { alsa::snd_seq_queue_tempo_get_skew_base(self.0) as u32 } }

//    pub fn set_queue(&self, value: i32) { unsafe { alsa::snd_seq_queue_tempo_set_queue(self.0, value as c_int) } }
    pub fn set_tempo(&self, value: u32) { unsafe { alsa::snd_seq_queue_tempo_set_tempo(self.0, value as c_uint) } }
    pub fn set_ppq(&self, value: i32) { unsafe { alsa::snd_seq_queue_tempo_set_ppq(self.0, value as c_int) } }
    pub fn set_skew(&self, value: u32) { unsafe { alsa::snd_seq_queue_tempo_set_skew(self.0, value as c_uint) } }
    pub fn set_skew_base(&self, value: u32) { unsafe { alsa::snd_seq_queue_tempo_set_skew_base(self.0, value as c_uint) } }
}

/// [snd_midi_event_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___m_i_d_i___event.html) Wrapper
///
/// Sequencer event <-> MIDI byte stream coder
pub struct MidiEvent(*mut alsa::snd_midi_event_t);

impl Drop for MidiEvent {
    fn drop(&mut self) { unsafe { alsa::snd_midi_event_free(self.0) } }
}

impl MidiEvent {
    pub fn new(bufsize: u32) -> Result<MidiEvent> {
        let mut q = ptr::null_mut();
        acheck!(snd_midi_event_new(bufsize as size_t, &mut q)).map(|_| MidiEvent(q))
    }

    pub fn resize_buffer(&self, bufsize: u32) -> Result<()> { acheck!(snd_midi_event_resize_buffer(self.0, bufsize as size_t)).map(|_| ()) }

    /// Note: this corresponds to snd_midi_event_no_status, but on and off are switched.
    ///
    /// Alsa-lib is a bit confusing here. Anyhow, set "enable" to true to enable running status.
    pub fn enable_running_status(&self, enable: bool) { unsafe { alsa::snd_midi_event_no_status(self.0, if enable {0} else {1}) } }

    pub fn decode(&self, buf: &mut [u8], ev: &Event) -> Result<usize> {
        acheck!(snd_midi_event_decode(self.0, buf.as_mut_ptr() as *mut c_uchar, buf.len() as c_long, &ev.0)).map(|r| r as usize)
    }

    pub fn encode(&self, buf: &[u8]) -> Result<(usize, Option<Event>)> {
        let mut ev = unsafe { mem::zeroed() };
        let r = try!(acheck!(snd_midi_event_encode(self.0, buf.as_ptr() as *const c_uchar, buf.len() as c_long, &mut ev)));
        let e = if ev._type == alsa::SND_SEQ_EVENT_NONE as u8 { None }
        else { Some(try!( unsafe { Event::extract(&mut ev, "snd_midi_event_encode") } )) };
        Ok((r as usize, e))
    }

}

#[test]
fn print_seqs() {
    use std::ffi::CString;
    let s = super::Seq::open(&CString::new("default").unwrap(), None, false).unwrap();
    s.set_client_name(&CString::new("rust_test_print_seqs").unwrap()).unwrap();
    let clients: Vec<_> = ClientIter::new(&s).collect();
    for a in &clients {
        let ports: Vec<_> = PortIter::new(&s, a.get_client()).collect();
        println!("{:?}: {:?}", a, ports);
    }
}

#[test]
fn seq_subscribe() {
    use std::ffi::CString;
    let s = super::Seq::open(&CString::new("default").unwrap(), None, false).unwrap();
    s.set_client_name(&CString::new("rust_test_seq_subscribe").unwrap()).unwrap();
    let timer_info = s.get_any_port_info(Addr { client: 0, port: 0 }).unwrap();
    assert_eq!(timer_info.get_name().unwrap(), "Timer");
    let info = PortInfo::empty().unwrap();
    let _port = s.create_port(&info);
    let subs = PortSubscribe::empty().unwrap();
    subs.set_sender(Addr { client: 0, port: 0 });
    subs.set_dest(Addr { client: s.client_id().unwrap(), port: info.get_port() });
    s.subscribe_port(&subs).unwrap();
}

#[test]
fn seq_loopback() {
    use std::ffi::CString;
    let s = super::Seq::open(&CString::new("default").unwrap(), None, false).unwrap();
    s.set_client_name(&CString::new("rust_test_seq_loopback").unwrap()).unwrap();

    // Create ports
    let sinfo = PortInfo::empty().unwrap();
    sinfo.set_capability(READ | SUBS_READ);
    sinfo.set_type(MIDI_GENERIC | APPLICATION);
    s.create_port(&sinfo).unwrap();
    let sport = sinfo.get_port();
    let dinfo = PortInfo::empty().unwrap();
    dinfo.set_capability(WRITE | SUBS_WRITE);
    dinfo.set_type(MIDI_GENERIC | APPLICATION);
    s.create_port(&dinfo).unwrap();
    let dport = dinfo.get_port();

    // Connect them
    let subs = PortSubscribe::empty().unwrap();
    subs.set_sender(Addr { client: s.client_id().unwrap(), port: sport });
    subs.set_dest(Addr { client: s.client_id().unwrap(), port: dport });
    s.subscribe_port(&subs).unwrap();
    println!("Connected {:?} to {:?}", subs.get_sender(), subs.get_dest());

    // Send a note!
    let note = EvNote { channel: 0, note: 64, duration: 100, velocity: 100, off_velocity: 64 };
    let mut e = Event::new(EventType::Noteon, &note);
    e.set_subs();
    e.set_direct();
    e.set_source(sport);
    println!("Sending {:?}", e);
    s.event_output(&mut e).unwrap();
    s.drain_output().unwrap();
 
    // Recieve the note!
    let e2 = s.event_input().unwrap();
    println!("Receiving {:?}", e2);
    assert_eq!(e2.get_type(), EventType::Noteon);
    assert_eq!(e2.get_data(), Some(note));
}

