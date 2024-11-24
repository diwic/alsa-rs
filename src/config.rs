//! Configuration file API
//!
//! For now, just contains two functions regarding the global configuration
//! stored as a cache inside alsa-lib. Calling `update_free_global` might help
//! against valgrind reporting memory leaks.
use crate::{alsa};
use super::error::*;

pub fn update() -> Result<()> {
	acheck!(snd_config_update()).map(|_| ())
}

pub fn update_free_global() -> Result<()> {
	acheck!(snd_config_update_free_global()).map(|_| ())
}