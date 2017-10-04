//! Experimental stuff

use libc;
use std::{mem, ptr, fmt};
use error::{Error, Result};
use std::os::unix::io::RawFd;
use {pcm, PollDescriptors};

// Some definitions from the kernel headers

// const SNDRV_PCM_MMAP_OFFSET_DATA: c_uint = 0x00000000;
const SNDRV_PCM_MMAP_OFFSET_STATUS: libc::c_uint = 0x80000000;
const SNDRV_PCM_MMAP_OFFSET_CONTROL: libc::c_uint = 0x81000000;

// #[repr(C)]
#[allow(non_camel_case_types)]
type snd_pcm_state_t = libc::c_int;

// #[repr(C)]
#[allow(non_camel_case_types)]
type snd_pcm_uframes_t = libc::c_ulong;

// I think?! Not sure how this will work with X32 ABI?!
#[allow(non_camel_case_types)]
type __kernel_off_t = libc::c_long;

#[repr(C)]
struct snd_pcm_mmap_status {
	pub state: snd_pcm_state_t,		/* RO: state - SNDRV_PCM_STATE_XXXX */
	pub pad1: libc::c_int,			/* Needed for 64 bit alignment */
	pub hw_ptr: snd_pcm_uframes_t,	/* RO: hw ptr (0...boundary-1) */
	pub tstamp: libc::timespec,		/* Timestamp */
	pub suspended_state: snd_pcm_state_t, /* RO: suspended stream state */
	pub audio_tstamp: libc::timespec,	/* from sample counter or wall clock */
}

#[repr(C)]
#[derive(Debug)]
struct snd_pcm_mmap_control {
	pub appl_ptr: snd_pcm_uframes_t,	/* RW: appl ptr (0...boundary-1) */
	pub avail_min: snd_pcm_uframes_t,	/* RW: min available frames for wakeup */
}

#[repr(C)]
#[derive(Debug)]
pub struct snd_pcm_channel_info {
	pub channel: libc::c_uint,
	pub offset: __kernel_off_t,		/* mmap offset */
	pub first: libc::c_uint,		/* offset to first sample in bits */
	pub step: libc::c_uint, 		/* samples distance in bits */
}

ioctl!(read sndrv_pcm_ioctl_channel_info with b'A', 0x32; snd_pcm_channel_info);

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
/// Status structs are dropped too.
///
#[derive(Debug)]
pub struct Status(DriverMemory<snd_pcm_mmap_status>);

fn pcm_to_fd(p: &pcm::PCM) -> Result<RawFd> {
    let mut fds: [libc::pollfd; 1] = unsafe { mem::zeroed() };
    let c = (p as &PollDescriptors).fill(&mut fds)?;
    if c != 1 {
        return Err(Error::new(Some("snd_pcm_poll_descriptors returned wrong number of fds".into()), c as libc::c_int))
    }
    Ok(fds[0].fd)
}

impl Status {
    pub fn new(p: &pcm::PCM) -> Result<Self> { Status::from_fd(pcm_to_fd(p)?) }

    pub fn from_fd(fd: RawFd) -> Result<Self> {
        DriverMemory::new(fd, 1, SNDRV_PCM_MMAP_OFFSET_STATUS as libc::off_t, false).map(|d| Status(d))
    }

    /// Current PCM state.
    pub fn state(&self) -> pcm::State {
        unsafe {
            let i = ptr::read_volatile(&(*self.0.ptr).state);
            assert!((i >= (pcm::State::Open as snd_pcm_state_t)) && (i <= (pcm::State::Disconnected as snd_pcm_state_t)));
            mem::transmute(i as u8)
        }
    }

    /// Number of frames hardware has read or written
    ///
    /// This number is updated every now and then by the kernel.
    /// Calling most functions on the PCM will update it, so will usually a period interrupt.
    /// No guarantees given.
    ///
    /// This value wraps at buffer boundary.
    pub fn hw_ptr(&self) -> pcm::Frames {
        unsafe {
            ptr::read_volatile(&(*self.0.ptr).hw_ptr) as pcm::Frames
        }
    }

