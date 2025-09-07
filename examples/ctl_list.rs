//! Example that enumerates controls for a device.

use alsa::card::Iter;
use alsa::ctl::Ctl;
use alsa::Card;
use alsa::Error;

fn list_controls_for_card(card: &Card) -> Result<(), Error> {
    // Get a Ctl for the card
    let ctl_id = format!("hw:{}", card.get_index());
    let ctl = Ctl::new(&ctl_id, false)?;

    println!("card {}", ctl_id);

    // Query the elements
    let elems = ctl.elem_list()?;
    for list_index in 0..elems.get_used() {
        let numeric_id = elems.get_numid(list_index)?;
        let name = elems.get_name(list_index)?;
        println!("  {}: {}", numeric_id, name);
    }

    Ok(())
}

fn main() {
    let cards = Iter::new();
    cards.for_each(|card| {
        if let Ok(c) = card {
            list_controls_for_card(&c).unwrap_or_default()
        }
    });
}
