Thin but safe wrappers for [ALSA](http://http://alsa-project.org).

WIP at this point, but basic playback/recording should work, and so should raw midi and get/set mixer values.

Quickstart guide / API design:

 * Most functions map 1-to-1 to alsa-lib functions, e g, `ctl::CardInfo::get_id()` is a wrapper around
   `snd_ctl_card_info_get_id` and the [alsa-lib documentation](http://www.alsa-project.org/alsa-doc/alsa-lib/)
   can be consulted for additional information.

 * Structs are RAII and closed/freed on drop, e g, when a `PCM` struct is dropped, `snd_pcm_close` is called.

 * To read and write buffers, call the `io_*` methods. It will return a separate struct from which you can
   read or write, and which can also be used for mmap (if supported by the driver).

 * Error handling - most alsa-lib functions can return errors, so the return value from these is a `Result`.

 * Enumeration of cards, devices etc is done through structs implementing `Iterator`.

 * Many structs implement `Polldescriptors`, to combine with poll or mio. (Or just use `wait` if you don't need that functionality.)
