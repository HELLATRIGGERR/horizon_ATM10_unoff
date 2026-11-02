//! The chest menu right-clicking a device opens, in place of the mod's GUI:
//! six rows of plain items a stock client can show. The top five list the
//! networks the player may use, each a coloured wool; the bottom row has
//! create, disconnect and the pages. This is what the slots hold and what a
//! click on one means; the game's side makes them into stacks.
use crate::networks::{Network, PRESET_COLORS};

pub const ROWS: u8 = 6;
pub const SIZE: usize = 54;
/// The networks on a page.
pub const PER_PAGE: usize = 45;
pub const CREATE_SLOT: usize = 45;
pub const PREVIOUS_SLOT: usize = 47;
pub const INFO_SLOT: usize = 49;
pub const NEXT_SLOT: usize = 51;
pub const DISCONNECT_SLOT: usize = 53;

/// What a slot shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub item: String,
    pub name: String,
    /// The name's colour, 0xRRGGBB.
    pub color: u32,
    pub lore: Vec<String>,
    /// The enchanted shine: the network the device is on.
    pub shine: bool,
}

/// What the player asks by clicking a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Connect(i32),
    Create,
    Disconnect,
    Page(usize),
}

/// The vanilla wools by the colours they show, for the nearest to a network's.
const WOOLS: [(&str, u32); 16] = [
    ("white", 0xE9ECEC),
    ("orange", 0xF07613),
    ("magenta", 0xBD44B3),
    ("light_blue", 0x3AAFD9),
    ("yellow", 0xF8C627),
    ("lime", 0x70B919),
    ("pink", 0xED8DAC),
    ("gray", 0x3E4447),
    ("light_gray", 0x8E8E86),
    ("cyan", 0x158991),
    ("purple", 0x792AAC),
    ("blue", 0x35399D),
    ("brown", 0x724728),
    ("green", 0x546D1B),
    ("red", 0xA12722),
    ("black", 0x141519),
];

/// The wool nearest to a network's colour.
pub fn wool_of(color: u32) -> String {
    let channels = |c: u32| [(c >> 16 & 0xFF) as i32, (c >> 8 & 0xFF) as i32, (c & 0xFF) as i32];
    let wanted = channels(color);
    let (name, _) = WOOLS
        .iter()
        .min_by_key(|(_, c)| {
            let c = channels(*c);
            (0..3).map(|i| (c[i] - wanted[i]).pow(2)).sum::<i32>()
        })
        .expect("not empty");
    format!("minecraft:{name}_wool")
}

/// How many pages `networks` networks take; at least one.
pub fn pages(networks: usize) -> usize {
    networks.div_ceil(PER_PAGE).max(1)
}

/// The device a menu is about, as its info slot shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceView {
    /// Its item, `fluxnetworks:flux_plug`.
    pub item: String,
    pub network: i32,
    pub priority: i32,
    pub surge: bool,
    pub limit: i64,
    pub disable_limit: bool,
    pub buffer: i64,
}

/// One network as the list shows it.
fn network_slot(network: &Network, access: &str, connected: bool) -> Slot {
    let mut lore = vec![format!("Security: {}", network.security.name()), format!("Access: {access}")];
    if connected {
        lore.insert(0, "Connected".to_owned());
    }
    Slot { item: wool_of(network.color), name: network.name.clone(), color: network.color, lore, shine: connected }
}

/// The slots of the menu: `networks` are the ones the player may use, with
/// their access as text.
pub fn layout(networks: &[(&Network, &str)], page: usize, device: &DeviceView) -> Vec<Option<Slot>> {
    let mut slots = vec![None; SIZE];
    let page = page.min(pages(networks.len()) - 1);
    for (i, (network, access)) in networks.iter().skip(page * PER_PAGE).take(PER_PAGE).enumerate() {
        slots[i] = Some(network_slot(network, access, network.id == device.network));
    }
    let plain = |item: &str, name: &str, lore: Vec<String>| {
        Some(Slot { item: item.to_owned(), name: name.to_owned(), color: 0xFFFFFF, lore, shine: false })
    };
    slots[CREATE_SLOT] = plain("minecraft:emerald", "Create New Network", vec![
        "A private network of yours, with the next colour".to_owned(),
    ]);
    if page > 0 {
        slots[PREVIOUS_SLOT] = plain("minecraft:arrow", "Previous Page", vec![]);
    }
    if page + 1 < pages(networks.len()) {
        slots[NEXT_SLOT] = plain("minecraft:arrow", "Next Page", vec![]);
    }
    let current = networks.iter().find(|(n, _)| n.id == device.network);
    let limit = if device.disable_limit { "no limit".to_owned() } else { format!("{} FE/t", device.limit) };
    slots[INFO_SLOT] = plain(&device.item, "This Device", vec![
        format!("Network: {}", current.map_or("none", |(n, _)| n.name.as_str())),
        format!("Priority: {}{}", device.priority, if device.surge { " (surge)" } else { "" }),
        format!("Transfer limit: {limit}"),
        format!("Buffer: {} FE", device.buffer),
    ]);
    slots[DISCONNECT_SLOT] = plain("minecraft:barrier", "Disconnect", vec![]);
    slots
}

