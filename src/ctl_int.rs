
use alsa;
use std::ffi::{CStr, CString};
use super::error::*;
use std::{ptr, mem, fmt};
use super::Card;
use libc::{c_uint, c_void, size_t, c_long};

/// We prefer not to allocate for every ElemId, ElemInfo or ElemValue.
/// But we don't know if these will increase in the future or on other platforms.
/// Unfortunately, Rust does not support alloca, so hard-code the sizes for now.

const ELEM_ID_SIZE: usize = 64;
// const ELEM_VALUE_SIZE: usize = 1224;
// const ELEM_INFO_SIZE: usize = 272;

/// [snd_ctl_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___control.html) wrapper
pub struct Ctl(*mut alsa::snd_ctl_t);

impl Ctl {
    /// Open does not support async mode (it's not very Rustic anyway)
    pub fn open(c: &CStr, nonblock: bool) -> Result<Ctl> {
        let mut r = ptr::null_mut();
        let flags = if nonblock { 1 } else { 0 }; // FIXME: alsa::SND_CTL_NONBLOCK does not exist in alsa-sys
        acheck!(snd_ctl_open(&mut r, c.as_ptr(), flags)).map(|_| Ctl(r))
    }

    pub fn from_card(c: &Card, nonblock: bool) -> Result<Ctl> {
        let s = format!("hw:{}", c.get_index());
        Ctl::open(&CString::new(s).unwrap(), nonblock)
    }

    pub fn card_info(&self) -> Result<CardInfo> { CardInfo::new().and_then(|c|
        acheck!(snd_ctl_card_info(self.0, c.0)).map(|_| c)) }
}

impl Drop for Ctl {
    fn drop(&mut self) { unsafe { alsa::snd_ctl_close(self.0) }; }
}

pub fn ctl_ptr(a: &Ctl) -> *mut alsa::snd_ctl_t { a.0 }

/// [snd_ctl_card_info_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___control.html) wrapper
pub struct CardInfo(*mut alsa::snd_ctl_card_info_t);

impl Drop for CardInfo {
    fn drop(&mut self) { unsafe { alsa::snd_ctl_card_info_free(self.0) }}
}

impl CardInfo {
    fn new() -> Result<CardInfo> {
        let mut p = ptr::null_mut();
        acheck!(snd_ctl_card_info_malloc(&mut p)).map(|_| CardInfo(p))
    }

    pub fn get_id(&self) -> Result<&str> {
        from_const("snd_ctl_card_info_get_id", unsafe { alsa::snd_ctl_card_info_get_id(self.0) })}
    pub fn get_driver(&self) -> Result<&str> {
        from_const("snd_ctl_card_info_get_driver", unsafe { alsa::snd_ctl_card_info_get_driver(self.0) })}
    pub fn get_components(&self) -> Result<&str> {
        from_const("snd_ctl_card_info_get_components", unsafe { alsa::snd_ctl_card_info_get_components(self.0) })}
    pub fn get_longname(&self) -> Result<&str> {
        from_const("snd_ctl_card_info_get_longname", unsafe { alsa::snd_ctl_card_info_get_longname(self.0) })}
    pub fn get_name(&self) -> Result<&str> {
        from_const("snd_ctl_card_info_get_name", unsafe { alsa::snd_ctl_card_info_get_name(self.0) })}
    pub fn get_mixername(&self) -> Result<&str> {
        from_const("snd_ctl_card_info_get_mixername", unsafe { alsa::snd_ctl_card_info_get_mixername(self.0) })}
    pub fn get_card(&self) -> Card { Card::new(unsafe { alsa::snd_ctl_card_info_get_card(self.0) })}
}

/// [SND_CTL_ELEM_IFACE_xxx](http://www.alsa-project.org/alsa-doc/alsa-lib/group___control.html) constants
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ElemIface {
    Card = alsa::SND_CTL_ELEM_IFACE_CARD as isize,
    Hwdep = alsa::SND_CTL_ELEM_IFACE_HWDEP as isize,
    Mixer = alsa::SND_CTL_ELEM_IFACE_MIXER as isize,
    PCM = alsa::SND_CTL_ELEM_IFACE_PCM as isize,
    Rawmidi = alsa::SND_CTL_ELEM_IFACE_RAWMIDI as isize,
    Timer = alsa::SND_CTL_ELEM_IFACE_TIMER as isize,
    Sequencer = alsa::SND_CTL_ELEM_IFACE_SEQUENCER as isize,
}

/// [SND_CTL_ELEM_TYPE_xxx](http://www.alsa-project.org/alsa-doc/alsa-lib/group___control.html) constants
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ElemType {
    None = alsa::SND_CTL_ELEM_TYPE_NONE as isize,
    Boolean = alsa::SND_CTL_ELEM_TYPE_BOOLEAN as isize,
    Integer = alsa::SND_CTL_ELEM_TYPE_INTEGER as isize,
    Enumerated = alsa::SND_CTL_ELEM_TYPE_ENUMERATED as isize,
    Bytes = alsa::SND_CTL_ELEM_TYPE_BYTES as isize,
    IEC958 = alsa::SND_CTL_ELEM_TYPE_IEC958 as isize,
    Integer64 = alsa::SND_CTL_ELEM_TYPE_INTEGER64 as isize,
}

