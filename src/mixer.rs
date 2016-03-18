//! Mixer API - Simple Mixer API for mixer control
//!
use std::{ptr, mem};
use std::ffi::CString;

use alsa;
use super::card;
use super::error::*;

const SELEM_ID_SIZE: usize = 64;
const SND_MIXER_SCHN_LAST: i32 = 31;

/// Iterator over available mixers of a card. This Iter wraps
/// [snd_mixer_elem_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___mixer.html) and
/// uses [snd_mixer_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___mixer.html)
///
/// # Example
/// ```ignore
/// use alsa::card;
/// use alsa:mixer;
/// let card = card::Card::new(0);  // use soundcard 0
/// for mixer in mixer::Iter::new(card).unwrap().map(|m| m.unwrap()) {
///   println!("Card #{}: {} ({})", card.get_index(), card.get_name().unwrap(), card.get_longname().unwrap());
/// }
/// ```
#[derive(Debug)]
pub struct Iter {
    handle: *mut alsa::snd_mixer_t,
    previous: Option<*mut alsa::snd_mixer_elem_t>
}

impl Iter {
    /// Creates a new iterator for a specific card using the cards using hw name, i.e. `hw:0`
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

/// Closes mixer and frees used alsa resources
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
            // remember last used handle to use it for snd_mixer_elem_next at next call of next()
            self.previous = Some(elem.handle);
            Some(Ok(elem))
        }
    }
}

/// use fixed size structure for selem_id
pub struct SelemId([u8; SELEM_ID_SIZE]);

/// Creates a new `selem_id` using `SelemId` of hardcoded size. This size is checked with `snd_mixer_selem_id_sizeof`
pub fn selem_id_new() -> Result<SelemId> {
    assert!(unsafe { alsa::snd_mixer_selem_id_sizeof() } as usize <= SELEM_ID_SIZE);
    Ok(SelemId(unsafe { mem::zeroed() }))
}

/// Convert SelemId into ``*mut *mut snd_mixer_selem_id_t` that the alsa call needs. See [snd_mixer_selem_id_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___simple_mixer.html)
#[inline]
pub fn selem_id_ptr(a: &SelemId) -> *mut alsa::snd_mixer_selem_id_t {
    a.0.as_ptr() as *const _ as *mut alsa::snd_mixer_selem_id_t
}

/// Mixer wraps [snd_mixer_selem_id_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___simple_mixer.html)
pub struct Mixer {
    handle: *mut alsa::snd_mixer_elem_t,
    selem_id: SelemId
}

impl Mixer {
    pub fn new(mixer_handle: *mut alsa::snd_mixer_elem_t) -> Mixer {
        let sid = selem_id_new().unwrap();

        // check for null pointer
        if mixer_handle != 0 as *mut alsa::snd_mixer_elem_t {
            unsafe { alsa::snd_mixer_selem_get_id(mixer_handle, selem_id_ptr(&sid)) };
        }

        Mixer {
            handle: mixer_handle,
            selem_id: sid
        }
    }

    /// Checks if this mixer is null by checking if the handle is null
    pub fn is_null(&self) -> bool {
        self.handle == 0 as *mut alsa::snd_mixer_elem_t
    }

    pub fn get_name(&self) -> Result<String> {
        let c = unsafe { alsa::snd_mixer_selem_id_get_name(selem_id_ptr(&self.selem_id)) };
        from_const("snd_mixer_selem_id_get_name", c).and_then(|s| Ok(s.to_string()))
    }

    pub fn get_index(&self) -> u32 {
        unsafe { alsa::snd_mixer_selem_id_get_index(selem_id_ptr(&self.selem_id)) }
    }

