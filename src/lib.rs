//! This ALSA API wrapper/binding is WIP - the ALSA API is huge, and new
//! functions and structs might be added as requested. Enjoy!

extern crate alsa_sys as alsa;
extern crate libc;

/// Replaces constants ending with PLAYBACK/CAPTURE as well as
/// INPUT/OUTPUT 
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    Playback,
    Capture
}
impl Direction {
    #[inline]
    pub fn input() -> Direction { Direction::Capture }
    #[inline]
    pub fn output() -> Direction { Direction::Playback }
}


mod error;
pub use error::{Error, Result};

pub mod card;
pub use card::Card as Card;

mod ctl_int;
pub mod ctl {
    //! Control device API
    pub use super::ctl_int::{Ctl, CardInfo, ElemIface, ElemId, ElemType, ElemValue, ElemInfo};
}

pub use ctl::Ctl as Ctl;

pub mod hctl;
pub use hctl::HCtl as HCtl;

pub mod pcm;
pub use pcm::PCM as PCM;

pub mod rawmidi;
pub use rawmidi::Rawmidi as Rawmidi;

pub mod device_name;

