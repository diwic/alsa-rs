//! Audio playback and capture
//!
//! # Example
//! Playback a sine wave through the "default" device.
//!
//! ```
//! use std::ffi::CString;
//! use alsa::Direction;
//! use alsa::pcm::{PCM, HwParams, Format, Access, State};
//!
//! // Open default playback device
//! let pcm = PCM::open(&*CString::new("default").unwrap(), Direction::Playback, false).unwrap();
//!
//! // Set hardware parameters: 44100 Hz / Mono / 16 bit
//! let hwp = HwParams::any(&pcm).unwrap();
//! hwp.set_channels(1).unwrap();
//! hwp.set_rate(44100, 0).unwrap();
//! hwp.set_format(Format::s16()).unwrap();
//! hwp.set_access(Access::RWInterleaved).unwrap();
//! pcm.hw_params(&hwp).unwrap();
//! let io = pcm.io_i16().unwrap();
//!
//! // Make a sine wave
//! let mut buf = [0i16; 1024];
//! for (i, a) in buf.iter_mut().enumerate() {
//!     *a = ((i as f32 * 2.0 * ::std::f32::consts::PI / 128.0).sin() * 8192.0) as i16
//! }
//!
//! // Play it back for 2 seconds.
//! for _ in 0..2*44100/1024 {
//!     assert_eq!(io.writei(&buf[..]).unwrap(), 1024);
//! }
//!
//! // In case the buffer was larger than 2 seconds, start the stream manually.
//! if pcm.state() != State::Running { pcm.start().unwrap() };
//! // Wait for the stream to finish playback.
//! pcm.drain().unwrap();
//! ```


