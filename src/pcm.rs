use libc::{c_int, c_uint, c_void, ssize_t};
use alsa;
use std::ffi::CStr;
use std::io;
use super::error::*;
use super::Direction;
use std::ptr;
//use std::mem::size_of;

pub type Frames = alsa::snd_pcm_sframes_t;

pub struct PCM(*mut alsa::snd_pcm_t);

impl PCM {
    // Does not offer async mode (it's not very Rustic anyway)
    pub fn new(name: &CStr, dir: Direction, nonblock: bool) -> Result<PCM> {
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

    pub fn state(&self) -> PCMState { unsafe { ::std::mem::transmute(alsa::snd_pcm_state(self.0) as u8) } }

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

    pub fn io<'a>(&'a self) -> PCMIO<'a> { PCMIO(&self) }

    pub fn hw_params(&self, h: &PCMHwParams) -> Result<()> {
        check("snd_pcm_hw_params", unsafe { alsa::snd_pcm_hw_params(self.0, h.0) }).map(|_| ())
    }

    pub fn hw_params_current<'a>(&'a self) -> Result<PCMHwParams<'a>> {
        PCMHwParams::new(&self).and_then(|h|
            check("snd_pcm_hw_params_current", unsafe { alsa::snd_pcm_hw_params_current(self.0, h.0) }).map(|_| h))
    }

/*    /// Returns number of T's in buf that are filled in.
    pub fn readi<T:Copy>(&self, buf: &mut [T]) -> Result<usize> {
        let size = (buf.len() * size_of::<T>()) as alsa::snd_pcm_uframes_t;
        let r = unsafe { alsa::snd_pcm_readi(self.0, buf.as_mut_ptr() as *mut c_void, size) };
        check("snd_pcm_readi", if r < 0 { r as c_int } else { 0 })
            .map(|_| (self.frames_to_bytes(r) as usize) / size_of::<T>())
    } */


}

impl Drop for PCM {
    fn drop(&mut self) { unsafe { alsa::snd_pcm_close(self.0) }; }
}

/// The reason we have a separate PCMIO struct is because read and write takes &mut self,
/// where as we only need and want &self for PCM.
pub struct PCMIO<'a>(&'a PCM);

impl<'a> io::Read for PCMIO<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let size = self.0.bytes_to_frames(buf.len() as isize) as alsa::snd_pcm_uframes_t; // TODO: Do we need to check for overflow here?
        let r = unsafe { alsa::snd_pcm_readi((self.0).0, buf.as_mut_ptr() as *mut c_void, size) };
        if r < 0 { Err(io::Error::from_raw_os_error(r as i32)) }
        else { Ok(self.0.frames_to_bytes(r) as usize) }
    }
}