    pub fn has_capture_volume(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_has_capture_volume(self.handle) > 0 }
    }

    pub fn has_capture_switch(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_has_capture_switch(self.handle) > 0 }
    }

    pub fn has_playback_volume(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_has_playback_volume(self.handle) > 0 }
    }

    pub fn has_playback_switch(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_has_playback_switch(self.handle) > 0 }
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

    /// returns array of [min,max] values
    pub fn capture_volume_range(&self) -> [i64;2] {
        let mut min: i64 = 0;
        let mut max: i64 = 0;
        unsafe { alsa::snd_mixer_selem_get_capture_volume_range(self.handle, &mut min, &mut max) };
        [min, max]
    }

    /// returns array of [min,max] values in decibels*100. To get correct dB value, devide by 100, i.e.
    ///
    /// # Example
    /// ```ignore
    /// let db_value = mixer.capture_decibel_range() as f32 / 100.0;
    /// ```
    pub fn capture_decibel_range(&self) -> [i64;2] {
        let mut min: i64 = 0;
        let mut max: i64 = 0;
        unsafe { alsa::snd_mixer_selem_get_capture_dB_range(self.handle, &mut min, &mut max) };
        [min, max]
    }

    /// returns array of [min,max] values
    pub fn playback_volume_range(&self) -> [i64;2] {
        let mut min: i64 = 0;
        let mut max: i64 = 0;
        unsafe { alsa::snd_mixer_selem_get_playback_volume_range(self.handle, &mut min, &mut max) };
        [min, max]
    }

    /// returns array of [min,max] values in decibels*100. To get correct dB value, devide by 100, i.e.
    ///
    /// # Example
    /// ```ignore
    /// let db_value = mixer.playback_decibel_range() as f32 / 100.0;
    /// ```
    pub fn playback_decibel_range(&self) -> [i64;2] {
        let mut min: i64 = 0;
        let mut max: i64 = 0;
        unsafe { alsa::snd_mixer_selem_get_playback_dB_range(self.handle, &mut min, &mut max) };
        [min, max]
    }
}

/// Represents a channel of a mixer
pub struct MixerChannel<'a> {
    mixer: &'a Mixer,
    channel: i32
}

impl<'a> MixerChannel<'a> {
    pub fn new(m: &'a Mixer, ch: i32) -> MixerChannel {
        MixerChannel {
            mixer: m,
            channel: ch
        }
    }

    pub fn can_capture(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_has_capture_channel(self.mixer.handle, self.channel) > 0 }
    }

    pub fn can_playback(&self) -> bool {
        unsafe { alsa::snd_mixer_selem_has_playback_channel(self.mixer.handle, self.channel) > 0 }
    }

    /// Gets name from snd_mixer_selem_channel_name
    pub fn channel_name(&self) -> Result<String> {
        let c = unsafe { alsa::snd_mixer_selem_channel_name(self.channel) };
        from_const("snd_mixer_selem_channel_name", c).and_then(|s| Ok(s.to_string()))
    }

    pub fn playback_volume(&self) -> Result<i64> {
        let mut value: i64 = 0;
        acheck!(snd_mixer_selem_get_playback_volume(self.mixer.handle, self.channel, &mut value)).and_then(|_| Ok(value))
    }

    /// returns volume in decibels*100. To get correct dB value, devide by 100
    ///
    /// # Example
    /// ```ignore
    /// let db_value = channel.playback_volume_decibel().unwrap() as f32 / 100.0;
    /// ```
    pub fn playback_volume_decibel(&self) -> Result<i64> {
        let mut decibel_value: i64 = 0;
        self.playback_volume()
            .and_then(|volume| acheck!(snd_mixer_selem_ask_playback_vol_dB (self.mixer.handle, volume, &mut decibel_value)))
            .and_then(|_| Ok(decibel_value))
    }

    pub fn capture_volume(&self) -> Result<i64> {
        let mut value: i64 = 0;
        acheck!(snd_mixer_selem_get_capture_volume(self.mixer.handle, self.channel, &mut value)).and_then(|_| Ok(value))
    }

    /// returns volume in decibels*100. To get correct dB value, devide by 100, i.e.
    ///
    /// # Example
    /// ```ignore
    /// let db_value = channel.capture_volume_decibel().unwrap() as f32 / 100.0;
    /// ```
    pub fn capture_volume_decibel(&self) -> Result<i64> {
        let mut decibel_value: i64 = 0;
        self.capture_volume()
            .and_then(|volume| acheck!(snd_mixer_selem_ask_capture_vol_dB (self.mixer.handle, volume, &mut decibel_value)))
            .and_then(|_| Ok(decibel_value))
    }
}