use libc::{c_int, c_uint, c_void, ssize_t};
use alsa;
use std::marker::PhantomData;
use std::mem::size_of;
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
        acheck!(snd_pcm_open(&mut r, name.as_ptr(), stream, flags)).map(|_| PCM(r))
    }

    pub fn start(&self) -> Result<()> { acheck!(snd_pcm_start(self.0)).map(|_| ()) }
    pub fn drop(&self) -> Result<()> { acheck!(snd_pcm_drop(self.0)).map(|_| ()) }
    pub fn pause(&self, pause: bool) -> Result<()> {
        acheck!(snd_pcm_pause(self.0, if pause { 1 } else { 0 })).map(|_| ()) }
    pub fn resume(&self) -> Result<()> { acheck!(snd_pcm_resume(self.0)).map(|_| ()) }
    pub fn drain(&self) -> Result<()> { acheck!(snd_pcm_drain(self.0)).map(|_| ()) }
    pub fn prepare(&self) -> Result<()> { acheck!(snd_pcm_prepare(self.0)).map(|_| ()) }
    pub fn reset(&self) -> Result<()> { acheck!(snd_pcm_reset(self.0)).map(|_| ()) }

    pub fn wait(&self, timeout_ms: Option<u32>) -> Result<bool> {
        acheck!(snd_pcm_wait(self.0, timeout_ms.map(|x| x as c_int).unwrap_or(-1))).map(|i| i == 1) }

    pub fn state(&self) -> State { unsafe { mem::transmute(alsa::snd_pcm_state(self.0) as u8) } }

    pub fn bytes_to_frames(&self, i: isize) -> Frames { unsafe { alsa::snd_pcm_bytes_to_frames(self.0, i as ssize_t) }}
    pub fn frames_to_bytes(&self, i: Frames) -> isize { unsafe { alsa::snd_pcm_frames_to_bytes(self.0, i) as isize }}

    pub fn avail_update(&self) -> Result<Frames> { acheck!(snd_pcm_avail_update(self.0)) }
    pub fn avail(&self) -> Result<Frames> { acheck!(snd_pcm_avail(self.0)) }

    pub fn avail_delay(&self) -> Result<(Frames, Frames)> {
        let (mut a, mut d) = (0, 0);
        acheck!(snd_pcm_avail_delay(self.0, &mut a, &mut d)).map(|_| (a, d))
    }

    fn verify_format(&self, f: Format) -> Result<()> {
        let ff = try!(self.hw_params_current().and_then(|h| h.get_format()));
        if ff == f { Ok(()) }
        else { 
            let s = format!("Invalid sample format ({:?}, expected {:?})", ff, f);
            Err(Error::new(Some(s.into()), INVALID_FORMAT))
        }
    }

    pub fn io_i8<'a>(&'a self) -> Result<IO<'a, i8>> { self.verify_format(Format::S8).map(|_| IO(&self, PhantomData)) }
    pub fn io_u8<'a>(&'a self) -> Result<IO<'a, u8>> { self.verify_format(Format::U8).map(|_| IO(&self, PhantomData)) }
    pub fn io_i16<'a>(&'a self) -> Result<IO<'a, i16>> { self.verify_format(Format::s16()).map(|_| IO(&self, PhantomData)) }
    pub fn io_u16<'a>(&'a self) -> Result<IO<'a, u16>> { self.verify_format(Format::u16()).map(|_| IO(&self, PhantomData)) }
    pub fn io_i32<'a>(&'a self) -> Result<IO<'a, i32>> { self.verify_format(Format::s32()).map(|_| IO(&self, PhantomData)) }
    pub fn io_u32<'a>(&'a self) -> Result<IO<'a, u32>> { self.verify_format(Format::u32()).map(|_| IO(&self, PhantomData)) }
    pub fn io_f32<'a>(&'a self) -> Result<IO<'a, f32>> { self.verify_format(Format::float()).map(|_| IO(&self, PhantomData)) }
    pub fn io_f64<'a>(&'a self) -> Result<IO<'a, f64>> { self.verify_format(Format::float64()).map(|_| IO(&self, PhantomData)) }

    pub fn io<'a>(&'a self) -> IO<'a, u8> { IO(&self, PhantomData) }

    pub fn hw_params(&self, h: &HwParams) -> Result<()> {
        // FIXME: how do we ensure no IO are in scope when this happens?
        acheck!(snd_pcm_hw_params(self.0, h.0)).map(|_| ())
    }

    pub fn hw_params_current<'a>(&'a self) -> Result<HwParams<'a>> {
        HwParams::new(&self).and_then(|h|
            acheck!(snd_pcm_hw_params_current(self.0, h.0)).map(|_| h))
    }

    pub fn sw_params(&self, h: &SwParams) -> Result<()> {
        acheck!(snd_pcm_sw_params(self.0, h.0)).map(|_| ())
    }

    pub fn sw_params_current<'a>(&'a self) -> Result<SwParams<'a>> {
        SwParams::new(&self).and_then(|h|
            acheck!(snd_pcm_sw_params_current(self.0, h.0)).map(|_| h))
    }
}

impl Drop for PCM {
    fn drop(&mut self) { unsafe { alsa::snd_pcm_close(self.0) }; }
}


/// Sample format dependent struct for reading from and writing data to a `PCM`.
/// Also implements `std::io::Read` and `std::io::Write`.
pub struct IO<'a, S: Copy>(&'a PCM, PhantomData<S>);

impl<'a, S: Copy> IO<'a, S> {

    fn to_frames(&self, b: usize) -> alsa::snd_pcm_uframes_t {
        // TODO: Do we need to check for overflow here?
        self.0.bytes_to_frames((b * size_of::<S>()) as isize) as alsa::snd_pcm_uframes_t
    }

    fn from_frames(&self, b: alsa::snd_pcm_uframes_t) -> usize {
        // TODO: Do we need to check for overflow here?
        (self.0.frames_to_bytes(b as Frames) as usize) * size_of::<S>()
    }

