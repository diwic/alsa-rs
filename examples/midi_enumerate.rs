//! Example that enumerates hardware and PCM devices.

use alsa::card::Iter as CardIter;
use alsa::ctl::Ctl;
use alsa::rawmidi::Iter as MidiIter;
use alsa::Card;
use alsa::Error;

use alsa::seq::{Addr, ClientIter, PortIter, PortSubscribeIter, QuerySubsType, Seq};

pub fn list_rawmidi_for_card(card: &Card) -> Result<(), Error> {
    // Get a Ctl for the card
    let ctl_id = format!("hw:{}", card.get_index());
    let ctl = Ctl::new(&ctl_id, false)?;

    // Read card id and name
    let cardinfo = ctl.card_info()?;
    let card_id = cardinfo.get_id()?;
    let card_name = cardinfo.get_name()?;
    let subdevices: Vec<_> = MidiIter::new(&ctl).filter_map(|d| d.ok()).collect();
    if !subdevices.is_empty() {
        println!("card_id: {:?} card_name {:?} ", card_id, card_name);
        for info in &subdevices {
            println!(
                "subdevice: {:?} {:?} {:?}",
                info.get_subdevice(),
                info.get_subdevice_name().unwrap(),
                info.get_stream()
            );
        }
        println!();
    }
    Ok(())
}

pub fn list_rawmidi_devices() {
    let cards = CardIter::new();
    cards.for_each(|card| {
        if let Ok(c) = card {
            list_rawmidi_for_card(&c).unwrap_or_default()
        }
    });
}

pub fn list_sequencer_port() {
    let seq = Seq::open(None, None, false).unwrap();
    println!("Seq client {:?}", seq.client_id().unwrap());
    println!();

    seq.set_client_name(&c"ALSA_RS_EXAMPLE").unwrap();
    for client in ClientIter::new(&seq) {
        println!(
            "Client {:?} {:?}",
            client.get_client(),
            client.get_name().unwrap()
        );
        for port in PortIter::new(&seq, client.get_client()) {
            println!(
                "  Port {:?} {:?}",
                port.get_port(),
                port.get_name().unwrap()
            );
            for sub in PortSubscribeIter::new(
                &seq,
                Addr {
                    client: client.get_client(),
                    port: port.get_port(),
                },
                QuerySubsType::READ,
            ) {
                println!("    READ {:?}", sub.get_dest());
            }
            for sub in PortSubscribeIter::new(
                &seq,
                Addr {
                    client: client.get_client(),
                    port: port.get_port(),
                },
                QuerySubsType::WRITE,
            ) {
                println!("    WRITE {:?}", sub.get_dest());
            }
        }
        println!();
    }
}

fn main() {
    println!("\n--- Raw MIDI devices ---");
    list_rawmidi_devices();
    println!("\n--- MIDI Sequencer Clients, Ports and Subscribers ---");
    list_sequencer_port();
}