    /// Timestamp - fast version of alsa-lib's Status::get_htstamp
    ///
    /// Note: This just reads the actual value in memory.
    /// Unfortunately, the timespec is too big to be read atomically on most archs.
    /// Therefore, this function can potentially give bogus result at times, at least in theory...?
    pub fn htstamp(&self) -> libc::timespec {
        unsafe {
            ptr::read_volatile(&(*self.0.ptr).tstamp)
        }
    }

    /// Audio timestamp - fast version of alsa-lib's Status::get_audio_htstamp
    ///
    /// Note: This just reads the actual value in memory.
    /// Unfortunately, the timespec is too big to be read atomically on most archs.
    /// Therefore, this function can potentially give bogus result at times, at least in theory...?
    pub fn audio_htstamp(&self) -> libc::timespec {
        unsafe {
            ptr::read_volatile(&(*self.0.ptr).audio_tstamp)
        }
    }
}

/// Write PCM appl ptr directly, bypassing alsa-lib.
///
/// Provides direct access to appl ptr and avail min, without the overhead of
/// alsa-lib or a syscall. Caveats that apply to Status applies to this struct too.
#[derive(Debug)]
pub struct Control(DriverMemory<snd_pcm_mmap_control>);

impl Control {
    pub fn new(p: &pcm::PCM) -> Result<Self> { Self::from_fd(pcm_to_fd(p)?) }

    pub fn from_fd(fd: RawFd) -> Result<Self> {
        DriverMemory::new(fd, 1, SNDRV_PCM_MMAP_OFFSET_CONTROL as libc::off_t, true).map(|d| Control(d))
    }

    /// Read number of frames application has read or written
    ///
    /// This value wraps at buffer boundary.
    pub fn appl_ptr(&self) -> pcm::Frames {
        unsafe {
            ptr::read_volatile(&(*self.0.ptr).appl_ptr) as pcm::Frames
        }
    }

    /// Set number of frames application has read or written
    ///
    /// When the kernel wakes up due to a period interrupt, this value will
    /// be checked by the kernel. An XRUN will happen in case the application
    /// has not read or written enough data.
    pub fn set_appl_ptr(&self, value: pcm::Frames) {
        unsafe {
            ptr::write_volatile(&mut (*self.0.ptr).appl_ptr, value as snd_pcm_uframes_t)
        }
    }

    /// Read minimum number of frames in buffer in order to wakeup process
    pub fn avail_min(&self) -> pcm::Frames {
        unsafe {
            ptr::read_volatile(&(*self.0.ptr).avail_min) as pcm::Frames
        }
    }

    /// Write minimum number of frames in buffer in order to wakeup process
    pub fn set_avail_min(&self, value: pcm::Frames) {
        unsafe {
            ptr::write_volatile(&mut (*self.0.ptr).avail_min, value as snd_pcm_uframes_t)
        }
    }
}

struct DriverMemory<T> {
   ptr: *mut T, 
   size: libc::size_t,
}

impl<T> fmt::Debug for DriverMemory<T> {
   fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "DriverMemory({:?})", self.ptr) }
}

impl<T> DriverMemory<T> {
    fn new(fd: RawFd, count: usize, offs: libc::off_t, writable: bool) -> Result<Self> {
        let mut total = count * mem::size_of::<T>();
        let ps = pagesize();
        assert!(total > 0);
        if total % ps != 0 { total += ps - total % ps };
        let flags = if writable { libc::PROT_WRITE | libc::PROT_READ } else { libc::PROT_READ };
        let p = unsafe { libc::mmap(ptr::null_mut(), total, flags, libc::MAP_FILE | libc::MAP_SHARED, fd, offs) };
        if p == ptr::null_mut() || p == libc::MAP_FAILED {
            return Err(Error::new(Some("driver memory mmap".into()), -1))
        }
        Ok(DriverMemory { ptr: p as *mut T, size: total })
    }
}

