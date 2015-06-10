//! Audio playback and capture

use libc::{c_int, c_uint, c_void, ssize_t};
use alsa;
use std::ffi::CStr;
use std::{io, fmt, ptr, mem};
use super::error::*;
use super::Direction;

/// [snd_pcm_sframes_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m.html)
pub type Frames = alsa::snd_pcm_sframes_t;

/// [snd_pcm_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m.html) wrapper - start here for audio playback and recording
pub struct PCM(*mut alsa::snd_pcm_t);

impl PCM {
    // Does not offer async mode (it's not very Rustic anyway)
    pub fn open(name: &CStr, dir: Direction, nonblock: bool) -> Result<PCM> {
        let mut r = ptr::null_mut();
        let stream = match dir {
            Direction::Capture => alsa::SND_PCM_STREAM_CAPTURE,
            Direction::Playback => alsa::SND_PCM_STREAM_PLAYBACK
        };
        let flags = if nonblock { alsa::SND_PCM_NONBLOCK } else { 0 };
        check("snd_pcm_open", unsafe { alsa::snd_pcm_open(&mut r, name.as_ptr(), stream, flags) })
            .map(|_| PCM(r))
    }

    pub fn start(&self) -> Result<()> { check("snd_pcm_start", unsafe { alsa::snd_pcm_start(self.0) }).map(|_| ()) }
    pub fn drop(&self) -> Result<()> { check("snd_pcm_drop", unsafe { alsa::snd_pcm_drop(self.0) }).map(|_| ()) }
    pub fn pause(&self, pause: bool) -> Result<()> {
        check("snd_pcm_pause", unsafe { alsa::snd_pcm_pause(self.0, if pause { 1 } else { 0 }) }).map(|_| ()) }
    pub fn resume(&self) -> Result<()> { check("snd_pcm_resume", unsafe { alsa::snd_pcm_resume(self.0) }).map(|_| ()) }
    pub fn drain(&self) -> Result<()> { check("snd_pcm_drain", unsafe { alsa::snd_pcm_drain(self.0) }).map(|_| ()) }
    pub fn prepare(&self) -> Result<()> { check("snd_pcm_prepare", unsafe { alsa::snd_pcm_prepare(self.0) }).map(|_| ()) }
    pub fn reset(&self) -> Result<()> { check("snd_pcm_reset", unsafe { alsa::snd_pcm_reset(self.0) }).map(|_| ()) }

    pub fn wait(&self, timeout_ms: Option<u32>) -> Result<bool> {
        check("snd_pcm_wait", unsafe { alsa::snd_pcm_wait(self.0, timeout_ms.map(|x| x as c_int).unwrap_or(-1)) }).map(|i| i == 1) }

    pub fn state(&self) -> State { unsafe { mem::transmute(alsa::snd_pcm_state(self.0) as u8) } }

    pub fn bytes_to_frames(&self, i: isize) -> Frames { unsafe { alsa::snd_pcm_bytes_to_frames(self.0, i as ssize_t) }}
    pub fn frames_to_bytes(&self, i: Frames) -> isize { unsafe { alsa::snd_pcm_frames_to_bytes(self.0, i) as isize }}

    pub fn avail_update(&self) -> Result<Frames> {
        let r = unsafe { alsa::snd_pcm_avail_update(self.0) };
        check("snd_pcm_avail_update", r as c_int).map(|_| r)
    }

    pub fn avail(&self) -> Result<Frames> {
        let r = unsafe { alsa::snd_pcm_avail(self.0) };
        check("snd_pcm_avail", r as c_int).map(|_| r)
    }

    pub fn avail_delay(&self) -> Result<(Frames, Frames)> {
        let (mut a, mut d) = (0, 0);
        check("snd_pcm_avail_delay", unsafe { alsa::snd_pcm_avail_delay(self.0, &mut a, &mut d) }).map(|_| (a, d))
    }

    pub fn io<'a>(&'a self) -> IO<'a> { IO(&self) }

    pub fn hw_params(&self, h: &HwParams) -> Result<()> {
        check("snd_pcm_hw_params", unsafe { alsa::snd_pcm_hw_params(self.0, h.0) }).map(|_| ())
    }

    pub fn hw_params_current<'a>(&'a self) -> Result<HwParams<'a>> {
        HwParams::new(&self).and_then(|h|
            check("snd_pcm_hw_params_current", unsafe { alsa::snd_pcm_hw_params_current(self.0, h.0) }).map(|_| h))
    }

    pub fn sw_params(&self, h: &SwParams) -> Result<()> {
        check("snd_pcm_sw_params", unsafe { alsa::snd_pcm_sw_params(self.0, h.0) }).map(|_| ())
    }

    pub fn sw_params_current<'a>(&'a self) -> Result<SwParams<'a>> {
        SwParams::new(&self).and_then(|h|
            check("snd_pcm_sw_params_current", unsafe { alsa::snd_pcm_sw_params_current(self.0, h.0) }).map(|_| h))
    }
}

