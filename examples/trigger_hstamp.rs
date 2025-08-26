use alsa::{
    pcm::{Access, Format, HwParams},
    Direction, ValueOr, PCM,
};

use anyhow::Result; // Sorry for the extra dep :D

fn main() -> Result<()> {
    // Also tried specifying the hw card name, but it doesn't matter
    let pcm = PCM::new("default", Direction::Capture, false)?;

    {
        let hwp = HwParams::any(&pcm)?;
        hwp.set_channels(2)?;
        hwp.set_rate(16000, ValueOr::Nearest)?;
        hwp.set_format(Format::s32())?;
        hwp.set_access(Access::RWInterleaved)?;
        pcm.hw_params(&hwp)?;
    }

    {
        let swp = pcm.sw_params_current()?;
        swp.set_tstamp_mode(true)?;
        swp.set_tstamp_type(alsa::pcm::TstampType::Monotonic)?; // Tried with every kind of timestamp type, it doesn't matter
        pcm.sw_params(&swp)?;
    }

    let mut buffer = [0i32; 8000];

    pcm.start()?;

    let pcm_io = pcm.io_i32()?;

    loop {
        // Debug: check if timestamps are enabled (it returns true as expected)
        let params = pcm.sw_params_current()?;
        println!("timestamps enabled: {}", params.get_tstamp_mode()?);

        let read_bytes = pcm_io.readi(&mut buffer)?;
        assert!(read_bytes > 0);

        // I've also tried to construct the status object just once outside the loop but nothing changes
        let status = pcm.status()?;
        
        dbg!(&status);

        // The following "htstamp" functions all wrongly return a timespec struct with 0 seconds and 0 nanoseconds
        // when using Rust >=1.88 (even tried on Nightly to check if it worked on there)
        let audio_htstamp = status.get_audio_htstamp();
        println!("{} {}", audio_htstamp.tv_sec, audio_htstamp.tv_nsec);

        let htstamp = status.get_htstamp();
        println!("{} {}", htstamp.tv_sec, htstamp.tv_nsec);

        let trigger_htstamp = status.get_trigger_htstamp();
        println!("{} {}", trigger_htstamp.tv_sec, trigger_htstamp.tv_nsec);
    }
}