unsafe impl<T> Send for DriverMemory<T> {}
unsafe impl<T> Sync for DriverMemory<T> {}

impl<T> Drop for DriverMemory<T> {
    fn drop(&mut self) {
        unsafe {{ libc::munmap(self.ptr as *mut libc::c_void, self.size); } }
    }
}

#[derive(Debug)]
pub struct SampleData<T> { 
   mem: DriverMemory<T>,
   frames: pcm::Frames,
   channels: u32,
}

impl<T> SampleData<T> {
    pub fn new(p: &pcm::PCM) -> Result<Self> {
        let params = p.hw_params_current()?;
        let bufsize = params.get_buffer_size()?;
        let channels = params.get_channels()?;
        if params.get_access()? != pcm::Access::MMapInterleaved {
            return Err(Error::new(Some("Not MMAP interleaved data".into()), -1))
        }

        let fd = pcm_to_fd(p)?;
        let info = unsafe {
            let mut info: snd_pcm_channel_info = mem::zeroed();
            sndrv_pcm_ioctl_channel_info(fd, &mut info).map_err(|_| Error::new(Some("SNDRV_PCM_IOCTL_CHANNEL_INFO".into()), -1))?;
            info
        };
        println!("{:?}", info);
        if (info.step != channels * mem::size_of::<T>() as u32 * 8) || (info.first != 0) {
            return Err(Error::new(Some("MMAP data size mismatch".into()), -1))
        }
        Ok(SampleData {
            mem: DriverMemory::new(fd, (bufsize as usize) * (channels as usize), info.offset, true)?,
            frames: bufsize,
            channels: channels,
        })
    }
}


#[test]
#[ignore] // Not everyone has a recording device on plughw:1. So let's ignore by default.
fn record_from_plughw_rw() {
    use pcm::*;
    use {ValueOr, Direction};
    use std::ffi::CString;
    let pcm = PCM::open(&*CString::new("plughw:1").unwrap(), Direction::Capture, false).unwrap();
    let ss = self::Status::new(&pcm).unwrap();
    let c = self::Control::new(&pcm).unwrap();
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
    assert_eq!(c.appl_ptr(), 0);
    println!("{:?}, {:?}", ss, c);
    let mut buf = [0i16; 512*2];
    assert_eq!(pcm.io_i16().unwrap().readi(&mut buf).unwrap(), 512);
    assert_eq!(c.appl_ptr(), 512);

    assert_eq!(ss.state(), State::Running);
    assert!(ss.hw_ptr() >= 512);
    let t2 = ss.htstamp();
    assert!(t2.tv_sec > 0 || t2.tv_sec > 0);
}


#[test]
#[ignore] // Not everyone has a playback device on plughw:1. So let's ignore by default.
fn playback_to_plughw_mmap() {
    use pcm::*;
    use {ValueOr, Direction};
    use std::ffi::CString;
    let pcm = PCM::open(&*CString::new("plughw:1").unwrap(), Direction::Playback, false).unwrap();
    let ss = self::Status::new(&pcm).unwrap();
    let c = self::Control::new(&pcm).unwrap();
    let hwp = HwParams::any(&pcm).unwrap();
    hwp.set_channels(2).unwrap();
    hwp.set_rate(44100, ValueOr::Nearest).unwrap();
    hwp.set_format(Format::s16()).unwrap();
    hwp.set_access(Access::MMapInterleaved).unwrap();
    pcm.hw_params(&hwp).unwrap();

    assert_eq!(ss.state(), State::Prepared);
    assert_eq!(c.appl_ptr(), 0);

    let data = SampleData::<i16>::new(&pcm).unwrap();
    println!("{:?}, {:?}, {:?}", data, ss, c);
}