impl Drop for PCM {
    fn drop(&mut self) { unsafe { alsa::snd_pcm_close(self.0) }; }
}

/// Implements `std::io::Read` and `std::io::Write` for `PCM`
pub struct IO<'a>(&'a PCM);

impl<'a> io::Read for IO<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let size = self.0.bytes_to_frames(buf.len() as isize) as alsa::snd_pcm_uframes_t; // TODO: Do we need to check for overflow here?
        let r = unsafe { alsa::snd_pcm_readi((self.0).0, buf.as_mut_ptr() as *mut c_void, size) };
        if r < 0 { Err(io::Error::from_raw_os_error(r as i32)) }
        else { Ok(self.0.frames_to_bytes(r) as usize) }
    }
}

impl<'a> io::Write for IO<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let size = self.0.bytes_to_frames(buf.len() as isize) as alsa::snd_pcm_uframes_t; // TODO: Do we need to check for overflow here?
        let r = unsafe { alsa::snd_pcm_writei((self.0).0, buf.as_ptr() as *const c_void, size) };
        if r < 0 { Err(io::Error::from_raw_os_error(r as i32)) }
        else { Ok(self.0.frames_to_bytes(r) as usize) }
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

/// [SND_PCM_STATE_xxx](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m.html) constants
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    Open = alsa::SND_PCM_STATE_OPEN as isize,
    Setup = alsa::SND_PCM_STATE_SETUP as isize,
    Prepared = alsa::SND_PCM_STATE_PREPARED as isize,
    Running = alsa::SND_PCM_STATE_RUNNING as isize,
    XRun = alsa::SND_PCM_STATE_XRUN as isize,
    Draining = alsa::SND_PCM_STATE_DRAINING as isize,
    Paused = alsa::SND_PCM_STATE_PAUSED as isize,
    Suspended = alsa::SND_PCM_STATE_SUSPENDED as isize,
    Disconnected = alsa::SND_PCM_STATE_DISCONNECTED as isize,
}

/// [SND_PCM_FORMAT_xxx](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m.html) constants
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Format {
    Unknown = alsa::SND_PCM_FORMAT_UNKNOWN as isize,
    // S16 = alsa::SND_PCM_FORMAT_S16 as isize,
    S16LE = alsa::SND_PCM_FORMAT_S16_LE as isize,
    FloatLE = alsa::SND_PCM_FORMAT_FLOAT_LE as isize,
    // TODO: More formats...
}

/// [SND_PCM_ACCESS_xxx](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m.html) constants
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Access {
    MMapInterleaved = alsa::SND_PCM_ACCESS_MMAP_INTERLEAVED as isize,
    MMapNonInterleaved = alsa::SND_PCM_ACCESS_MMAP_NONINTERLEAVED as isize,
    MMapComplex = alsa::SND_PCM_ACCESS_MMAP_COMPLEX as isize,
    RWInterleaved = alsa::SND_PCM_ACCESS_RW_INTERLEAVED as isize,
    RWNonInterleaved = alsa::SND_PCM_ACCESS_RW_NONINTERLEAVED as isize,
}

/// [snd_pcm_hw_params_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m___h_w___params.html) wrapper
pub struct HwParams<'a>(*mut alsa::snd_pcm_hw_params_t, &'a PCM);

impl<'a> Drop for HwParams<'a> {
    fn drop(&mut self) { unsafe { alsa::snd_pcm_hw_params_free(self.0) }; }
}

