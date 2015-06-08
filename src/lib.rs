extern crate alsa_sys as alsa;
extern crate libc;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    Playback,
    Capture
}

mod error;
pub use error::{Error, Result};

pub mod card;
pub use card::Card as Card;

pub mod ctl;
pub use ctl::Ctl as Ctl;

pub mod pcm;
pub use pcm::PCM as PCM;

pub mod rawmidi;

