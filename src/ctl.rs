//! Control device API

use alsa;
use std::ffi::{CStr, CString};
use super::error::*;
use std::ptr;
use super::Card;

/// [snd_ctl_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___control.html) wrapper
pub struct Ctl(*mut alsa::snd_ctl_t);

impl Ctl {
    /// Open does not support async mode (it's not very Rustic anyway)
    pub fn open(c: &CStr, nonblock: bool) -> Result<Ctl> {
        let mut r = ptr::null_mut();
        let flags = if nonblock { 1 } else { 0 }; // FIXME: alsa::SND_CTL_NONBLOCK does not exist in alsa-sys
        check("snd_ctl_open", unsafe { alsa::snd_ctl_open(&mut r, c.as_ptr(), flags) })
            .map(|_| Ctl(r))
    }

    pub fn from_card(c: &Card, nonblock: bool) -> Result<Ctl> {
        let s = format!("hw:{}", c.get_index());
        Ctl::open(&CString::new(s).unwrap(), nonblock)
    }

    pub unsafe fn handle(&self) -> *mut alsa::snd_ctl_t { self.0 }

    pub fn card_info(&self) -> Result<CardInfo> { CardInfo::new().and_then(|c|
        check("snd_ctl_card_info", unsafe { alsa::snd_ctl_card_info(self.0, c.0) }).map(|_| c)) }
}

impl Drop for Ctl {
    fn drop(&mut self) { unsafe { alsa::snd_ctl_close(self.0) }; }
}

/// [snd_ctl_card_info_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___control.html) wrapper
pub struct CardInfo(*mut alsa::snd_ctl_card_info_t);

impl Drop for CardInfo {
    fn drop(&mut self) { unsafe { alsa::snd_ctl_card_info_free(self.0) }}
}

impl CardInfo {
    fn new() -> Result<CardInfo> {
        let mut p = ptr::null_mut();
        check("snd_ctl_card_info_malloc", unsafe { alsa::snd_ctl_card_info_malloc(&mut p) }).map(|_| CardInfo(p))
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

