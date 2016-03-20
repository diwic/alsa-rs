//! Mixer API - Simple Mixer API for mixer control
//!
use std::ffi::CString;
use std::{ptr, mem};

use alsa;
use super::card;
use super::error::*;

const SELEM_ID_SIZE: usize = 64;

/// wraps [snd_mixer_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___mixer.html)
pub struct Mixer(*mut alsa::snd_mixer_t);

impl Mixer {
    /// Creates a new iterator for a specific card using the cards index and
    /// using hw name, i.e. `hw:0`
    pub fn new(c: &card::Card) -> Result<Mixer> {
        let card = &CString::new(format!("hw:{}", c.get_index())).unwrap();
        let mut mixer_handle = ptr::null_mut();
        try!(acheck!(snd_mixer_open(&mut mixer_handle, 0)));
        try!(acheck!(snd_mixer_attach(mixer_handle, card.as_ptr())));
        try!(acheck!(snd_mixer_selem_register(mixer_handle, ptr::null_mut(), ptr::null_mut())));
        try!(acheck!(snd_mixer_load(mixer_handle)));
        Ok(Mixer(mixer_handle))
    }
}

/// Closes mixer and frees used resources
impl Drop for Mixer {
    fn drop(&mut self) {
        unsafe { alsa::snd_mixer_close(self.0) };
    }
}

#[derive(Copy, Clone)]
pub struct Elem<'a>{
    handle: *mut alsa::snd_mixer_elem_t,
    mixer: &'a Mixer
}

impl<'a> Elem<'a> {
    /// Checks if this element is null by checking if the handle is null
    pub fn is_null(&self) -> bool {
        self.handle == 0 as *mut alsa::snd_mixer_elem_t
    }
}

#[derive(Copy, Clone)]
pub struct Iter<'a>{
    last_handle: *mut alsa::snd_mixer_elem_t,
    mixer: &'a Mixer
}

impl<'a> Iter<'a> {
    pub fn new(m: &'a Mixer) -> Iter<'a> {
        Iter {
            last_handle: ptr::null_mut(),
            mixer: m
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Elem<'a>;

    fn next(&mut self) -> Option<Elem<'a>> {
        let elem = if self.last_handle.is_null() {
            unsafe { alsa::snd_mixer_first_elem(self.mixer.0) }
        } else {
            unsafe { alsa::snd_mixer_elem_next(self.last_handle) }
        };

        if elem.is_null() {
            None
        } else {
            self.last_handle = elem;
            Some(Elem { handle: elem, mixer: self.mixer})
        }
    }

}

// #[derive(Copy, Clone)]
pub struct SelemId([u8; SELEM_ID_SIZE]);

impl SelemId {
    /// Creates a new SelemId` of hardcoded size SELEM_ID_SIZE.
    /// This size is checked against `snd_mixer_selem_id_sizeof`
    pub fn new(elem: Elem) -> SelemId {
        // Create empty selem_id and fill from mixer
        let sid = SelemId::empty();
        unsafe { alsa::snd_mixer_selem_get_id(elem.handle, sid.as_ptr()) };
        sid
    }

    pub fn empty() -> SelemId {
        assert!(unsafe { alsa::snd_mixer_selem_id_sizeof() } as usize <= SELEM_ID_SIZE);
        // Create empty selem_id and fill from mixer
        SelemId(unsafe { mem::zeroed() })
    }

    /// Convert SelemId into ``*mut *mut snd_mixer_selem_id_t` that the alsa call needs.
    /// See [snd_mixer_selem_id_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___simple_mixer.html)
    #[inline]
    pub fn as_ptr(&self) -> *mut alsa::snd_mixer_selem_id_t {
        self.0.as_ptr() as *const _ as *mut alsa::snd_mixer_selem_id_t
    }
}

// #[derive(Copy, Clone)]
pub struct Selem<'a>(SelemId, Elem<'a>);

impl<'a> Selem<'a> {
    pub fn new(elem: Elem<'a>) -> Selem<'a> {
        Selem(SelemId::new(elem), elem)
    }

    pub fn find_by_name(mixer: &'a Mixer, name: &str) -> Result<Selem<'a>> {
        let sid = SelemId::empty();
        unsafe { alsa::snd_mixer_selem_id_set_index(sid.as_ptr(), 0) };
        unsafe { alsa::snd_mixer_selem_id_set_name(sid.as_ptr(), CString::new(name).unwrap().as_ptr()) };
        let elem = Elem { handle: unsafe { alsa::snd_mixer_find_selem(mixer.0, sid.as_ptr()) }, mixer: mixer};

        if elem.is_null() {
            Err(Error::new(Some(stringify!("snd_mixer_find_selem").into()), -1 as ::libc::c_int))
        } else {
            Ok(Selem::new(elem))
        }
    }


    pub fn get_elem(&self) -> Elem<'a> {
        self.1
    }

    pub fn get_name(&self) -> Result<String> {
        let c = unsafe { alsa::snd_mixer_selem_id_get_name(self.0.as_ptr()) };
        from_const("snd_mixer_selem_id_get_name", c).and_then(|s| Ok(s.to_string()))
    }

    pub fn get_index(&self) -> u32 {
        unsafe { alsa::snd_mixer_selem_id_get_index(self.0.as_ptr()) }
    }

    pub fn has_capture_volume(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_has_capture_volume(self.1.handle) > 0 }
    }