    /// On success, returns number of *frames* written.
    /// (Multiply with number of channels to get number of items in buf successfully written.)
    pub fn writei(&self, buf: &[S]) -> Result<usize> {
        acheck!(snd_pcm_writei((self.0).0, buf.as_ptr() as *const c_void, self.to_frames(buf.len()))).map(|r| r as usize)
    }

    /// On success, returns number of *frames* read.
    /// (Multiply with number of channels to get number of items in buf successfully read.)
    pub fn readi(&self, buf: &mut [S]) -> Result<usize> {
        acheck!(snd_pcm_readi((self.0).0, buf.as_mut_ptr() as *mut c_void, self.to_frames(buf.len()))).map(|r| r as usize)
    }

    /// Wrapper around snd_pcm_mmap_begin and snd_pcm_mmap_commit.
    ///
    /// You can read/write into the sound card's buffer during the call to the closure.
    /// According to alsa-lib docs, you should call avail_update before calling this function.
    ///
    /// All calculations are in *frames*, i e, the closure should return number of frames processed.
    /// Also, there might not be as many frames to read/write as requested, and there can even be
    /// an empty buffer supplied to the closure.
    ///
    /// Note: This function works only with interleaved access mode.
    pub fn mmap<F: FnOnce(&mut [S]) -> usize>(&self, frames: usize, func: F) -> Result<usize> {
        let mut f = frames as alsa::snd_pcm_uframes_t;
        let mut offs: alsa::snd_pcm_uframes_t = 0;
        let mut areas = ptr::null();
        try!(acheck!(snd_pcm_mmap_begin((self.0).0, &mut areas, &mut offs, &mut f)));
        let buf = unsafe {
            let p = ((*areas).addr as *mut S).offset(self.from_frames(offs) as isize);
            ::std::slice::from_raw_parts_mut(p, self.from_frames(f))
        };
        let fres = func(buf);
        debug_assert!(fres <= f as usize);
        acheck!(snd_pcm_mmap_commit((self.0).0, offs, fres as alsa::snd_pcm_uframes_t)).map(|r| r as usize)
    }
}

impl<'a, S: Copy> io::Read for IO<'a, S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let size = self.0.bytes_to_frames(buf.len() as isize) as alsa::snd_pcm_uframes_t; // TODO: Do we need to check for overflow here?
        let r = unsafe { alsa::snd_pcm_readi((self.0).0, buf.as_mut_ptr() as *mut c_void, size) };
        if r < 0 { Err(io::Error::from_raw_os_error(r as i32)) }
        else { Ok(self.0.frames_to_bytes(r) as usize) }
    }
}

impl<'a, S: Copy> io::Write for IO<'a, S> {
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
    S8 = alsa::SND_PCM_FORMAT_S8 as isize,
    U8 = alsa::SND_PCM_FORMAT_U8 as isize,
    S16LE = alsa::SND_PCM_FORMAT_S16_LE as isize,
    S16BE = alsa::SND_PCM_FORMAT_S16_BE as isize,
    U16LE = alsa::SND_PCM_FORMAT_U16_LE as isize,
    U16BE = alsa::SND_PCM_FORMAT_U16_BE as isize,
    S32LE = alsa::SND_PCM_FORMAT_S32_LE as isize,
    S32BE = alsa::SND_PCM_FORMAT_S32_BE as isize,
    U32LE = alsa::SND_PCM_FORMAT_U32_LE as isize,
    U32BE = alsa::SND_PCM_FORMAT_U32_BE as isize,
    FloatLE = alsa::SND_PCM_FORMAT_FLOAT_LE as isize,
    FloatBE = alsa::SND_PCM_FORMAT_FLOAT_BE as isize,
    Float64LE = alsa::SND_PCM_FORMAT_FLOAT64_LE as isize,
    Float64BE = alsa::SND_PCM_FORMAT_FLOAT64_BE as isize,
    // TODO: More formats...
}

impl Format {
    #[cfg(target_endian = "little")] pub fn s16() -> Format { Format::S16LE }
    #[cfg(target_endian = "big")] pub fn s16() -> Format { Format::S16BE }

