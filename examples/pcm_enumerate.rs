//! Example that enumerates hardware and PCM devices.

use alsa::Card;
use alsa::card::Iter;
use alsa::device_name::HintIter;
use alsa::ctl::{Ctl, DeviceIter};
use alsa::{Direction, Error};

// Each card can have multiple devices and subdevices, list them all
fn list_devices_for_card(card: &Card, direction: Direction) -> Result<(), Error>{
    // Get a Ctl for the card
    let ctl_id = format!("hw:{}", card.get_index());
    let ctl = Ctl::new(&ctl_id, false)?;

    // Read card id and name
    let cardinfo = ctl.card_info()?;
    let card_id = cardinfo.get_id()?;
    let card_name = cardinfo.get_name()?;
    for device in DeviceIter::new(&ctl) {
        // Read info from Ctl
        let pcm_info = match ctl.pcm_info(device as u32, 0, direction) {
            // If pcm_info returns ENOENT, there are no streams in this direction
            Err(x) if x.errno() == libc::ENOENT => continue,
            x => x,
        }?;

        // Read PCM name
        let pcm_name = pcm_info.get_name()?.to_string();

        println!("card: {:<2} id: {:<10} device: {:<2} card name: '{}' PCM name: '{}'", card.get_index(), card_id, device, card_name, pcm_name);

        // Loop through subdevices and get their names
        let subdevs = pcm_info.get_subdevices_count();
        for subdev in 0..subdevs {
            // Get subdevice name
            let pcm_info = ctl.pcm_info(device as u32, subdev, direction)?;
            let subdev_name = pcm_info.get_subdevice_name()?;

            println!("  subdevice: {:<2} name: '{}'", subdev, subdev_name);
        }
    }

    Ok(())
}

pub fn list_hw_devices(direction: Direction) {
    let cards = Iter::new();
    cards.for_each(|card| if let Ok(c) = card { list_devices_for_card(&c, direction).unwrap_or_default() });
}

pub fn list_pcm_devices(direction: Direction) {
    let hints = HintIter::new_str(None, "pcm").unwrap();
    for hint in hints {
        // When Direction is None it means that both the PCM supports both playback and capture
        if hint.name.is_some() && hint.desc.is_some() && (hint.direction.is_none() || hint.direction.map(|dir| dir == direction).unwrap_or_default()) {
            println!("name: {:<35} desc: {:?}", hint.name.unwrap(), hint.desc.unwrap());
        }
    }
}

fn main() {
    println!("\n--- Hardware playback devices ---");
    list_hw_devices(Direction::Playback);
    println!("\n--- Hardware capture devices ---");
    list_hw_devices(Direction::Capture);

    println!("\n--- PCM playback devices ---");
    list_pcm_devices(Direction::Playback);
    println!("\n--- PCM capture devices ---");
    list_pcm_devices(Direction::Capture);
}