    pub fn has_capture_switch(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_has_capture_switch(self.1.handle) > 0 }
    }

    pub fn has_playback_volume(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_has_playback_volume(self.1.handle) > 0 }
    }

    pub fn has_playback_switch(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_has_playback_switch(self.1.handle) > 0 }
    }

    pub fn can_capture(&self) -> bool {
        self.has_capture_volume() || self.has_capture_switch()
    }

    pub fn can_playback(&self) -> bool {
        self.has_playback_volume() || self.has_playback_switch()
    }

    pub fn has_volume(&self) -> bool {
        self.has_capture_volume() || self.has_playback_volume()
    }

    /// returns range for capture volume in an array of [min,max] values
    pub fn get_capture_volume_range(&self) -> [i64;2] {
        let mut min: i64 = 0;
        let mut max: i64 = 0;
        unsafe { alsa::snd_mixer_selem_get_capture_volume_range(self.1.handle, &mut min, &mut max) };
        [min, max]
    }

    /// returns array of [min,max] values in decibels*100. To get correct dB value, devide by 100, i.e.
    ///
    /// # Example
    /// ```ignore
    /// let db_value = selem.capture_decibel_range() as f32 / 100.0;
    /// ```
    pub fn get_capture_decibel_range(&self) -> [i64;2] {
        let mut min: i64 = 0;
        let mut max: i64 = 0;
        unsafe { alsa::snd_mixer_selem_get_capture_dB_range(self.1.handle, &mut min, &mut max) };
        [min, max]
    }

    /// returns array of [min,max] values
    pub fn get_playback_volume_range(&self) -> [i64;2] {
        let mut min: i64 = 0;
        let mut max: i64 = 0;
        unsafe { alsa::snd_mixer_selem_get_playback_volume_range(self.1.handle, &mut min, &mut max) };
        [min, max]
    }

    /// returns array of [min,max] values in decibels*100. To get correct dB value, devide by 100, i.e.
    ///
    /// # Example
    /// ```ignore
    /// let db_value = selem.playback_decibel_range() as f32 / 100.0;
    /// ```
    pub fn get_playback_decibel_range(&self) -> [i64;2] {
        let mut min: i64 = 0;
        let mut max: i64 = 0;
        unsafe { alsa::snd_mixer_selem_get_playback_dB_range(self.1.handle, &mut min, &mut max) };
        [min, max]
    }

    pub fn has_capture_channel(&self, channel: i32) -> bool {
        unsafe { alsa::snd_mixer_selem_has_capture_channel(self.1.handle, channel) > 0 }
    }

    pub fn has_playback_channel(&self, channel: i32) -> bool {
        unsafe { alsa::snd_mixer_selem_has_playback_channel(self.1.handle, channel) > 0 }
    }

    /// Gets name from snd_mixer_selem_channel_name
    pub fn channel_name(&self, channel: i32) -> Result<String> {
        let c = unsafe { alsa::snd_mixer_selem_channel_name(channel) };
        from_const("snd_mixer_selem_channel_name", c).and_then(|s| Ok(s.to_string()))
    }

    pub fn get_playback_volume(&self, channel: i32) -> Result<i64> {
        let mut value: i64 = 0;
        acheck!(snd_mixer_selem_get_playback_volume(self.1.handle, channel, &mut value)).and_then(|_| Ok(value))
    }

    /// returns volume in decibels*100. To get correct dB value, devide by 100
    ///
    /// # Example
    /// ```ignore
    /// let db_value = selem.playback_volume_decibel(SelemChannelId::FrontLeft as i32).unwrap() as f32 / 100.0;
    /// ```
    pub fn ask_playback_vol_decibel(&self, channel: i32) -> Result<i64> {
        let mut decibel_value: i64 = 0;
        self.get_playback_volume(channel)
            .and_then(|volume| acheck!(snd_mixer_selem_ask_playback_vol_dB (self.1.handle, volume, &mut decibel_value)))
            .and_then(|_| Ok(decibel_value))
    }

    pub fn get_capture_volume(&self, channel: i32) -> Result<i64> {
        let mut value: i64 = 0;
        acheck!(snd_mixer_selem_get_capture_volume(self.1.handle, channel, &mut value)).and_then(|_| Ok(value))
    }

    /// returns volume in decibels*100. To get correct dB value, devide by 100, i.e.
    ///
    /// # Example
    /// ```ignore
    /// let db_value = selem.capture_volume_decibel(SelemChannelId::FrontLeft as i32).unwrap() as f32 / 100.0;
    /// ```
    pub fn ask_capture_vol_decibel(&self, channel: i32) -> Result<i64> {
        let mut decibel_value: i64 = 0;
        self.get_capture_volume(channel)
            .and_then(|volume| acheck!(snd_mixer_selem_ask_capture_vol_dB (self.1.handle, volume, &mut decibel_value)))
            .and_then(|_| Ok(decibel_value))
    }

    pub fn set_playback_volume(&self, channel: i32, value: i64) -> Result<i32> {
        acheck!(snd_mixer_selem_set_playback_volume(self.1.handle, channel, value))
    }

    pub fn set_capture_volume(&self, channel: i32, value: i64) -> Result<i32> {
        acheck!(snd_mixer_selem_set_capture_volume(self.1.handle, channel, value))
    }
}