impl<'a> io::Write for PCMIO<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let size = self.0.bytes_to_frames(buf.len() as isize) as alsa::snd_pcm_uframes_t; // TODO: Do we need to check for overflow here?
        let r = unsafe { alsa::snd_pcm_writei((self.0).0, buf.as_ptr() as *const c_void, size) };
        if r < 0 { Err(io::Error::from_raw_os_error(r as i32)) }
        else { Ok(self.0.frames_to_bytes(r) as usize) }
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PCMState {
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PCMFormat {
    Unknown = alsa::SND_PCM_FORMAT_UNKNOWN as isize,
    // S16 = alsa::SND_PCM_FORMAT_S16 as isize,
    S16LE = alsa::SND_PCM_FORMAT_S16_LE as isize,
    FloatLE = alsa::SND_PCM_FORMAT_FLOAT_LE as isize,
    // TODO: More formats...
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PCMAccess {
    MMapInterleaved = alsa::SND_PCM_ACCESS_MMAP_INTERLEAVED as isize,
    MMapNonInterleaved = alsa::SND_PCM_ACCESS_MMAP_NONINTERLEAVED as isize,
    MMapComplex = alsa::SND_PCM_ACCESS_MMAP_COMPLEX as isize,
    RWInterleaved = alsa::SND_PCM_ACCESS_RW_INTERLEAVED as isize,
    RWNonInterleaved = alsa::SND_PCM_ACCESS_RW_NONINTERLEAVED as isize,
}

pub struct PCMHwParams<'a>(*mut alsa::snd_pcm_hw_params_t, &'a PCM);

impl<'a> Drop for PCMHwParams<'a> {
    fn drop(&mut self) { unsafe { alsa::snd_pcm_hw_params_free(self.0) }; }
}

impl<'a> PCMHwParams<'a> {
    fn new(a: &'a PCM) -> Result<PCMHwParams<'a>> {
        let mut p = ptr::null_mut();
        check("snd_pcm_hw_params_malloc", unsafe { alsa::snd_pcm_hw_params_malloc(&mut p) }).map(|_| PCMHwParams(p, a))
    }

    pub fn any(a: &'a PCM) -> Result<PCMHwParams<'a>> { PCMHwParams::new(a).and_then(|p|
        check("snd_pcm_hw_params_any", unsafe { alsa::snd_pcm_hw_params_any(a.0, p.0) }).map(|_| p)
    )}

    pub fn set_channels(&self, v: u32) -> Result<()> { check("snd_pcm_hw_params_set_channels",
        unsafe { alsa::snd_pcm_hw_params_set_channels((self.1).0, self.0, v as c_uint) }).map(|_| ())
    }

    pub fn set_rate(&self, v: u32, dir: i32) -> Result<()> { check("snd_pcm_hw_params_set_rate",
        unsafe { alsa::snd_pcm_hw_params_set_rate((self.1).0, self.0, v as c_uint, dir as c_int) }).map(|_| ())
    }

    pub fn set_format(&self, v: PCMFormat) -> Result<()> { check("snd_pcm_hw_params_set_format",
        unsafe { alsa::snd_pcm_hw_params_set_format((self.1).0, self.0, v as c_int) }).map(|_| ())
    }

    pub fn set_access(&self, v: PCMAccess) -> Result<()> { check("snd_pcm_hw_params_set_access",
        unsafe { alsa::snd_pcm_hw_params_set_access((self.1).0, self.0, v as c_uint) }).map(|_| ())
    }

}

#[test]
fn record_from_default() {
    use std::ffi::CString;
    use std::io::Read;
    let pcm = PCM::new(&*CString::new("default").unwrap(), true, false).unwrap();
    let hwp = PCMHwParams::any(&pcm).unwrap();
    hwp.set_channels(2).unwrap();
    hwp.set_rate(44100, 0).unwrap();
    hwp.set_format(PCMFormat::S16LE).unwrap();
    hwp.set_access(PCMAccess::RWInterleaved).unwrap();
    pcm.hw_params(&hwp).unwrap();
    pcm.start().unwrap();
    let mut buf = [0u8; 1024]; 
    assert_eq!(pcm.io().read(&mut buf).unwrap(), 1024);
}

#[test]
fn playback_to_default() {
    use std::ffi::CString;
    use std::io::Write;
    let pcm = PCM::new(&*CString::new("default").unwrap(), false, false).unwrap();
    let hwp = PCMHwParams::any(&pcm).unwrap();
    hwp.set_channels(1).unwrap();
    hwp.set_rate(44100, 0).unwrap();
    hwp.set_format(PCMFormat::S16LE).unwrap();
    hwp.set_access(PCMAccess::RWInterleaved).unwrap();
    pcm.hw_params(&hwp).unwrap();
    let mut buf = [0i16; 1024];
    for (i, a) in buf.iter_mut().enumerate() {
        *a = ((i as f32 * 2.0 * ::std::f32::consts::PI / 128.0).sin() * 8192.0) as i16
    }
    let b: &[u8] = unsafe { ::std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len() * 2) };
    for _ in 0..2*44100/1024 { // 2 seconds of playback
        assert_eq!(pcm.io().write(b).unwrap(), 2048);
    }
    if pcm.state() != PCMState::Running { pcm.start().unwrap() };
    pcm.drain().unwrap();
}
