use std::{ptr, mem};
use std::ffi::CString;

use alsa;
use super::card;
use super::error::*;

const SELEM_ID_SIZE: usize = 64;

#[derive(Debug)]
pub struct Iter {
    handle: *mut alsa::snd_mixer_t,
    previous: Option<*mut alsa::snd_mixer_elem_t>
}

impl Iter {
    pub fn new(c: card::Card) -> Result<Iter> {
        let card = &CString::new(format!("hw:{}", c.get_index())).unwrap();
        let mut mixer_handle = ptr::null_mut();

        try!(acheck!(snd_mixer_open(&mut mixer_handle, 0)));
        try!(acheck!(snd_mixer_attach(mixer_handle, card.as_ptr())));
        try!(acheck!(snd_mixer_selem_register(mixer_handle, ptr::null_mut(), ptr::null_mut())));
        try!(acheck!(snd_mixer_load(mixer_handle)));

        Ok(Iter {
            handle: mixer_handle,
            previous: None
        })
    }
}

impl Drop for Iter {
    fn drop(&mut self) {
        unsafe { alsa::snd_mixer_close(self.handle) };
    }
}

impl Iterator for Iter {
    type Item = Result<Mixer>;

    fn next(&mut self) -> Option<Result<Mixer>> {
        let elem = Mixer::new(
            if self.previous.is_none() {
                unsafe { alsa::snd_mixer_first_elem(self.handle) }
            } else {
                unsafe { alsa::snd_mixer_elem_next(self.previous.unwrap()) }
            }
        );

        if elem.is_null() {
            None
        } else {
            self.previous = Some(elem.handle);
            Some(Ok(elem))
        }
    }
}

pub struct SelemId([u8; SELEM_ID_SIZE]);

pub fn selem_id_new() -> Result<SelemId> {
    assert!(unsafe { alsa::snd_mixer_selem_id_sizeof() } as usize <= SELEM_ID_SIZE);
    Ok(SelemId(unsafe { mem::zeroed() }))
}

#[inline]
pub fn selem_id_ptr(a: &SelemId) -> *mut alsa::snd_mixer_selem_id_t {
    a.0.as_ptr() as *const _ as *mut alsa::snd_mixer_selem_id_t
}

pub struct Mixer {
    handle: *mut alsa::snd_mixer_elem_t,
    // selem_id: *mut alsa::snd_mixer_selem_id_t
    selem_id: SelemId
}

impl Mixer {
    pub fn new(mixer_handle: *mut alsa::snd_mixer_elem_t) -> Mixer {
        let sid = selem_id_new().unwrap();

        if mixer_handle != 0 as *mut alsa::snd_mixer_elem_t {
            unsafe { alsa::snd_mixer_selem_get_id(mixer_handle, selem_id_ptr(&sid)) };
        }

        Mixer {
            handle: mixer_handle,
            selem_id: sid
        }
    }

    pub fn is_null(&self) -> bool {
        self.handle == 0 as *mut alsa::snd_mixer_elem_t
    }

    pub fn get_name(&self) -> Result<String> {
        let c = unsafe { alsa::snd_mixer_selem_id_get_name(selem_id_ptr(&self.selem_id)) };
        from_const("snd_mixer_selem_id_get_name", c).and_then(|s| Ok(s.to_string()))
    }
}

#[test]
fn print_mixer_of_card_0() {
    for card in card::Iter::new().map(|c| c.unwrap()) {
        println!("Card #{}: {} ({})", card.get_index(), card.get_name().unwrap(), card.get_longname().unwrap());
        for mixer in Iter::new(card::Card::new(card.get_index())).unwrap().map(|m| m.unwrap()) {
            assert!(mixer.is_null() == false );
            println!("\tMixer {}", mixer.get_name().unwrap());
        }
    }
}
