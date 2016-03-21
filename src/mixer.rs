//! Mixer API - Simple Mixer API for mixer control
//!
use std::ffi::CString;
use std::{ptr, mem};
use std::ops::Deref;

use alsa;
use super::error::*;

const SELEM_ID_SIZE: usize = 64;

/// wraps [snd_mixer_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___mixer.html)
pub struct Mixer(*mut alsa::snd_mixer_t);

impl Mixer {
    /// Opens a mixer and attaches it to a card identified by its name (like hw:0) and loads the
    /// mixer after registering a Selem.
    pub fn new(name: &str) -> Result<Mixer> {
        let mut mixer = Mixer(ptr::null_mut());
        try!(mixer.open());
        try!(mixer.attach(name));
        try!(Selem::register(&mixer));
        try!(mixer.load());
        Ok(mixer)
    }

    /// Creates a Selem by looking for a specific selem by name given a mixer (of a card)
    pub fn find_selem(&self, name: &str) -> Result<Selem> {
        let sid = SelemId::empty();
        sid.set_index(0);
        sid.set_name(name);
        let selem = unsafe { alsa::snd_mixer_find_selem(self.0, sid.as_ptr()) };

        if selem == 0 as *mut alsa::snd_mixer_elem_t {
            Err(Error::new(Some("snd_mixer_find_selem".into()), -1 as ::libc::c_int))
        } else {
            Ok(Selem::new(Elem {handle: selem, mixer: self}))
        }
    }

    pub fn open(&mut self) -> Result<i32> {
        acheck!(snd_mixer_open(&mut self.0, 0))
    }

    pub fn attach(&self, name: &str) -> Result<i32> {
        let card = &CString::new(name).unwrap();
        acheck!(snd_mixer_attach(self.0, card.as_ptr()))
    }

    pub fn load(&self) -> Result<i32> {
        acheck!(snd_mixer_load(self.0))
    }

    pub fn iter(&self) -> Iter {
        Iter {
            last_handle: ptr::null_mut(),
            mixer: self
        }
    }
}

/// Closes mixer and frees used resources
impl Drop for Mixer {
    fn drop(&mut self) {
        unsafe { alsa::snd_mixer_close(self.0) };
    }
}

/// Wraps [snd_mixer_elem_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___mixer.html)
#[derive(Copy, Clone)]
pub struct Elem<'a>{
    handle: *mut alsa::snd_mixer_elem_t,
    mixer: &'a Mixer
}

/// Iterator for all elements of mixer
#[derive(Copy, Clone)]
pub struct Iter<'a>{
    last_handle: *mut alsa::snd_mixer_elem_t,
    mixer: &'a Mixer
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

/// Wrapper for `snd_mixer_selem_id_t`, using array of hardcoded length
// #[derive(Copy, Clone)]
pub struct SelemId([u8; SELEM_ID_SIZE]);

impl SelemId {
    /// Creates a new SelemId` of hardcoded size SELEM_ID_SIZE.
    /// This size is checked against `snd_mixer_selem_id_sizeof`
    pub fn get_id(elem: Elem) -> SelemId {
        // Create empty selem_id and fill from mixer
        let sid = SelemId::empty();
        unsafe { alsa::snd_mixer_selem_get_id(elem.handle, sid.as_ptr()) };
        sid
    }

    /// Returns an empty (zeroed) SelemId. This id is not a useable id and need to be initialized
    /// like `SelemId::new()` does
    pub fn empty() -> SelemId {
        assert!(unsafe { alsa::snd_mixer_selem_id_sizeof() } as usize <= SELEM_ID_SIZE);
        // Create empty selem_id and fill from mixer
        SelemId(unsafe { mem::zeroed() })
    }

    /// Convert SelemId into ``*mut *mut snd_mixer_selem_id_t` that the alsa call needs.
    /// See [snd_mixer_selem_id_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___simple_mixer.html)
    #[inline]
    fn as_ptr(&self) -> *mut alsa::snd_mixer_selem_id_t {
        self.0.as_ptr() as *const _ as *mut alsa::snd_mixer_selem_id_t
    }

