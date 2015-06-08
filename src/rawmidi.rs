use libc::{c_int, c_uint};
use super::{Ctl, Direction};
use super::error::*;
use alsa;
use std::ptr;

pub struct Iter<'a> {
    ctl: &'a Ctl,
    device: c_int,
    in_count: i32,
    out_count: i32,
    current: i32,
}

pub struct Info(*mut alsa::snd_rawmidi_info_t);

impl Drop for Info {
    fn drop(&mut self) { unsafe { alsa::snd_rawmidi_info_free(self.0) }; }
}

impl Info {
    fn new() -> Result<Info> {
        let mut p = ptr::null_mut();
        check("snd_rawmidi_info_malloc", unsafe { alsa::snd_rawmidi_info_malloc(&mut p) }).map(|_| Info(p))
    }

    fn from_iter(c: &Ctl, device: i32, sub: i32, dir: Direction) -> Result<Info> {
        let r = try!(Info::new());
        unsafe { alsa::snd_rawmidi_info_set_device(r.0, device as c_uint) };
        let d = match dir {
            Direction::Playback => alsa::SND_RAWMIDI_STREAM_OUTPUT,
            Direction::Capture => alsa::SND_RAWMIDI_STREAM_INPUT,
        };
        unsafe { alsa::snd_rawmidi_info_set_stream(r.0, d) };
        unsafe { alsa::snd_rawmidi_info_set_subdevice(r.0, sub as c_uint) };
        try!(check("snd_ctl_rawmidi_info", unsafe { alsa::snd_ctl_rawmidi_info(c.handle(), r.0) }));
        Ok(r)
    }

    fn subdev_count(c: &Ctl, device: c_int) -> Result<(i32, i32)> {
        let i = try!(Info::from_iter(c, device, 0, Direction::Capture));
        let o = try!(Info::from_iter(c, device, 0, Direction::Playback));
        Ok((unsafe { alsa::snd_rawmidi_info_get_subdevices_count(o.0) as i32 },
            unsafe { alsa::snd_rawmidi_info_get_subdevices_count(i.0) as i32 }))
    }

    pub fn get_device(&self) -> i32 { unsafe { alsa::snd_rawmidi_info_get_device(self.0) as i32 }}
    pub fn get_subdevice(&self) -> i32 { unsafe { alsa::snd_rawmidi_info_get_subdevice(self.0) as i32 }}
    pub fn get_stream(&self) -> super::Direction {
        if unsafe { alsa::snd_rawmidi_info_get_stream(self.0) } == alsa::SND_RAWMIDI_STREAM_OUTPUT { super::Direction::Playback }
        else { super::Direction::Capture }
    }

    pub fn get_subdevice_name(&self) -> Result<String> {
        let c = unsafe { alsa::snd_rawmidi_info_get_subdevice_name(self.0) };
        from_const("snd_rawmidi_info_get_subdevice_name", c).map(|s| s.to_string())
    }
}


impl<'a> Iter<'a> {
    pub fn new(c: &'a Ctl) -> Iter<'a> { Iter { ctl: c, device: -1, in_count: 0, out_count: 0, current: 0 }}
}

impl<'a> Iterator for Iter<'a> {
    type Item = Result<Info>;
    fn next(&mut self) -> Option<Result<Info>> {
        if self.current < self.in_count {
            self.current += 1;
            return Some(Info::from_iter(&self.ctl, self.device, self.current-1, Direction::Capture));
        }
        if self.current - self.in_count < self.out_count {
            self.current += 1;
            return Some(Info::from_iter(&self.ctl, self.device, self.current-1-self.in_count, Direction::Playback));
        }

        let r = check("snd_ctl_rawmidi_next_device", unsafe { alsa::snd_ctl_rawmidi_next_device(self.ctl.handle(), &mut self.device) });
        match r {
            Err(e) => return Some(Err(e)),
            Ok(_) if self.device == -1 => return None,
            _ => {},
        }
        self.current = 0;
        match Info::subdev_count(&self.ctl, self.device) {
            Err(e) => Some(Err(e)),
            Ok((oo, ii)) => {
                self.in_count = ii;
                self.out_count = oo;
                self.next()
            }
        }
    }
}

#[test]
fn print_rawmidis() {
    for a in super::card::Iter::new().map(|a| a.unwrap()) {
        for b in Iter::new(&Ctl::from_card(&a, false).unwrap()).map(|b| b.unwrap()) {
            println!("Rawmidi {:?} (hw:{},{},{}) {} - {}", b.get_stream(), a.get_index(), b.get_device(), b.get_subdevice(),
                 a.get_name().unwrap(), b.get_subdevice_name().unwrap())
        }
    }
}