#[derive(Copy, Clone)]
pub enum SelemChannelId {
    Unknown     = alsa::SND_MIXER_SCHN_UNKNOWN as isize,
    FrontLeft   = alsa::SND_MIXER_SCHN_FRONT_LEFT as isize,
    FrontRight  = alsa::SND_MIXER_SCHN_FRONT_RIGHT as isize,
    RearLeft    = alsa::SND_MIXER_SCHN_REAR_LEFT as isize,
    RearRight   = alsa::SND_MIXER_SCHN_REAR_RIGHT as isize,
    FrontCenter = alsa::SND_MIXER_SCHN_FRONT_CENTER as isize,
    Woofer      = alsa::SND_MIXER_SCHN_WOOFER as isize,
    SideLeft    = alsa::SND_MIXER_SCHN_SIDE_LEFT as isize,
    SideRight   = alsa::SND_MIXER_SCHN_SIDE_RIGHT as isize,
    RearCenter  = alsa::SND_MIXER_SCHN_REAR_CENTER as isize,
    Last        = alsa::SND_MIXER_SCHN_LAST as isize,
    // Mono        = alsa::SND_MIXER_SCHN_MONO as isize,
}

#[test]
fn print_mixer_of_cards() {
    for card in card::Iter::new().map(|c| c.unwrap()) {
        println!("Card #{}: {} ({})", card.get_index(), card.get_name().unwrap(), card.get_longname().unwrap());

        let mixer = Mixer::new(&card).unwrap();
        for elem in Iter::new(&mixer) {

            assert!(elem.is_null() == false );
            let selem = Selem::new(elem);
            println!("\tMixer element {}:{}", selem.get_index(), selem.get_name().unwrap());

            if selem.has_volume() {
                print!("\t  Volume limits: ");
                if selem.has_capture_volume() {
                    print!("Capture={}-{} ", selem.get_capture_volume_range()[0], selem.get_capture_volume_range()[1] );
                    print!("/{}dB-{}dB ", selem.get_capture_decibel_range()[0] as f32 / 100.0, selem.get_capture_decibel_range()[1] as f32 / 100.0 );
                }
                if selem.has_playback_volume() {
                    print!("Playback={}-{} ", selem.get_playback_volume_range()[0],selem.get_playback_volume_range()[1]);
                    print!("/{}dB-{}dB ", selem.get_playback_decibel_range()[0] as f32 / 100.0, selem.get_playback_decibel_range()[1] as f32 / 100.0);
                }
                println!("");
            }

            if selem.can_capture() {
                print!("\t  Capture channels: ");
                for channel in 0..SelemChannelId::Last as i32 {
                    if selem.has_capture_channel(channel) { print!("{}:{} ", channel, selem.channel_name(channel).unwrap()) };
                }
                println!("");
                print!("\t  Capture volumes: ");
                for channel in 0..SelemChannelId::Last as i32 {
                    if selem.has_capture_channel(channel) { print!("{}:{}/{}dB ", channel,
                        match selem.get_capture_volume(channel) {Ok(v) => format!("{}",v).to_string(), Err(_) => "n/a".to_string()},
                        match selem.ask_capture_vol_decibel(channel) {Ok(v) => format!("{}",v as f32 /100.0).to_string(), Err(_) => "n/a".to_string()}
                    );}
                }
                println!("");
            }

            if selem.can_playback() {
                print!("\t  Playback channels: ");
                for channel in 0..SelemChannelId::Last as i32 {
                    if selem.has_playback_channel(channel) { print!("{}:{} ", channel, selem.channel_name(channel).unwrap()) };
                }
                println!("");
                print!("\t  Playback volumes: ");
                for channel in 0..SelemChannelId::Last as i32 {
                    if selem.has_playback_channel(channel) { print!("{}:{}/{}dB ",
                        channel,
                        match selem.get_playback_volume(channel) {Ok(v) => format!("{}",v).to_string(), Err(_) => "n/a".to_string()},
                        match selem.ask_playback_vol_decibel(channel) {Ok(v) => format!("{}",(v as f32) / 100.0).to_string(), Err(_) => "n/a".to_string()}
                    );}
                }
                println!("");
            }
        }
    }
}