    pub fn get_name(&self) -> Result<String> {
        let c = unsafe { alsa::snd_mixer_selem_id_get_name(self.as_ptr()) };
        from_const("snd_mixer_selem_id_get_name", c).and_then(|s| Ok(s.to_string()))
    }

    pub fn get_index(&self) -> u32 {
        unsafe { alsa::snd_mixer_selem_id_get_index(self.as_ptr()) }
    }

    pub fn set_name(&self, name: &str) {
        unsafe { alsa::snd_mixer_selem_id_set_name(self.as_ptr(), CString::new(name).unwrap().as_ptr()) };
    }

    pub fn set_index(&self, index: u32) {
        unsafe { alsa::snd_mixer_selem_id_set_index(self.as_ptr(), index) };
    }

}

/// Wraps an Elem as a Selem
// #[derive(Copy, Clone)]
pub struct Selem<'a>(SelemId, Elem<'a>);

impl<'a> Selem<'a> {
    /// Creates a Selem by wrapping `elem`.
    pub fn new(elem: Elem<'a>) -> Selem<'a> {
        Selem(SelemId::get_id(elem), elem)
    }

    pub fn register(mixer: &Mixer) -> Result<i32> {
        acheck!(snd_mixer_selem_register(mixer.0, ptr::null_mut(), ptr::null_mut()))
    }

    pub fn get_id(&'a self) -> &'a SelemId {
        &self.0
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

    pub fn is_playback_mono(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_is_playback_mono(self.1.handle) == 1 }
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

impl<'a> Deref for Selem<'a> {
    type Target = Elem<'a>;

    /// returns the elem of this selem
    fn deref(&self) -> &Elem<'a> {
        &self.1
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
}

impl SelemChannelId {
    pub fn mono() -> isize {
        alsa::SND_MIXER_SCHN_MONO as isize
    }
}

#[test]
fn print_mixer_of_cards() {
    use super::card;

    for card in card::Iter::new().map(|c| c.unwrap()) {
        println!("Card #{}: {} ({})", card.get_index(), card.get_name().unwrap(), card.get_longname().unwrap());

        let mixer = Mixer::new(format!("hw:{}", card.get_index()).as_str()).unwrap();
        for elem in mixer.iter() {

            let selem = Selem::new(elem);
            println!("\tMixer element {}:{}", selem.get_id().get_index(), selem.get_id().get_name().unwrap());

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
                if selem.is_playback_mono() {
                    print!("Mono");
                } else {
                    for channel in 0..SelemChannelId::Last as i32 {
                        if selem.has_playback_channel(channel) { print!("{}:{} ", channel, selem.channel_name(channel).unwrap()) };
                    }
                }
                println!("");
                if selem.has_playback_volume() {
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
}

#[test]
fn get_and_set_playback_volume() {
    let mixer = Mixer::new("hw:2").unwrap();
    let selem = mixer.find_selem("Speaker").unwrap();

    let range: [i64;2] = selem.get_playback_volume_range();
    let mut channel: i32 = 0;
    for c in 0..SelemChannelId::Last as i32 {
        if selem.has_playback_channel(c) { channel = c; break }
    }
    println!("Testing on {} with limits {}-{} on channel {}", selem.get_id().get_name().unwrap(), range[0], range[1], channel);

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
    let mixer = Mixer::new("hw:2").unwrap();
    let selem = mixer.find_selem("Mic").unwrap();

    let range: [i64;2] = selem.get_capture_volume_range();
    let mut channel: i32 = 0;
    for c in 0..SelemChannelId::Last as i32 {
        if selem.has_capture_channel(c) { channel = c; break }
    }
    println!("Testing on {} with limits {}-{} on channel {}", selem.get_id().get_name().unwrap(), range[0], range[1], channel);

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