/// [snd_ctl_elem_value_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___control.html) wrapper
pub struct ElemValue {
    ptr: *mut alsa::snd_ctl_elem_value_t,
    etype: ElemType,
    count: u32,
}

impl Drop for ElemValue {
    fn drop(&mut self) { unsafe { alsa::snd_ctl_elem_value_free(self.ptr) }; }
}

pub fn elem_value_ptr(a: &ElemValue) -> *mut alsa::snd_ctl_elem_value_t { a.ptr }

pub fn elem_value_new(t: ElemType, count: u32) -> Result<ElemValue> {
    let mut p = ptr::null_mut();
    acheck!(snd_ctl_elem_value_malloc(&mut p))
        .map(|_| ElemValue { ptr: p, etype: t, count: count })
}

impl ElemValue {

    // Note: The get_bytes hands out a reference to inside the object. Therefore, we can't treat 
    // the content as "cell"ed, but must take a "&mut self" (to make sure the reference
    // from get_bytes has been dropped when calling a set_* function).

    pub fn get_boolean(&self, idx: u32) -> Option<bool> {
        if self.etype != ElemType::Boolean || idx >= self.count { None }
        else { Some( unsafe { alsa::snd_ctl_elem_value_get_boolean(self.ptr, idx as c_uint) } != 0) }
    }

    pub fn set_boolean(&mut self, idx: u32, val: bool) -> Option<()> {
        if self.etype != ElemType::Boolean || idx >= self.count { None }
        else { unsafe { alsa::snd_ctl_elem_value_set_boolean(self.ptr, idx as c_uint, if val {1} else {0}) }; Some(()) }
    }

    pub fn get_integer(&self, idx: u32) -> Option<i32> {
        if self.etype != ElemType::Integer || idx >= self.count { None }
        else { Some( unsafe { alsa::snd_ctl_elem_value_get_integer(self.ptr, idx as c_uint) } as i32) }
    }

    pub fn set_integer(&mut self, idx: u32, val: i32) -> Option<()> {
        if self.etype != ElemType::Integer || idx >= self.count { None }
        else { unsafe { alsa::snd_ctl_elem_value_set_integer(self.ptr, idx as c_uint, val as c_long) }; Some(()) }
    }

    pub fn get_integer64(&self, idx: u32) -> Option<i64> {
        if self.etype != ElemType::Integer64 || idx >= self.count { None }
        else { Some( unsafe { alsa::snd_ctl_elem_value_get_integer64(self.ptr, idx as c_uint) } as i64) }
    }

    pub fn set_integer64(&mut self, idx: u32, val: i64) -> Option<()> {
        if self.etype != ElemType::Integer || idx >= self.count { None }
        else { unsafe { alsa::snd_ctl_elem_value_set_integer64(self.ptr, idx as c_uint, val) }; Some(()) }
    }

    pub fn get_enumerated(&self, idx: u32) -> Option<u32> {
        if self.etype != ElemType::Enumerated || idx >= self.count { None }
        else { Some( unsafe { alsa::snd_ctl_elem_value_get_enumerated(self.ptr, idx as c_uint) } as u32) }
    }

    pub fn set_enumerated(&mut self, idx: u32, val: u32) -> Option<()> {
        if self.etype != ElemType::Enumerated || idx >= self.count { None }
        else { unsafe { alsa::snd_ctl_elem_value_set_enumerated(self.ptr, idx as c_uint, val as c_uint) }; Some(()) }
    }

    pub fn get_byte(&self, idx: u32) -> Option<u8> {
        if self.etype != ElemType::Bytes || idx >= self.count { None }
        else { Some( unsafe { alsa::snd_ctl_elem_value_get_byte(self.ptr, idx as c_uint) } as u8) }
    }

    pub fn set_byte(&mut self, idx: u32, val: u8) -> Option<()> {
        if self.etype != ElemType::Bytes || idx >= self.count { None }
        else { unsafe { alsa::snd_ctl_elem_value_set_byte(self.ptr, idx as c_uint, val) }; Some(()) }
    }

    pub fn get_bytes(&self) -> Option<&[u8]> {
        if self.etype != ElemType::Bytes { None }
        else { Some( unsafe { ::std::slice::from_raw_parts(
            alsa::snd_ctl_elem_value_get_bytes(self.ptr) as *const u8, self.count as usize) } ) }
    }

    pub fn set_bytes(&mut self, val: &[u8]) -> Option<()> {
        if self.etype != ElemType::Bytes || val.len() != self.count as usize { None }

        // Note: the alsa-lib function definition is broken. First, the pointer is declared as mut even 
        // though it's const, and second, there is a "value" missing between "elem" and "set_bytes".
        else { unsafe { alsa::snd_ctl_elem_set_bytes(self.ptr, val.as_ptr() as *mut c_void, val.len() as size_t) }; Some(()) }
    }

}