#[test]
fn get_and_set_playback_volume() {
    let card = card::Card::new(2);
    let mixer = Mixer::new(&card).unwrap();
    let selem = Selem::find_by_name(&mixer, "Speaker").unwrap();
    assert!(!selem.get_elem().is_null());

    let range: [i64;2] = selem.get_playback_volume_range();
    let mut channel: i32 = 0;
    for c in 0..SelemChannelId::Last as i32 {
        if selem.has_playback_channel(c) { channel = c; break }
    }
    println!("Testing on {} with limits {}-{} on channel {}", selem.get_name().unwrap(), range[0], range[1], channel);

    let old: i64 = selem.get_playback_volume(channel).unwrap();
    let new: i64 = range[1] / 2;
    assert!( new != old );

    println!("Changing volume of {} from {} to {}", channel, old, new);
    selem.set_playback_volume(channel, new).unwrap();
    let mut result: i64 = selem.get_playback_volume(channel).unwrap();
    assert_eq!(new, result);

    // return volume to old value
    selem.set_playback_volume(channel, old).unwrap();
    result = selem.get_playback_volume(channel).unwrap();
    assert_eq!(old, result);
}

#[test]
fn get_and_set_capture_volume() {
    let card = card::Card::new(2);
    let mixer = Mixer::new(&card).unwrap();
    let selem = Selem::find_by_name(&mixer, "Mic").unwrap();
    assert!(!selem.get_elem().is_null());

    let range: [i64;2] = selem.get_capture_volume_range();
    let mut channel: i32 = 0;
    for c in 0..SelemChannelId::Last as i32 {
        if selem.has_capture_channel(c) { channel = c; break }
    }
    println!("Testing on {} with limits {}-{} on channel {}", selem.get_name().unwrap(), range[0], range[1], channel);

    let old: i64 = selem.get_capture_volume(channel).unwrap();
    let new: i64 = range[1] / 2;
    assert!( new != old );

    println!("Changing volume of {} from {} to {}", channel, old, new);
    selem.set_capture_volume(channel, new).unwrap();
    let mut result: i64 = selem.get_capture_volume(channel).unwrap();
    assert_eq!(new, result);

    // return volume to old value
    selem.set_capture_volume(channel, old).unwrap();
    result = selem.get_capture_volume(channel).unwrap();
    assert_eq!(old, result);
}