/// What a click on `slot` means; a click outside what the page shows means
/// nothing. `slot` is the menu's own numbering.
pub fn action(networks: &[i32], page: usize, slot: i32) -> Option<Action> {
    let slot = usize::try_from(slot).ok()?;
    let page = page.min(pages(networks.len()) - 1);
    match slot {
        CREATE_SLOT => Some(Action::Create),
        DISCONNECT_SLOT => Some(Action::Disconnect),
        PREVIOUS_SLOT if page > 0 => Some(Action::Page(page - 1)),
        NEXT_SLOT if page + 1 < pages(networks.len()) => Some(Action::Page(page + 1)),
        0..PER_PAGE => networks.get(page * PER_PAGE + slot).copied().map(Action::Connect),
        _ => None,
    }
}

/// The first preset colour, what a device on no network would show.
pub const fn first_preset() -> u32 {
    PRESET_COLORS[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networks::Networks;

    const ALEX: u128 = 1;

    fn device(network: i32) -> DeviceView {
        DeviceView {
            item: "fluxnetworks:flux_plug".into(),
            network,
            priority: 2,
            surge: true,
            limit: 800_000,
            disable_limit: false,
            buffer: 10,
        }
    }

    fn some(networks: usize) -> Networks {
        let mut n = Networks::new(0);
        for _ in 0..networks {
            n.create_default(ALEX, "Alex", -1).unwrap();
        }
        n
    }

    #[test]
    fn the_list_shows_names_colours_and_the_network_in_use() {
        let n = some(2);
        let list: Vec<(&Network, &str)> = n.by_id.values().map(|x| (x, "Owner")).collect();
        let slots = layout(&list, 0, &device(2));
        let first = slots[0].as_ref().unwrap();
        assert_eq!(first.name, "Alex's Network #1");
        assert_eq!(first.color, PRESET_COLORS[0]);
        assert_eq!(first.item, "minecraft:blue_wool");
        assert!(!first.shine);
        let second = slots[1].as_ref().unwrap();
        assert!(second.shine);
        assert_eq!(second.lore[0], "Connected");
        assert!(slots[2].is_none());
        assert_eq!(slots[CREATE_SLOT].as_ref().unwrap().item, "minecraft:emerald");
        assert_eq!(slots[DISCONNECT_SLOT].as_ref().unwrap().item, "minecraft:barrier");
        let info = slots[INFO_SLOT].as_ref().unwrap();
        assert_eq!(info.item, "fluxnetworks:flux_plug");
        assert_eq!(info.lore[0], "Network: Alex's Network #2");
        assert_eq!(info.lore[1], "Priority: 2 (surge)");
        assert_eq!(info.lore[2], "Transfer limit: 800000 FE/t");
        assert!(slots[PREVIOUS_SLOT].is_none() && slots[NEXT_SLOT].is_none());
    }

    #[test]
    fn clicks_mean_what_the_slots_show() {
        let ids = [11, 22];
        assert_eq!(action(&ids, 0, 0), Some(Action::Connect(11)));
        assert_eq!(action(&ids, 0, 1), Some(Action::Connect(22)));
        assert_eq!(action(&ids, 0, 2), None, "an empty slot");
        assert_eq!(action(&ids, 0, CREATE_SLOT as i32), Some(Action::Create));
        assert_eq!(action(&ids, 0, DISCONNECT_SLOT as i32), Some(Action::Disconnect));
        assert_eq!(action(&ids, 0, INFO_SLOT as i32), None);
        assert_eq!(action(&ids, 0, PREVIOUS_SLOT as i32), None);
        assert_eq!(action(&ids, 0, NEXT_SLOT as i32), None);
        assert_eq!(action(&ids, 0, -999), None, "outside the window");
        assert_eq!(action(&ids, 0, 60), None, "the player's inventory");
    }

    #[test]
    fn many_networks_take_pages() {
        let n = some(50);
        let list: Vec<(&Network, &str)> = n.by_id.values().map(|x| (x, "Owner")).collect();
        let ids: Vec<i32> = n.by_id.keys().copied().collect();
        assert_eq!(pages(ids.len()), 2);
        let first = layout(&list, 0, &device(-1));
        assert!(first[NEXT_SLOT].is_some() && first[PREVIOUS_SLOT].is_none());
        let second = layout(&list, 1, &device(-1));
        assert_eq!(second[0].as_ref().unwrap().name, "Alex's Network #46");
        assert!(second[5].is_none());
        assert!(second[PREVIOUS_SLOT].is_some() && second[NEXT_SLOT].is_none());
        assert_eq!(action(&ids, 1, 0), Some(Action::Connect(46)));
        assert_eq!(action(&ids, 0, NEXT_SLOT as i32), Some(Action::Page(1)));
        assert_eq!(action(&ids, 1, PREVIOUS_SLOT as i32), Some(Action::Page(0)));
        assert_eq!(pages(0), 1);
        assert_eq!(pages(45), 1);
        assert_eq!(pages(46), 2);
    }

    #[test]
    fn colours_pick_the_nearest_wool() {
        assert_eq!(wool_of(0xAA3939), "minecraft:red_wool");
        assert_eq!(wool_of(0xC6B900), "minecraft:yellow_wool");
        assert_eq!(wool_of(0x87CEFA), "minecraft:light_blue_wool");
        assert_eq!(wool_of(0x609732), "minecraft:lime_wool");
        assert_eq!(wool_of(first_preset()), "minecraft:blue_wool");
    }
}
