use alsa;
use libc;
use std::{fmt, mem, ptr, slice};
use super::error::*;


/// [SND_CHMAP_TYPE_xxx](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m.html) constants
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChmapType {
    None = alsa::SND_CHMAP_TYPE_NONE as isize,
    Fixed = alsa::SND_CHMAP_TYPE_FIXED as isize,
    Var = alsa::SND_CHMAP_TYPE_VAR as isize,
    Paired = alsa::SND_CHMAP_TYPE_PAIRED as isize,
}

/// [SND_CHMAP_xxx](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m.html) constants
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChmapPosition {
    Unknown = alsa::SND_CHMAP_UNKNOWN as isize,
    NA = alsa::SND_CHMAP_NA as isize,
    Mono = alsa::SND_CHMAP_MONO as isize,
    FL = alsa::SND_CHMAP_FL as isize,
    FR = alsa::SND_CHMAP_FR as isize,
    RL = alsa::SND_CHMAP_RL as isize,
    SR = alsa::SND_CHMAP_SR as isize,
    RC = alsa::SND_CHMAP_RC as isize,
    FLC = alsa::SND_CHMAP_FLC as isize,
    FRC = alsa::SND_CHMAP_FRC as isize,
    RLC = alsa::SND_CHMAP_RLC as isize,
    RRC = alsa::SND_CHMAP_RRC as isize,
    FLW = alsa::SND_CHMAP_FLW as isize,
    FRW = alsa::SND_CHMAP_FRW as isize,
    FLH = alsa::SND_CHMAP_FLH as isize,
    FCH = alsa::SND_CHMAP_FCH as isize,
    FRH = alsa::SND_CHMAP_FRH as isize,
    TC = alsa::SND_CHMAP_TC as isize,
    TFL = alsa::SND_CHMAP_TFL as isize,
    TFR = alsa::SND_CHMAP_TFR as isize,
    TFC = alsa::SND_CHMAP_TFC as isize,
    TRL = alsa::SND_CHMAP_TRL as isize,
    TRR = alsa::SND_CHMAP_TRR as isize,
    TRC = alsa::SND_CHMAP_TRC as isize,
    TFLC = alsa::SND_CHMAP_TFLC as isize,
    TFRC = alsa::SND_CHMAP_TFRC as isize,
    TSL = alsa::SND_CHMAP_TSL as isize,
    TSR = alsa::SND_CHMAP_TSR as isize,
    LLFE = alsa::SND_CHMAP_LLFE as isize,
    RLFE = alsa::SND_CHMAP_RLFE as isize,
    BC = alsa::SND_CHMAP_BC as isize,
    BLC = alsa::SND_CHMAP_BLC as isize,
    BRC = alsa::SND_CHMAP_BRC as isize,
}

impl fmt::Display for ChmapPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = unsafe { alsa::snd_pcm_chmap_long_name(*self as libc::c_uint) };
        let s = try!(from_const("snd_pcm_chmap_long_name", s));
        write!(f, "{}", s)
    }
}


/// [snd_pcm_chmap_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m.html) wrapper
pub struct Chmap(*mut alsa::snd_pcm_chmap_t, bool);

impl Drop for Chmap {
    fn drop(&mut self) { if self.1 { unsafe { libc::free(self.0 as *mut libc::c_void) }}}
}

impl Chmap {
    fn set_channels(&mut self, c: libc::c_uint) { unsafe { (*self.0) .channels = c }}
    fn as_slice_mut(&mut self) -> &mut [libc::c_uint] {
        unsafe { slice::from_raw_parts_mut(&mut (*self.0).pos[0], (*self.0).channels as usize) }
    }
    fn as_slice(&self) -> &[libc::c_uint] {
        unsafe { slice::from_raw_parts(&mut (*self.0).pos[0], (*self.0).channels as usize) }
    }
}

impl fmt::Display for Chmap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buf: Vec<libc::c_char> = vec![0; 512];
        try!(acheck!(snd_pcm_chmap_print(self.0, buf.len() as libc::size_t, buf.as_mut_ptr())));
        let s = try!(from_const("snd_pcm_chmap_print", buf.as_mut_ptr()));
        write!(f, "{}", s)
    }
}

impl<'a> From<&'a [ChmapPosition]> for Chmap {
    fn from(a: &'a [ChmapPosition]) -> Chmap {
        let p = unsafe { libc::malloc((mem::size_of::<alsa::snd_pcm_chmap_t>() + mem::size_of::<libc::c_uint>() * a.len()) as libc::size_t) };
        if p == ptr::null_mut() { panic!("Out of memory") }
        let mut r = Chmap(p as *mut alsa::snd_pcm_chmap_t, true);
        r.set_channels(a.len() as libc::c_uint);
        for (i,v) in r.as_slice_mut().iter_mut().enumerate() { *v = a[i] as libc::c_uint }
        r
    }
}

impl<'a> From<&'a Chmap> for Vec<ChmapPosition> {
    fn from(a: &'a Chmap) -> Vec<ChmapPosition> {
        a.as_slice().iter().map(|&v| unsafe { mem::transmute(v as u8) }).collect()
    }
}

pub fn chmap_new(a: *mut alsa::snd_pcm_chmap_t) -> Chmap { Chmap(a, true) }
pub fn chmap_handle(a: &Chmap) -> *mut alsa::snd_pcm_chmap_t { a.0 }


/// Iterator over available channel maps - see [snd_pcm_chmap_query_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___p_c_m.html)
pub struct ChmapsQuery(*mut *mut alsa::snd_pcm_chmap_query_t, isize);

impl Drop for ChmapsQuery {
    fn drop(&mut self) { unsafe { alsa::snd_pcm_free_chmaps(self.0) }}
}

pub fn chmaps_query_new(a: *mut *mut alsa::snd_pcm_chmap_query_t) -> ChmapsQuery { ChmapsQuery(a, 0) }

impl Iterator for ChmapsQuery {
    type Item = (ChmapType, Chmap);
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == ptr::null_mut() { return None; }
        let p = unsafe { *self.0.offset(self.1) };
        if p == ptr::null_mut() { return None; }
        self.1 += 1;
        let t = unsafe { mem::transmute((*p)._type as u8) };
        let m = Chmap(unsafe { &mut (*p).map }, false);
        Some((t, m))
    }
}


#[test]
fn chmap_for_first_pcm() {
    use super::*;
    use std::ffi::CString;
    use device_name::HintIter;
    let mut i = HintIter::new(None, &*CString::new("pcm").unwrap()).unwrap();

    let a = PCM::open(&CString::new(i.next().unwrap().name.unwrap()).unwrap(), Direction::Playback, false).unwrap();
    for c in a.query_chmaps() {
        println!("{:?}, {}", c.0, c.1);
    }
}