/// Iterator over all possible channels. Use can_capture and can_playback to check if the channel is used or not
///
/// # Example
/// ```ignore
/// let mixer = ...
/// for c in MixerChannelIter::new(&mixer).filter(|c| c.can_playback() ) {
///    ...
/// }
/// ```
pub struct MixerChannelIter<'a> {
    mixer: &'a Mixer,
    current: i32
}

impl<'a> MixerChannelIter<'a> {
    pub fn new(m: &'a Mixer) -> MixerChannelIter {
        MixerChannelIter {
            mixer: m,
            current: 0
        }
    }
}

impl<'a> Iterator for MixerChannelIter<'a> {
    type Item = MixerChannel<'a>;

    fn next(&mut self) -> Option<MixerChannel<'a>> {
        if self.current > SND_MIXER_SCHN_LAST {
            None
        } else {
            self.current += 1;
            Some(MixerChannel::new(self.mixer, self.current-1))
        }
    }
}

#[test]
fn print_mixer_of_cards() {
    for card in card::Iter::new().map(|c| c.unwrap()) {
        println!("Card #{}: {} ({})", card.get_index(), card.get_name().unwrap(), card.get_longname().unwrap());
        for mixer in Iter::new(card::Card::new(card.get_index())).unwrap().map(|m| m.unwrap()) {
            assert!(mixer.is_null() == false );
            println!("\tMixer {}:{}", mixer.get_index(), mixer.get_name().unwrap());

            if mixer.has_volume() {
                print!("\t  Volume limits: ");
                if mixer.has_capture_volume() {
                    print!("Capture={}-{} ", mixer.capture_volume_range()[0],mixer.capture_volume_range()[1] );
                    print!("/{}dB-{}dB ", mixer.capture_decibel_range()[0] as f32 / 100.0, mixer.capture_decibel_range()[1] as f32 / 100.0 );
                }
                if mixer.has_playback_volume() {
                    print!("Playback={}-{} ", mixer.playback_volume_range()[0],mixer.playback_volume_range()[1]);
                    print!("/{}dB-{}dB ", mixer.playback_decibel_range()[0] as f32 / 100.0, mixer.playback_decibel_range()[1] as f32 / 100.0);
                }
                println!("");
            }

            if mixer.can_capture() {
                print!("\t  Capture channels: ");
                for c in MixerChannelIter::new(&mixer).filter(|c| c.can_capture() ) {
                    if c.can_capture() { print!("{}:{} ", c.channel, c.channel_name().unwrap()) };
                }
                println!("");
                print!("\t  Capture volumes: ");
                for c in MixerChannelIter::new(&mixer).filter(|c| c.can_playback() ) {
                    print!("{}:{}/{}dB ", c.channel,
                        match c.capture_volume() {Ok(v) => format!("{}",v).to_string(), Err(_) => "n/a".to_string()},
                        match c.capture_volume_decibel() {Ok(v) => format!("{}",v as f32 /100.0).to_string(), Err(_) => "n/a".to_string()}
                    );
                }
                println!("");
            }

            if mixer.can_playback() {
                print!("\t  Playback channels: ");
                for c in MixerChannelIter::new(&mixer) {
                    if c.can_playback() { print!("{}:{} ", c.channel, c.channel_name().unwrap()) };
                }
                println!("");
                print!("\t  Playback volumes: ");
                for c in MixerChannelIter::new(&mixer).filter(|c| c.can_playback() ) {
                    print!("{}:{}/{}dB ",
                        c.channel,
                        match c.playback_volume() {Ok(v) => format!("{}",v).to_string(), Err(_) => "n/a".to_string()},
                        match c.playback_volume_decibel() {Ok(v) => format!("{}",(v as f32) / 100.0).to_string(), Err(_) => "n/a".to_string()}
                    );
                }
                println!("");
            }
        }
    }
}