impl fmt::Debug for ElemValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ElemType::*;
        try!(write!(f, "ElemValue({:?}", self.etype));
        for a in 0..self.count { try!(match self.etype {
            Boolean => write!(f, ",{:?}", self.get_boolean(a).unwrap()),
            Integer => write!(f, ",{:?}", self.get_integer(a).unwrap()),
            Integer64 => write!(f, ",{:?}", self.get_integer64(a).unwrap()),
            Enumerated => write!(f, ",{:?}", self.get_enumerated(a).unwrap()),
            Bytes => write!(f, ",{:?}", self.get_byte(a).unwrap()),
            _ => Ok(()),
        })};
        write!(f, ")")
    }
}

/// [snd_ctl_elem_info_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___control.html) wrapper
pub struct ElemInfo(*mut alsa::snd_ctl_elem_info_t);

pub fn elem_info_ptr(a: &ElemInfo) -> *mut alsa::snd_ctl_elem_info_t { a.0 }

impl Drop for ElemInfo {
    fn drop(&mut self) { unsafe { alsa::snd_ctl_elem_info_free(self.0) }; }
}

pub fn elem_info_new() -> Result<ElemInfo> {
    let mut p = ptr::null_mut();
    acheck!(snd_ctl_elem_info_malloc(&mut p)).map(|_| ElemInfo(p))
}

impl ElemInfo {
    pub fn get_type(&self) -> ElemType { unsafe { mem::transmute(alsa::snd_ctl_elem_info_get_type(self.0) as u8) } }
    pub fn get_count(&self) -> u32 { unsafe { alsa::snd_ctl_elem_info_get_count(self.0) as u32 } }
}

//
// Non-allocating version of ElemId
//

/// [snd_ctl_elem_id_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___control.html) wrapper
pub struct ElemId([u8; ELEM_ID_SIZE]);

pub fn elem_id_new() -> Result<ElemId> {
    assert!(unsafe { alsa::snd_ctl_elem_id_sizeof() } as usize <= ELEM_ID_SIZE);
    Ok(ElemId(unsafe { mem::zeroed() }))
}

#[inline]
pub fn elem_id_ptr(a: &ElemId) -> *mut alsa::snd_ctl_elem_id_t { a.0.as_ptr() as *const _ as *mut alsa::snd_ctl_elem_id_t }

//
// Allocating version of ElemId
//

/*

/// [snd_ctl_elem_id_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___control.html) wrapper
pub struct ElemId(*mut alsa::snd_ctl_elem_id_t);

impl Drop for ElemId {
    fn drop(&mut self) { unsafe { alsa::snd_ctl_elem_id_free(self.0) }; }
}

pub fn elem_id_new() -> Result<ElemId> {
    let mut p = ptr::null_mut();
    acheck!(snd_ctl_elem_id_malloc(&mut p)).map(|_| ElemId(p))
}

pub fn elem_id_ptr(a: &ElemId) -> *mut alsa::snd_ctl_elem_id_t { a.0 }

*/

impl ElemId {
    pub fn get_name(&self) -> Result<&str> {
        from_const("snd_hctl_elem_id_get_name", unsafe { alsa::snd_ctl_elem_id_get_name(elem_id_ptr(&self)) })}
    pub fn get_device(&self) -> u32 { unsafe { alsa::snd_ctl_elem_id_get_device(elem_id_ptr(&self)) as u32 }}
    pub fn get_subdevice(&self) -> u32 { unsafe { alsa::snd_ctl_elem_id_get_subdevice(elem_id_ptr(&self)) as u32 }}
    pub fn get_numid(&self) -> u32 { unsafe { alsa::snd_ctl_elem_id_get_numid(elem_id_ptr(&self)) as u32 }}
    pub fn get_index(&self) -> u32 { unsafe { alsa::snd_ctl_elem_id_get_index(elem_id_ptr(&self)) as u32 }}
    pub fn get_interface(&self) -> ElemIface { unsafe { mem::transmute(alsa::snd_ctl_elem_id_get_interface(elem_id_ptr(&self)) as u8) }}
}

impl fmt::Debug for ElemId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let index = self.get_index();
        let device = self.get_device();
        let subdevice = self.get_subdevice();

        try!(write!(f, "ElemId(#{}, {:?}, {:?}", self.get_numid(), self.get_interface(), self.get_name()));
        if index > 0 { try!(write!(f, ", index={}", index)) };
        if device > 0 || subdevice > 0 { try!(write!(f, ", device={}", device)) };
        if subdevice > 0 { try!(write!(f, ", subdevice={}", device)) };
        write!(f, ")")
    }
}

#[test]
fn print_sizeof() {
    let elemid = unsafe { alsa::snd_ctl_elem_id_sizeof() } as usize;
    let elemvalue = unsafe { alsa::snd_ctl_elem_value_sizeof() } as usize;
    let eleminfo = unsafe { alsa::snd_ctl_elem_info_sizeof() } as usize;

    assert!(elemid >= ELEM_ID_SIZE);
//    assert!(elemvalue >= ELEM_VALUE_SIZE);
//    assert!(eleminfo >= ELEM_INFO_SIZE);

    println!("Elem id: {}, Elem value: {}, Elem info: {}", elemid, elemvalue, eleminfo);
}