impl<'a> HwParams<'a> {
    fn new(a: &'a PCM) -> Result<HwParams<'a>> {
        let mut p = ptr::null_mut();
        check("snd_pcm_hw_params_malloc", unsafe { alsa::snd_pcm_hw_params_malloc(&mut p) }).map(|_| HwParams(p, a))
    }

    pub fn any(a: &'a PCM) -> Result<HwParams<'a>> { HwParams::new(a).and_then(|p|
        check("snd_pcm_hw_params_any", unsafe { alsa::snd_pcm_hw_params_any(a.0, p.0) }).map(|_| p)
    )}

    pub fn set_channels(&self, v: u32) -> Result<()> { check("snd_pcm_hw_params_set_channels",
        unsafe { alsa::snd_pcm_hw_params_set_channels((self.1).0, self.0, v as c_uint) }).map(|_| ())
    }

    pub fn get_channels(&self) -> Result<u32> {
        let mut v = 0;
        check("snd_pcm_hw_params_get_channels",
            unsafe { alsa::snd_pcm_hw_params_get_channels(self.0, &mut v) }).map(|_| v as u32)
    }

    pub fn set_rate(&self, v: u32, dir: i32) -> Result<()> { check("snd_pcm_hw_params_set_rate",
        unsafe { alsa::snd_pcm_hw_params_set_rate((self.1).0, self.0, v as c_uint, dir as c_int) }).map(|_| ())
    }

    pub fn get_rate(&self) -> Result<u32> {
        let (mut v, mut d) = (0,0);
        check("snd_pcm_hw_params_get_rate",
            unsafe { alsa::snd_pcm_hw_params_get_rate(self.0, &mut v, &mut d) }).map(|_| v as u32)
    }

    pub fn set_format(&self, v: Format) -> Result<()> { check("snd_pcm_hw_params_set_format",
        unsafe { alsa::snd_pcm_hw_params_set_format((self.1).0, self.0, v as c_int) }).map(|_| ())
    }

    pub fn get_format(&self) -> Result<Format> {
        let mut v = 0;
        check("snd_pcm_hw_params_get_format",
            unsafe { alsa::snd_pcm_hw_params_get_format(self.0, &mut v) } ).map(|_| unsafe { mem::transmute(v as u8) })
    }

    pub fn set_access(&self, v: Access) -> Result<()> { check("snd_pcm_hw_params_set_access",
        unsafe { alsa::snd_pcm_hw_params_set_access((self.1).0, self.0, v as c_uint) }).map(|_| ())
    }

    pub fn get_access(&self) -> Result<Access> {
        let mut v = 0;
        check("snd_pcm_hw_params_get_access",
            unsafe { alsa::snd_pcm_hw_params_get_access(self.0, &mut v) } ).map(|_| unsafe { mem::transmute(v as u8) })
    }

    pub fn set_period_size(&self, v: Frames, dir: i32) -> Result<()> { check("snd_pcm_hw_params_set_period_size",
        unsafe { alsa::snd_pcm_hw_params_set_period_size((self.1).0, self.0, v as alsa::snd_pcm_uframes_t, dir as c_int) }).map(|_| ())
    }

    pub fn get_period_size(&self) -> Result<Frames> {
        let (mut v, mut d) = (0,0);
        check("snd_pcm_hw_params_get_period_size",
            unsafe { alsa::snd_pcm_hw_params_get_period_size(self.0, &mut v, &mut d) }).map(|_| v as Frames)
    }

    pub fn set_periods(&self, v: u32, dir: i32) -> Result<()> { check("snd_pcm_hw_params_set_periods",
        unsafe { alsa::snd_pcm_hw_params_set_periods((self.1).0, self.0, v as c_uint, dir as c_int) }).map(|_| ())
    }

    pub fn get_periods(&self) -> Result<u32> {
        let (mut v, mut d) = (0,0);
        check("snd_pcm_hw_params_get_periods",
            unsafe { alsa::snd_pcm_hw_params_get_periods(self.0, &mut v, &mut d) }).map(|_| v as u32)
    }

    pub fn set_buffer_size(&self, v: Frames) -> Result<()> { check("snd_pcm_hw_params_set_buffer_size",
        unsafe { alsa::snd_pcm_hw_params_set_buffer_size((self.1).0, self.0, v as alsa::snd_pcm_uframes_t) }).map(|_| ())
    }

    pub fn get_buffer_size(&self) -> Result<Frames> {
        let mut v = 0;
        check("snd_pcm_hw_params_get_buffer_size",
            unsafe { alsa::snd_pcm_hw_params_get_buffer_size(self.0, &mut v) }).map(|_| v as Frames)
    }
}

impl<'a> fmt::Debug for HwParams<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
           "HwParams(channels: {:?}, rate: {:?} Hz, format: {:?}, access: {:?}, period size: {:?} frames, buffer size: {:?} frames)",
           self.get_channels(), self.get_rate(), self.get_format(), self.get_access(), self.get_period_size(), self.get_buffer_size())
    }
}

/// [snd_pcm_sw_params_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m___s_w___params.html) wrapper
pub struct SwParams<'a>(*mut alsa::snd_pcm_sw_params_t, &'a PCM);

