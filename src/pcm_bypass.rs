//! Experimental stuff

use libc;
use std::{mem, ptr};
use error::{Error, Result};
use std::os::unix::io::RawFd;
use pcm;

// Some definitions from the kernel headers

// const SNDRV_PCM_MMAP_OFFSET_DATA: c_uint = 0x00000000;
const SNDRV_PCM_MMAP_OFFSET_STATUS: libc::c_uint = 0x80000000;
// const SNDRV_PCM_MMAP_OFFSET_CONTROL: c_uint = 0x81000000;

// #[repr(C)]
#[allow(non_camel_case_types)]
type snd_pcm_state_t = libc::c_int;

// #[repr(C)]
#[allow(non_camel_case_types)]
type snd_pcm_uframes_t = libc::c_ulong;

#[repr(C)]
struct snd_pcm_mmap_status {
	pub state: snd_pcm_state_t,		/* RO: state - SNDRV_PCM_STATE_XXXX */
	pub pad1: libc::c_int,			/* Needed for 64 bit alignment */
	pub hw_ptr: snd_pcm_uframes_t,	/* RO: hw ptr (0...boundary-1) */
	pub tstamp: libc::timespec,		/* Timestamp */
	pub suspended_state: snd_pcm_state_t, /* RO: suspended stream state */
	pub audio_tstamp: libc::timespec,	/* from sample counter or wall clock */
}

fn pagesize() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

/// Read PCM status directly, bypassing alsa-lib.
///
/// This means that it's
/// 1) less overhead for reading status (no syscall, no allocations, no virtual dispatch, just a read from memory)
/// 2) Send + Sync, and
/// 3) will only work for "hw" / "plughw" devices (not e g PulseAudio plugins), and not
/// all of those are supported, although all common ones are (as of 2017, and a kernel from the same decade).
///
/// The values are updated every now and then by the kernel. Many functions will force an update to happen,
/// e g `PCM::avail()` and `PCM::delay()`.
///
/// Note: Even if you close the original PCM device, ALSA will not actually close the device until all
/// PcmStatus structs are dropped too.
pub struct PcmStatus {
    area: *mut libc::c_void
}

unsafe impl Send for PcmStatus {}
unsafe impl Sync for PcmStatus {}

impl PcmStatus {
    pub fn new(p: &pcm::PCM) -> Result<Self> {
        use PollDescriptors;
        let mut fds: [libc::pollfd; 1] = unsafe { mem::zeroed() };
        let c = (p as &PollDescriptors).fill(&mut fds)?;
        if c != 1 {
            return Err(Error::new(Some("snd_pcm_poll_descriptors returned wrong number of fds".into()), c as libc::c_int))
        }
        PcmStatus::from_fd(fds[0].fd)
    }

    pub fn from_fd(fd: RawFd) -> Result<Self> {
        let ps = pagesize();
        assert!(mem::size_of::<snd_pcm_mmap_status>() < ps);
        let p = unsafe { libc::mmap(ptr::null_mut(), ps, libc::PROT_READ, libc::MAP_FILE | libc::MAP_SHARED, 
			   fd, SNDRV_PCM_MMAP_OFFSET_STATUS as libc::off_t) };
        if p == ptr::null_mut() || p == libc::MAP_FAILED {
            return Err(Error::new(Some("mmap (PcmStatus)".into()), -1))
        }
        Ok(PcmStatus { area: p })
    }

    /// Current PCM state.
    pub fn state(&self) -> pcm::State {
        unsafe {
            let p: *const snd_pcm_mmap_status = self.area as *const _;
            let i = ptr::read_volatile(&(*p).state);
            assert!((i >= (pcm::State::Open as snd_pcm_state_t)) && (i <= (pcm::State::Disconnected as snd_pcm_state_t)));
            mem::transmute(i as u8)
        }
    }

    /// Number of frames played back or recorded
    ///
    /// This number is updated every now and then by the kernel.
    /// Calling most functions on the PCM will update it, so will usually a period interrupt.
    /// No guarantees given.
    pub fn hw_ptr(&self) -> pcm::Frames {
        unsafe {
            let p: *const snd_pcm_mmap_status = self.area as *const _;
            ptr::read_volatile(&(*p).hw_ptr) as pcm::Frames
        }
    }

    /// Timestamp - fast version of Status::get_htstamp
    ///
    /// Note: This just reads the actual value in memory.
    /// Unfortunately, the timespec is too big to be read atomically on most archs.
    /// Therefore, this function can potentially give bogus result at times, at least in theory...?
    pub fn htstamp(&self) -> libc::timespec {
        unsafe {
            let p: *const snd_pcm_mmap_status = self.area as *const _;
            ptr::read_volatile(&(*p).tstamp)
        }
    }

    /// Audio timestamp - fast version of Status::get_audio_htstamp
    ///
    /// Note: This just reads the actual value in memory.
    /// Unfortunately, the timespec is too big to be read atomically on most archs.
    /// Therefore, this function can potentially give bogus result at times, at least in theory...?
    pub fn audio_htstamp(&self) -> libc::timespec {
        unsafe {
            let p: *const snd_pcm_mmap_status = self.area as *const _;
            ptr::read_volatile(&(*p).audio_tstamp)
        }
    }
}



impl Drop for PcmStatus {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.area, pagesize()); }
    }
}

#[test]
#[ignore] // Not everyone has a recording device on plughw:1. So let's ignore by default.
fn record_from_plughw() {
    use pcm::*;
    use {ValueOr, Direction};
    use std::ffi::CString;
    let pcm = PCM::open(&*CString::new("plughw:1").unwrap(), Direction::Capture, false).unwrap();
    let ss = PcmStatus::new(&pcm).unwrap();
    let hwp = HwParams::any(&pcm).unwrap();
    hwp.set_channels(2).unwrap();
    hwp.set_rate(44100, ValueOr::Nearest).unwrap();
    hwp.set_format(Format::s16()).unwrap();
    hwp.set_access(Access::RWInterleaved).unwrap();
    pcm.hw_params(&hwp).unwrap();

    {
        let swp = pcm.sw_params_current().unwrap();
        swp.set_tstamp_mode(true).unwrap();
        pcm.sw_params(&swp).unwrap();
    }
    assert_eq!(ss.state(), State::Prepared);
    pcm.start().unwrap();
    let mut buf = [0i16; 1024];
    assert_eq!(pcm.io_i16().unwrap().readi(&mut buf).unwrap(), 1024/2);

    assert_eq!(ss.state(), State::Running);
    assert!(ss.hw_ptr() >= 1024/2);
    let t2 = ss.htstamp();
    assert!(t2.tv_sec > 0 || t2.tv_sec > 0);
}