    #[cfg(target_endian = "little")] pub fn u16() -> Format { Format::U16LE }
    #[cfg(target_endian = "big")] pub fn u16() -> Format { Format::U16BE }

    #[cfg(target_endian = "little")] pub fn s32() -> Format { Format::S32LE }
    #[cfg(target_endian = "big")] pub fn s32() -> Format { Format::S32BE }

    #[cfg(target_endian = "little")] pub fn u32() -> Format { Format::U32LE }
    #[cfg(target_endian = "big")] pub fn u32() -> Format { Format::U32BE }

    #[cfg(target_endian = "little")] pub fn float() -> Format { Format::FloatLE }
    #[cfg(target_endian = "big")] pub fn float() -> Format { Format::FloatBE }

    #[cfg(target_endian = "little")] pub fn float64() -> Format { Format::Float64LE }
    #[cfg(target_endian = "big")] pub fn float64() -> Format { Format::Float64BE }
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
        acheck!(snd_pcm_hw_params_malloc(&mut p)).map(|_| HwParams(p, a))
    }

    pub fn any(a: &'a PCM) -> Result<HwParams<'a>> { HwParams::new(a).and_then(|p|
        acheck!(snd_pcm_hw_params_any(a.0, p.0)).map(|_| p)
    )}

    pub fn set_channels(&self, v: u32) -> Result<()> {
        acheck!(snd_pcm_hw_params_set_channels((self.1).0, self.0, v as c_uint)).map(|_| ())
    }

    pub fn get_channels(&self) -> Result<u32> {
        let mut v = 0;
        acheck!(snd_pcm_hw_params_get_channels(self.0, &mut v)).map(|_| v as u32)
    }

    pub fn set_rate(&self, v: u32, dir: i32) -> Result<()> {
        acheck!(snd_pcm_hw_params_set_rate((self.1).0, self.0, v as c_uint, dir as c_int)).map(|_| ())
    }

    pub fn get_rate(&self) -> Result<u32> {
        let (mut v, mut d) = (0,0);
        acheck!(snd_pcm_hw_params_get_rate(self.0, &mut v, &mut d)).map(|_| v as u32)
    }

    pub fn set_format(&self, v: Format) -> Result<()> {
        acheck!(snd_pcm_hw_params_set_format((self.1).0, self.0, v as c_int)).map(|_| ())
    }

    pub fn get_format(&self) -> Result<Format> {
        let mut v = 0;
        acheck!(snd_pcm_hw_params_get_format(self.0, &mut v)).map(|_| unsafe { mem::transmute(v as u8) })
    }

    pub fn set_access(&self, v: Access) -> Result<()> {
        acheck!(snd_pcm_hw_params_set_access((self.1).0, self.0, v as c_uint)).map(|_| ())
    }

    pub fn get_access(&self) -> Result<Access> {
        let mut v = 0;
        acheck!(snd_pcm_hw_params_get_access(self.0, &mut v)).map(|_| unsafe { mem::transmute(v as u8) })
    }

    pub fn set_period_size(&self, v: Frames, dir: i32) -> Result<()> {
        acheck!(snd_pcm_hw_params_set_period_size((self.1).0, self.0, v as alsa::snd_pcm_uframes_t, dir as c_int)).map(|_| ())
    }

    pub fn get_period_size(&self) -> Result<Frames> {
        let (mut v, mut d) = (0,0);
        acheck!(snd_pcm_hw_params_get_period_size(self.0, &mut v, &mut d)).map(|_| v as Frames)
    }

    pub fn set_periods(&self, v: u32, dir: i32) -> Result<()> {
        acheck!(snd_pcm_hw_params_set_periods((self.1).0, self.0, v as c_uint, dir as c_int)).map(|_| ())
    }

    pub fn get_periods(&self) -> Result<u32> {
        let (mut v, mut d) = (0,0);
        acheck!(snd_pcm_hw_params_get_periods(self.0, &mut v, &mut d)).map(|_| v as u32)
    }

    pub fn set_buffer_size(&self, v: Frames) -> Result<()> {
        acheck!(snd_pcm_hw_params_set_buffer_size((self.1).0, self.0, v as alsa::snd_pcm_uframes_t)).map(|_| ())
    }

    pub fn get_buffer_size(&self) -> Result<Frames> {
        let mut v = 0;
        acheck!(snd_pcm_hw_params_get_buffer_size(self.0, &mut v)).map(|_| v as Frames)
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
        acheck!(snd_pcm_sw_params_malloc(&mut p)).map(|_| SwParams(p, a))
    }

    pub fn set_avail_min(&self, v: Frames) -> Result<()> {
        acheck!(snd_pcm_sw_params_set_avail_min((self.1).0, self.0, v as alsa::snd_pcm_uframes_t)).map(|_| ())
    }

    pub fn get_avail_min(&self) -> Result<Frames> {
        let mut v = 0;
        acheck!(snd_pcm_sw_params_get_avail_min(self.0, &mut v)).map(|_| v as Frames)
    }

    pub fn set_start_threshold(&self, v: Frames) -> Result<()> {
        acheck!(snd_pcm_sw_params_set_start_threshold((self.1).0, self.0, v as alsa::snd_pcm_uframes_t)).map(|_| ())
    }

    pub fn get_start_threshold(&self) -> Result<Frames> {
        let mut v = 0;
        acheck!(snd_pcm_sw_params_get_start_threshold(self.0, &mut v)).map(|_| v as Frames)
    }

    pub fn set_stop_threshold(&self, v: Frames) -> Result<()> {
        acheck!(snd_pcm_sw_params_set_stop_threshold((self.1).0, self.0, v as alsa::snd_pcm_uframes_t)).map(|_| ())
    }

    pub fn get_stop_threshold(&self) -> Result<Frames> {
        let mut v = 0;
        acheck!(snd_pcm_sw_params_get_stop_threshold(self.0, &mut v)).map(|_| v as Frames)
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
    let pcm = PCM::open(&*CString::new("default").unwrap(), Direction::Capture, false).unwrap();
    let hwp = HwParams::any(&pcm).unwrap();
    hwp.set_channels(2).unwrap();
    hwp.set_rate(44100, 0).unwrap();
    hwp.set_format(Format::s16()).unwrap();
    hwp.set_access(Access::RWInterleaved).unwrap();
    pcm.hw_params(&hwp).unwrap();
    pcm.start().unwrap();
    let mut buf = [0i16; 1024];
    assert_eq!(pcm.io_i16().unwrap().readi(&mut buf).unwrap(), 1024/2);
}

#[test]
fn playback_to_default() {
    use std::ffi::CString;
    let pcm = PCM::open(&*CString::new("default").unwrap(), Direction::Playback, false).unwrap();
    let hwp = HwParams::any(&pcm).unwrap();
    hwp.set_channels(1).unwrap();
    hwp.set_rate(44100, 0).unwrap();
    hwp.set_format(Format::s16()).unwrap();
    hwp.set_access(Access::RWInterleaved).unwrap();
    pcm.hw_params(&hwp).unwrap();
    println!("PCM status: {:?}, {:?}", pcm.state(), pcm.hw_params_current().unwrap());
    let mut buf = [0i16; 1024];
    for (i, a) in buf.iter_mut().enumerate() {
        *a = ((i as f32 * 2.0 * ::std::f32::consts::PI / 128.0).sin() * 8192.0) as i16
    }
    let io = pcm.io_i16().unwrap();
    for _ in 0..2*44100/1024 { // 2 seconds of playback
        assert_eq!(io.writei(&buf[..]).unwrap(), 1024);
    }
    if pcm.state() != State::Running { pcm.start().unwrap() };
    pcm.drain().unwrap();
}