impl<'a> Drop for SwParams<'a> {
    fn drop(&mut self) { unsafe { alsa::snd_pcm_sw_params_free(self.0) }; }
}

impl<'a> SwParams<'a> {

    fn new(a: &'a PCM) -> Result<SwParams<'a>> {
        let mut p = ptr::null_mut();
        check("snd_pcm_sw_params_malloc", unsafe { alsa::snd_pcm_sw_params_malloc(&mut p) }).map(|_| SwParams(p, a))
    }

    pub fn set_avail_min(&self, v: Frames) -> Result<()> { check("snd_pcm_sw_params_set_avail_min",
        unsafe { alsa::snd_pcm_sw_params_set_avail_min((self.1).0, self.0, v as alsa::snd_pcm_uframes_t) }).map(|_| ())
    }

    pub fn get_avail_min(&self) -> Result<Frames> {
        let mut v = 0;
        check("snd_pcm_sw_params_get_avail_min",
            unsafe { alsa::snd_pcm_sw_params_get_avail_min(self.0, &mut v) }).map(|_| v as Frames)
    }

    pub fn set_start_threshold(&self, v: Frames) -> Result<()> { check("snd_pcm_sw_params_set_start_threshold",
        unsafe { alsa::snd_pcm_sw_params_set_start_threshold((self.1).0, self.0, v as alsa::snd_pcm_uframes_t) }).map(|_| ())
    }

    pub fn get_start_threshold(&self) -> Result<Frames> {
        let mut v = 0;
        check("snd_pcm_sw_params_get_start_threshold",
            unsafe { alsa::snd_pcm_sw_params_get_start_threshold(self.0, &mut v) }).map(|_| v as Frames)
    }

    pub fn set_stop_threshold(&self, v: Frames) -> Result<()> { check("snd_pcm_sw_params_set_stop_threshold",
        unsafe { alsa::snd_pcm_sw_params_set_stop_threshold((self.1).0, self.0, v as alsa::snd_pcm_uframes_t) }).map(|_| ())
    }

    pub fn get_stop_threshold(&self) -> Result<Frames> {
        let mut v = 0;
        check("snd_pcm_sw_params_get_stop_threshold",
            unsafe { alsa::snd_pcm_sw_params_get_stop_threshold(self.0, &mut v) }).map(|_| v as Frames)
    }
}

impl<'a> fmt::Debug for SwParams<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
           "SwParams(avail_min: {:?} frames, start_threshold: {:?} frames, stop_threshold: {:?} frames)",
           self.get_avail_min(), self.get_start_threshold(), self.get_stop_threshold())
    }
}

#[test]
fn record_from_default() {
    use std::ffi::CString;
    use std::io::Read;
    let pcm = PCM::open(&*CString::new("default").unwrap(), Direction::Capture, false).unwrap();
    let hwp = HwParams::any(&pcm).unwrap();
    hwp.set_channels(2).unwrap();
    hwp.set_rate(44100, 0).unwrap();
    hwp.set_format(Format::S16LE).unwrap();
    hwp.set_access(Access::RWInterleaved).unwrap();
    pcm.hw_params(&hwp).unwrap();
    pcm.start().unwrap();
    let mut buf = [0u8; 1024]; 
    assert_eq!(pcm.io().read(&mut buf).unwrap(), 1024);
}

#[test]
fn playback_to_default() {
    use std::ffi::CString;
    use std::io::Write;
    let pcm = PCM::open(&*CString::new("default").unwrap(), Direction::Playback, false).unwrap();
    let hwp = HwParams::any(&pcm).unwrap();
    hwp.set_channels(1).unwrap();
    hwp.set_rate(44100, 0).unwrap();
    hwp.set_format(Format::S16LE).unwrap();
    hwp.set_access(Access::RWInterleaved).unwrap();
    pcm.hw_params(&hwp).unwrap();
    println!("PCM status: {:?}, {:?}", pcm.state(), pcm.hw_params_current().unwrap());
    let mut buf = [0i16; 1024];
    for (i, a) in buf.iter_mut().enumerate() {
        *a = ((i as f32 * 2.0 * ::std::f32::consts::PI / 128.0).sin() * 8192.0) as i16
    }
    let b: &[u8] = unsafe { ::std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len() * 2) };
    for _ in 0..2*44100/1024 { // 2 seconds of playback
        assert_eq!(pcm.io().write(b).unwrap(), 2048);
    }
    if pcm.state() != State::Running { pcm.start().unwrap() };
    pcm.drain().unwrap();
}
