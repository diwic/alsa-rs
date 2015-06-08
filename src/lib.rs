extern crate alsa_sys as alsa;
extern crate libc;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    Playback,
    Capture
}

mod error;
pub use error::{Error, Result};

mod card;
pub use card::{Card, CardIter};

mod ctl;
pub use ctl::{Ctl};

mod pcm;
pub use pcm::{PCM, PCMFormat, PCMHwParams, PCMAccess};

mod rawmidi;
pub use rawmidi::{RawmidiIter, RawmidiInfo};

