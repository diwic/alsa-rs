//! Thin but safe wrappers for [ALSA](http://http://alsa-project.org).
//!
//! [Github repo](https://github.com/diwic/alsa-rs)
//!
//! [Crates.io](https://crates.io/crates/alsa)
//! 
//! This ALSA API wrapper/binding is WIP - the ALSA API is huge, and new
//! functions and structs might be added as requested.
//!
//! Most functions map 1-to-1 to alsa-lib functions, e g, `ctl::CardInfo::get_id()` is a wrapper around
//! `snd_ctl_card_info_get_id` and the [alsa-lib documentation](http://www.alsa-project.org/alsa-doc/alsa-lib/)
//! can be consulted for additional information. 
//!
//! Enjoy!

extern crate alsa_sys as alsa;
extern crate libc;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate nix;

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

/// Rounding mode (used in some mixer related calls)
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Round {
    /// Round down (towards negative infinity)
    Floor = 0,
    /// Round up (towards positive infinity)
    Ceil = 1,
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

pub mod seq;
pub use seq::Seq as Seq;

mod io;
pub use io::Output;

// Reexported inside PCM module
mod chmap;

mod pcm_direct;

/// Functions that bypass alsa-lib and talk directly to the kernel.
pub mod direct {
    /// This module bypasses alsa-lib and directly read and write into memory mapped kernel memory.
    ///
    /// In case of the sample memory, this is in many cases the DMA buffers that is transferred to the sound card.
    ///
    /// The reasons for doing this are:
    ///
    ///  * Minimum overhead where it matters most: let alsa-lib do the code heavy setup - 
    ///    then steal its file descriptor and deal with sample streaming from Rust.
    ///  * RT-safety to the maximum extent possible. Creating/dropping any of these structs causes syscalls,
    ///    but function calls on these are just read and write from memory. No syscalls, no memory allocations,
    ///    not even loops (with the exception of `MmapPlayback::write` that loops over samples to write).
    ///  * Possibility to allow Send + Sync for structs
    ///  * It's a fun experiment and an interesting deep dive into how alsa-lib does things.
    ///
    /// Note: Not all sound card drivers support this direct method of communication; although almost all
    /// modern/common ones do. It only works with hardware devices though (such as "hw:xxx" device strings),
    /// don't expect it to work with, e g, the PulseAudio plugin or so.
    pub mod pcm {
        pub use pcm_direct::{Status, Control, MmapCapture, MmapPlayback, MmapIO};

    }
}
