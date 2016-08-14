//! This ALSA API wrapper/binding is WIP - the ALSA API is huge, and new
//! functions and structs might be added as requested. Enjoy!

extern crate alsa_sys as alsa;
extern crate libc;
#[macro_use]
extern crate bitflags;

macro_rules! alsa_enum {
 ($(#[$attr:meta])+ $name:ident, $static_name:ident [$count:expr], $( $a:ident = $b:ident),* ,) =>
{
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
$(#[$attr])*
pub enum $name {
$(
    $a = alsa::$b as isize,
)*
}

static $static_name: [$name; $count] =
  [ $( $name::$a, )* ];

impl $name {
    /// Returns a slice of all possible values; useful for iteration
    pub fn all() -> &'static [$name] { &$static_name[..] }

    #[allow(dead_code)]
    fn from_c_int(c: ::libc::c_int, s: &'static str) -> Result<$name> {
        Self::all().iter().find(|&&x| c == x as ::libc::c_int).map(|&x| x)
            .ok_or_else(|| Error::new(Some(s.into()), INVALID_ENUM))
    }
}

}
}

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

/// Used to restrict hw parameters. In case the submitted
/// value is unavailable, in which direction should one search
/// for available values?
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValueOr {
    /// The value set is the submitted value, or less
    Less = -1,
    /// The value set is the submitted value, or the nearest
    Nearest = 0,
    /// The value set is the submitted value, or greater
    Greater = 1,
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

pub mod poll;
pub use poll::PollDescriptors as PollDescriptors;

pub mod mixer;
pub use mixer::Mixer as Mixer;

mod io;
pub use io::Output;

// Reexported inside PCM module
mod chmap;
