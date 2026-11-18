// Mekanism's Quantum Item Orchestration, slices 1 and 2 (`pack/mods/mekanism/NOTES.md`, "QIO"): the QIO Drives
// (`ItemQIODrive`, four tiers, their items in their custom data), the QIO Drive Array (`TileEntityQIODriveArray`,
// twelve drive slots, each drive's light on the block), QIO frequencies (`QIOFrequency` on Mekanism's frequency list,
// `mek_frequency`: named, public, private or trusted, coloured; picked on `GuiQIOTileFrequencySelect`) and the QIO
// Dashboard (`TileEntityQIODashboard`) with its item viewer (`GuiQIOItemViewer`: search, sorting, scrolling; taking and
// putting items with the cursor, shift-clicks both ways). The rules are `mekanism_energy::qio`.
//
// The server keeps each frequency's drives in memory (`FREQS`) as Mekanism does; a drive array registers the drives
// in its slots on its tick, and every change is written back into the drive's stack at once (`flush`), so the
// array's saved items are the frequency's items and a drive taken out carries them. The dashboard packs its
// frequency's item list into its menu data (`pack`); its screen filters, sorts and draws it on the client. The
// frequency select screen talks to the server in mod messages (`mekanism:qio`): what it sets, deletes or colours,
// and the list it shows, which the server answers for that player alone.
use super::{
    Game,
    mek_ui::{self, Pen, Rect},
    mek_world::{key, number},
    native_hooks::NativeScreen,
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    hud::HudBatch,
    items::ItemStack,
    text::Text,
};
use horizon_world::nbt::{Compound, Tag};
use mekanism_energy::{
    frequency::{COLORS, Identity, Security},
    qio::{self, Drive, DriveStatus, DriveTier, Storage},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

pub(super) const DRIVE_ARRAY: &str = "mekanism:qio_drive_array";
pub(super) const DASHBOARD: &str = "mekanism:qio_dashboard";
const DRIVE_SLOTS: usize = 12;
const CHANNEL: &str = "mekanism:qio";
/// QIO's `FrequencyType` on the frequency list.
const KIND: &str = "qio";
use mekanism_energy::frequency::NAME_MAX;

// Menu buttons: the viewer's cells ((index + 1) * 4 + mode).
const VIEWER: i32 = 1_000;
// Mod messages, client to server: `[op, x, y, z, ...]` (`PacketSetTileFrequency`, `PacketSetFrequencyColor`); the
// list's answer comes back as `[OP_LIST, mode, ...]`.
const OP_TYPED: u8 = 1;
const OP_PICK: u8 = 2;
const OP_DELETE: u8 = 3;
const OP_COLOR: u8 = 4;
const OP_LIST: u8 = 5;

pub(super) fn is_qio(block: &str) -> bool {
    block == DRIVE_ARRAY || block == DASHBOARD
}

/// An item type: stacks of the same item and components (`HashedItem`).
#[derive(Clone)]
pub(super) struct Key(ItemStack);
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_item_same_components(&other.0)
    }
}

/// A frequency: its name, mode and owner (`Frequency.FrequencyIdentity`), as the list keeps it.
type FreqId = Identity;

/// A drive in a slot of an array; `token` tells a drive put in its place from the one before.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DriveId {
    dim: String,
    pos: [i32; 3],
    slot: u8,
    token: i64,
}

struct Freq {
    id: FreqId,
    storage: Storage<DriveId, Key>,
}

thread_local! {
    static FREQS: RefCell<Vec<Freq>> = const { RefCell::new(Vec::new()) };
    /// Each array's registered drives, by slot.
    static ARRAYS: RefCell<HashMap<(String, [i32; 3]), Vec<Option<(FreqId, DriveId)>>>> = RefCell::new(HashMap::new());
    /// Drive writes `flush` could not reach (another dimension): the array's tick writes them.
    static PENDING: RefCell<Vec<DriveId>> = const { RefCell::new(Vec::new()) };
    /// The revision each dashboard packed last, and its age.
    static PACKED: RefCell<HashMap<(String, [i32; 3]), (u64, u32)>> = RefCell::new(HashMap::new());
    static TOKEN: Cell<i64> = const { Cell::new(0) };
    // The client's screen state.
    static FOCUS: Cell<u8> = const { Cell::new(0) };
    static SEARCH: RefCell<String> = const { RefCell::new(String::new()) };
    static NAME: RefCell<String> = const { RefCell::new(String::new()) };
    /// The select screen: shown, its mode, the picked row, the list's scroll, the list the server sent and its age.
    static SELECT: Cell<bool> = const { Cell::new(false) };
    static MODE: Cell<u8> = const { Cell::new(0) };
    static ROW: Cell<Option<usize>> = const { Cell::new(None) };
    static LIST_SCROLL: Cell<usize> = const { Cell::new(0) };
    static LIST: RefCell<(u8, Vec<Listed>)> = const { RefCell::new((0, Vec::new())) };
    static LIST_AGE: Cell<u32> = const { Cell::new(0) };
    /// The open screen's menu data, for the select screen drawn after it.
    static LAST_DATA: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    static SCROLL: Cell<usize> = const { Cell::new(0) };
    static SORT: Cell<u8> = const { Cell::new(0) };
    static DESCENDING: Cell<bool> = const { Cell::new(false) };
    static OPEN: Cell<Option<[i32; 3]>> = const { Cell::new(None) };
    static OUTBOX: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

/// A frequency the select screen lists.
#[derive(Clone, Debug, Default)]
struct Listed {
    owner: u128,
    color: u8,
    name: String,
    owner_name: String,
}

/// The world closed: no frequency outlives it.
pub(super) fn clear() {
    FREQS.with(|f| f.borrow_mut().clear());
    ARRAYS.with(|a| a.borrow_mut().clear());
    PENDING.with(|p| p.borrow_mut().clear());
    PACKED.with(|p| p.borrow_mut().clear());
}

pub(super) fn new_entity(block: &str) -> (String, Compound) {
    (block.to_owned(), Compound::new())
}

/// The array's twelve drive slots in a 9x2 menu whose last six the screen leaves out; nothing reaches them from
/// outside (`TileEntityQIODriveArray` exposes no item handler). The dashboard's menu holds nothing yet (its crafting
/// windows are slice 4).
pub(super) fn container(block: &str) -> ModContainer {
    if block == DRIVE_ARRAY {
        ModContainer { slots: 18, faces: Default::default(), inputs: Vec::new(), outputs: Vec::new(), menu: Some("minecraft:generic_9x2".into()) }
    } else {
        ModContainer { slots: 9, faces: Default::default(), inputs: Vec::new(), outputs: Vec::new(), menu: Some("minecraft:generic_9x1".into()) }
    }
}

/// `QIODriveSlot`: drives only, in the array's twelve.
pub(super) fn slot_accepts(block: &str, slot: usize, stack: &ItemStack, accepts: &mut bool) {
    *accepts = block == DRIVE_ARRAY && slot < DRIVE_SLOTS && DriveTier::of_item(&stack.item).is_some();
}

// ---- saved data ----------------------------------------------------------------------------------------------

fn frequency_of(nbt: &Compound) -> Option<FreqId> {
    let Some(Tag::Compound(c)) = nbt.get(&key("qio_frequency")) else {
        return None;
    };
    let name = match c.get(&key("name")) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => return None,
    };
    let owner = match c.get(&key("owner")) {
        Some(Tag::String(s)) => u128::from_str_radix(&s.to_string_lossy(), 16).ok(),
        _ => None,
    };
    // Slice 1 wrote no mode: an owner meant private.
    let security = match c.get(&key("security")) {
        Some(Tag::String(s)) => Security::parse(&s.to_string_lossy())?,
        _ if owner.is_some() => Security::Private,
        _ => Security::Public,
    };
    Some(FreqId { name, security, owner: owner.unwrap_or(0) })
}

fn write_frequency(nbt: &mut Compound, id: Option<&FreqId>) {
    let Some(id) = id else {
        nbt.remove(&key("qio_frequency"));
        return;
    };
    let mut c = Compound::new();
    c.insert(key("name"), Tag::String(key(&id.name)));
    c.insert(key("security"), Tag::String(key(id.security.name())));
    c.insert(key("owner"), Tag::String(key(&format!("{:032x}", id.owner))));
    nbt.insert(key("qio_frequency"), Tag::Compound(c));
}

const CUSTOM_DATA: &str = "minecraft:custom_data";

/// A drive stack's `custom_data.mekanism.qio`: its token and items (Mekanism keeps them in its
/// `mekanism:drive_contents` and `drive_metadata` components, which the game doesn't take).
fn drive_data(stack: &ItemStack) -> Option<Compound> {
    match super::mek_world::item_data(stack)?.get(&key("qio"))? {
        Tag::Compound(c) => Some(c.clone()),
        _ => None,
    }
}

fn read_drive(stack: &ItemStack) -> Option<(Drive<Key>, i64)> {
    let tier = DriveTier::of_item(&stack.item)?;
    let data = drive_data(stack).unwrap_or_default();
    let token = number(data.get(&key("token"))).unwrap_or(0);
    let mut items = Vec::new();
    if let Some(Tag::List(list)) = data.get(&key("items")) {
        for entry in list {
            if let Tag::Compound(c) = entry
                && let Some(tag) = c.get(&key("stack"))
                && let Some(mut s) = horizon_client::save::item_from_nbt(tag)
            {
                s.count = 1;
                items.push((Key(s), number(c.get(&key("count"))).unwrap_or(0).max(0) as u64));
            }
        }
    }
    Some((Drive::new(tier, items), token))
}

fn write_drive(stack: &mut ItemStack, drive: &Drive<Key>, token: i64) {
    let mut qio = Compound::new();
    qio.insert(key("token"), Tag::Long(token));
    qio.insert(key("count"), Tag::Long(drive.count as i64));
    qio.insert(key("types"), Tag::Int(drive.items.len() as i32));
    let items = drive
        .items
        .iter()
        .map(|(k, n)| {
            let mut c = Compound::new();
            c.insert(key("stack"), horizon_client::save::item_to_nbt(&k.0));
            c.insert(key("count"), Tag::Long(*n as i64));
            Tag::Compound(c)
        })
        .collect();
    qio.insert(key("items"), Tag::List(items));
    let mut custom = match stack.data().and_then(|d| d.other(CUSTOM_DATA)) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::new(),
    };
    let mut mekanism = match custom.get(&key("mekanism")) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::new(),
    };
    mekanism.insert(key("qio"), Tag::Compound(qio));
    custom.insert(key("mekanism"), Tag::Compound(mekanism));
    stack.data_mut().set_other(CUSTOM_DATA, Some(Tag::Compound(custom)));
    stack.tidy();
}

fn next_token() -> i64 {
    TOKEN.with(|t| {
        if t.get() == 0 {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as i64);
            t.set(now & 0x7FFF_FFFF_FFFF_0000);
        }
        t.set(t.get() + 1);
        t.get()
    })
}

// ---- frequencies -----------------------------------------------------------------------------------------------

fn with_freq<R>(id: &FreqId, f: impl FnOnce(&mut Freq) -> R) -> R {
    FREQS.with(|all| {
        let mut all = all.borrow_mut();
        let at = match all.iter().position(|q| &q.id == id) {
            Some(i) => i,
            None => {
                all.push(Freq { id: id.clone(), storage: Storage::new() });
                all.len() - 1
            }
        };
        f(&mut all[at])
    })
}

fn find_freq<R>(id: &FreqId, f: impl FnOnce(&Freq) -> R) -> Option<R> {
    FREQS.with(|all| all.borrow().iter().find(|q| &q.id == id).map(f))
}

/// Writes every changed drive back into its stack: at once where the array is in reach, else at its next tick.
fn flush(game: &mut Game<'_>) {
    let changed: Vec<(DriveId, Drive<Key>)> = FREQS.with(|all| {
        let mut out = Vec::new();
        for q in all.borrow_mut().iter_mut() {
            for id in q.storage.take_changed() {
                if let Some(d) = q.storage.drive(&id) {
                    out.push((id, d.clone()));
                }
            }
        }
        out
    });
    let dim = game.native_dimension().to_owned();
    for (id, drive) in changed {
        let done = id.dim == dim && game.native_with_mod_block_entity(id.pos, |m| write_into(m, &id, &drive)).unwrap_or(false);
        if !done {
            PENDING.with(|p| p.borrow_mut().push(id));
        }
    }
}

fn write_into(m: &mut ModBlockEntity, id: &DriveId, drive: &Drive<Key>) -> bool {
    if m.id != DRIVE_ARRAY {
        return false;
    }
    match m.items.get_mut(id.slot as usize) {
        Some(Some(stack)) if read_drive(stack).is_some_and(|(_, t)| t == id.token) => {
            write_drive(stack, drive, id.token);
            true
        }
        _ => false,
    }
}

/// Takes `stack` into the frequency: what is left of it.
fn insert(id: &FreqId, stack: &ItemStack, amount: u64) -> u64 {
    let mut k = stack.clone();
    k.count = 1;
    let key = Key(k);
    amount - with_freq(id, |q| q.storage.insert(&key, amount, true))
}

/// Up to `amount` of the frequency's `index`th type as a stack.
fn extract(id: &FreqId, index: usize, amount: u64) -> Option<ItemStack> {
    with_freq(id, |q| {
        let key = q.storage.types.get(index)?.key.clone();
        let given = q.storage.extract(&key, amount, true);
        (given > 0).then(|| ItemStack { count: given as u32, ..key.0 })
    })
}

// ---- the drive array -------------------------------------------------------------------------------------------

pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dim = game.native_dimension().to_owned();
    let mut freq = frequency_of(&entity.nbt);
    // `TileComponentFrequency`: a deleted frequency leaves its holders.
    if let Some(f) = &freq
        && !super::mek_frequency::validate(game, KIND, f)
    {
        write_frequency(&mut entity.nbt, None);
        freq = None;
    }
    if entity.id == DRIVE_ARRAY {
        tick_array(&dim, pos, entity, freq.as_ref());
        let lights = drive_lights(&dim, pos, entity);
        if entity.nbt.get(&key("qio_lights")) != Some(&Tag::String(key(&lights))) {
            entity.nbt.insert(key("qio_lights"), Tag::String(key(&lights)));
            game.native_block_entity_sync(pos);
        }
    } else {
        tick_dashboard(&dim, pos, entity, freq.as_ref());
    }
    let active = freq.is_some();
    let shown = game.native_block_property(pos, "active").as_deref() == Some("true");
    if active != shown {
        game.native_set_block_property(pos, "active", if active { "true" } else { "false" });
    }
}

fn tick_array(dim: &str, pos: [i32; 3], entity: &mut ModBlockEntity, freq: Option<&FreqId>) {
    // Writes another dimension's dashboard left for it.
    let mine: Vec<DriveId> = PENDING.with(|p| {
        let mut p = p.borrow_mut();
        let (mine, rest) = std::mem::take(&mut *p).into_iter().partition(|d| d.dim == dim && d.pos == pos);
        *p = rest;
        mine
    });
    let mut regs = ARRAYS.with(|a| a.borrow_mut().remove(&(dim.to_owned(), pos))).unwrap_or_else(|| vec![None; DRIVE_SLOTS]);
    for id in mine {
        if let Some((f, _)) = regs.get(id.slot as usize).cloned().flatten()
            && let Some(drive) = find_freq(&f, |q| q.storage.drive(&id).cloned()).flatten()
        {
            write_into(entity, &id, &drive);
        }
    }
    for slot in 0..DRIVE_SLOTS {
        let stack = entity.items.get_mut(slot).and_then(|s| s.as_mut());
        let mut drive = stack.as_ref().and_then(|s| read_drive(s));
        if let (Some(s), Some((d, token))) = (stack, drive.as_mut())
            && *token == 0
        {
            *token = next_token();
            write_drive(s, d, *token);
        }
        let token = drive.as_ref().map(|(_, t)| *t);
        if let Some((f, id)) = regs[slot].clone()
            && (Some(id.token) != token || Some(&f) != freq)
        {
            with_freq(&f, |q| q.storage.remove_drive(&id));
            regs[slot] = None;
        }
        if regs[slot].is_none()
            && let (Some((d, token)), Some(f)) = (drive, freq)
        {
            let id = DriveId { dim: dim.to_owned(), pos, slot: slot as u8, token };
            with_freq(f, |q| q.storage.add_drive(id.clone(), d));
            regs[slot] = Some((f.clone(), id));
        }
    }
    ARRAYS.with(|a| a.borrow_mut().insert((dim.to_owned(), pos), regs));
}

/// `TileEntityQIODriveArray.onUpdateServer`'s `DriveStatus` of each slot, one digit a slot (`DriveStatus` order).
fn drive_lights(dim: &str, pos: [i32; 3], entity: &ModBlockEntity) -> String {
    let regs = ARRAYS.with(|a| a.borrow().get(&(dim.to_owned(), pos)).cloned()).unwrap_or_default();
    (0..DRIVE_SLOTS)
        .map(|slot| {
            let has = entity.items.get(slot).and_then(|s| s.as_ref()).is_some_and(|s| DriveTier::of_item(&s.item).is_some());
            let online = regs.get(slot).cloned().flatten().and_then(|(f, id)| find_freq(&f, |q| q.storage.drive(&id).cloned()).flatten());
            let status = DriveStatus::of(has, online.as_ref());
            char::from(b'0' + DriveStatus::ALL.iter().position(|s| *s == status).unwrap_or(0) as u8)
        })
        .collect()
}

fn lights_of(nbt: &Compound) -> Vec<DriveStatus> {
    match nbt.get(&key("qio_lights")) {
        Some(Tag::String(s)) => s.to_string_lossy().bytes().map(|b| DriveStatus::ALL.get(usize::from(b.wrapping_sub(b'0'))).copied().unwrap_or(DriveStatus::None)).collect(),
        _ => Vec::new(),
    }
}

/// `native_block_entity_update_tag`: the array's lights reach the clients (`getReducedUpdateTag`'s drive status).
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id != DRIVE_ARRAY {
        return;
    }
    if let Some(lights) = entity.nbt.get(&key("qio_lights")) {
        tag.get_or_insert_with(Compound::new).insert(key("qio_lights"), lights.clone());
    }
}

/// `native_standalone_models`: the drives `DriveArrayBakedModel` puts in the slots.
pub(super) fn standalone_models(models: &mut Vec<String>) {
    models.extend(DriveStatus::ALL.iter().filter_map(|s| s.model()).map(str::to_owned));
}

/// `native_block_entity_submit`: each slot's drive in its light (`DriveArrayBakedModel`), turned with the block.
pub(super) fn draw_block(game: &Game<'_>, pos: [i32; 3], block: &str, entity: &ModBlockEntity, draw: &mut super::native_block_entity_draw::BlockEntityDraw) {
    if block != DRIVE_ARRAY {
        return;
    }
    let turn = match game.native_block_property(pos, "facing").as_deref() {
        Some("south") => 180.0,
        Some("east") => 90.0,
        Some("west") => -90.0,
        _ => 0.0,
    };
    for (slot, status) in lights_of(&entity.nbt).into_iter().enumerate().take(DRIVE_SLOTS) {
        let Some(model) = status.model() else { continue };
        let [x, y] = qio::drive_placement(slot);
        draw.push();
        draw.rotate_about([0.0, 1.0, 0.0], -turn, [0.5, 0.5, 0.5]);
        draw.translate([x, y, 0.0]);
        game.native_draw_model(draw, model, 0xFFFFFF);
        draw.pop();
    }
}

/// `native_block_entity_unload`: an array's drives leave their frequency (`onDeactivate`, `removeDrive`).
pub(super) fn unloaded(dimension: &str, pos: [i32; 3], id: &str) {
    if id == DRIVE_ARRAY
        && let Some(regs) = ARRAYS.with(|a| a.borrow_mut().remove(&(dimension.to_owned(), pos)))
    {
        for (f, d) in regs.into_iter().flatten() {
            with_freq(&f, |q| q.storage.remove_drive(&d));
        }
    }
    PACKED.with(|p| p.borrow_mut().remove(&(dimension.to_owned(), pos)));
}

// ---- the dashboard ---------------------------------------------------------------------------------------------

fn tick_dashboard(dim: &str, pos: [i32; 3], entity: &mut ModBlockEntity, freq: Option<&FreqId>) {
    let revision = freq.and_then(|f| find_freq(f, |q| q.storage.revision)).unwrap_or(0);
    let count = super::mek_frequency::read(|r| r.revision);
    let stamp = revision.wrapping_mul(31).wrapping_add(count);
    let fresh = PACKED.with(|p| {
        let mut p = p.borrow_mut();
        let e = p.entry((dim.to_owned(), pos)).or_insert((u64::MAX, 0));
        e.1 += 1;
        let again = e.0 != stamp || e.1 >= 20 || entity.data.is_empty();
        if again {
            *e = (stamp, 0);
        }
        again
    });
    if fresh {
        entity.data = pack(freq);
    }
}

/// The menu data: the frequency, its totals, its types (item id and count) and the frequencies to pick from, in
/// 16-bit words.
fn pack(freq: Option<&FreqId>) -> Vec<i32> {
    let mut d = Vec::new();
    d.push(i32::from(freq.is_some()));
    let (mut total, mut cap, mut types, mut type_cap) = (0, 0, 0, 0);
    let mut items: Vec<(String, u64)> = Vec::new();
    if let Some(f) = freq {
        find_freq(f, |q| {
            total = q.storage.total;
            cap = q.storage.count_capacity();
            types = q.storage.types.len() as u64;
            type_cap = q.storage.type_capacity() as u64;
            items = q.storage.types.iter().map(|t| (t.key.0.item.to_string(), t.count)).collect();
        });
    }
    push(&mut d, total, 4);
    push(&mut d, cap, 4);
    push(&mut d, types, 2);
    push(&mut d, type_cap, 2);
    push_str(&mut d, freq.map_or("", |f| f.name.as_str()));
    d.push(freq.map_or(0, |f| i32::from(f.security.index())));
    let (owner, color) = freq
        .and_then(|f| super::mek_frequency::read(|r| r.find(KIND, f).map(|q| (q.owner_name.clone(), q.color))))
        .unwrap_or_default();
    push_str(&mut d, &owner);
    d.push(i32::from(color));
    push(&mut d, items.len() as u64, 2);
    for (id, n) in &items {
        push_str(&mut d, id);
        push(&mut d, *n, 4);
    }
    d
}

fn push(out: &mut Vec<i32>, v: u64, pieces: usize) {
    out.extend((0..pieces).map(|i| ((v >> (16 * i)) & 0xFFFF) as i32));
}

fn read(data: &[i32], at: &mut usize, pieces: usize) -> u64 {
    let v = (0..pieces).fold(0, |v, i| v | (u64::from(data.get(*at + i).copied().unwrap_or(0) as u16) << (16 * i)));
    *at += pieces;
    v
}

fn push_str(out: &mut Vec<i32>, s: &str) {
    let bytes = &s.as_bytes()[..s.len().min(96)];
    out.push(bytes.len() as i32);
    out.extend(bytes.chunks(2).map(|c| i32::from(c[0]) | (i32::from(*c.get(1).unwrap_or(&0)) << 8)));
}

fn read_str(data: &[i32], at: &mut usize) -> String {
    let len = data.get(*at).copied().unwrap_or(0).clamp(0, 96) as usize;
    *at += 1;
    let mut bytes = Vec::with_capacity(len + 1);
    for i in 0..len.div_ceil(2) {
        let w = data.get(*at + i).copied().unwrap_or(0);
        bytes.push((w & 0xFF) as u8);
        bytes.push(((w >> 8) & 0xFF) as u8);
    }
    bytes.truncate(len);
    *at += len.div_ceil(2);
    String::from_utf8_lossy(&bytes).into_owned()
}

#[derive(Default)]
struct View {
    has: bool,
    total: u64,
    cap: u64,
    types: u64,
    type_cap: u64,
    name: String,
    security: u8,
    owner: String,
    color: u8,
    items: Vec<(String, u64)>,
}

fn unpack(data: &[i32]) -> View {
    if data.is_empty() {
        return View::default();
    }
    let mut at = 1;
    let mut v = View { has: data[0] != 0, ..View::default() };
    v.total = read(data, &mut at, 4);
    v.cap = read(data, &mut at, 4);
    v.types = read(data, &mut at, 2);
    v.type_cap = read(data, &mut at, 2);
    v.name = read_str(data, &mut at);
    v.security = read(data, &mut at, 1) as u8;
    v.owner = read_str(data, &mut at);
    v.color = read(data, &mut at, 1) as u8;
    let n = read(data, &mut at, 2) as usize;
    for _ in 0..n.min(data.len()) {
        let id = read_str(data, &mut at);
        let count = read(data, &mut at, 4);
        v.items.push((id, count));
    }
    v
}

// ---- server: menu buttons, shift-clicks, messages, commands --------------------------------------------------

/// `native_menu_button_v2`: a click on the viewer: with a stack on the cursor it
/// goes in (all of it, or one on a right click: `PacketQIOItemViewerSlotPlace`), else the cell's type comes out, a
/// stack (`PacketQIOItemViewerSlotTake`), half of one on a right click, or into the inventory on a shift-click
/// (`PacketQIOItemViewerSlotShiftTake`).
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32, carried: &mut Option<ItemStack>) {
    if !is_qio(&entity.id) {
        return;
    }
    let player = game.native_acting_id();
    if entity.id != DASHBOARD || id < VIEWER {
        return;
    }
    let Some(freq) = frequency_of(&entity.nbt) else { return };
    let cell = (id - VIEWER) / 4 - 1;
    let mode = (id - VIEWER) % 4;
    if let Some(held) = carried.as_mut().filter(|s| s.count > 0 && mode != 2) {
        let amount = if mode == 1 { 1 } else { u64::from(held.count) };
        let left = insert(&freq, held, amount);
        held.count -= (amount - left) as u32;
        if held.count == 0 {
            *carried = None;
        }
    } else if cell >= 0 {
        let index = cell as usize;
        let item = find_freq(&freq, |q| q.storage.types.get(index).map(|t| (t.key.0.item.clone(), t.count))).flatten();
        let Some((item, stored)) = item else { return };
        let max = u64::from(game.native_max_stack_size(&item).unwrap_or(64).max(1));
        let whole = max.min(stored);
        let amount = if mode == 1 { whole.div_ceil(2) } else { whole };
        let Some(stack) = extract(&freq, index, amount) else { return };
        if mode == 2 {
            if let Some(rest) = game.native_give_stack(player, stack) {
                let n = u64::from(rest.count);
                insert(&freq, &rest, n);
            }
        } else if carried.is_none() {
            *carried = Some(stack);
        } else {
            let n = u64::from(stack.count);
            insert(&freq, &stack, n);
        }
    }
    flush(game);
    let dim = game.native_dimension().to_owned();
    PACKED.with(|p| p.borrow_mut().remove(&(dim, pos)));
}

/// `native_menu_quick_move`: a shift-click on the player's inventory under a dashboard puts the stack into its
/// frequency (`QIOItemViewerContainer.quickMoveStack`); nothing moves without one.
pub(super) fn quick_move(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, container_slots: usize, slot: usize, stack: &mut ItemStack, handled: &mut bool) {
    if entity.id != DASHBOARD || slot < container_slots {
        return;
    }
    *handled = true;
    let Some(freq) = frequency_of(&entity.nbt) else { return };
    let amount = u64::from(stack.count);
    let left = insert(&freq, stack, amount);
    stack.count = left as u32;
    flush(game);
    let dim = game.native_dimension().to_owned();
    PACKED.with(|p| p.borrow_mut().remove(&(dim, pos)));
}

/// `native_net_from_client`: the frequency select screen (`GuiFrequencySelector`), each message `[op, x, y, z, ...]`
/// with the coordinates as little-endian ints: a typed name in a mode (`PacketSetTileFrequency` with a new
/// identity: `[mode, name...]`), a frequency picked from the list (`[mode, owner, name...]`), one deleted (the same;
/// `PacketSetTileFrequency` with `set` false), the block's frequency's colour stepped (`[next]`,
/// `PacketSetFrequencyColor`), and the list of a mode (`[mode]`), answered to that player alone.
pub(super) fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != CHANNEL || data.len() < 14 {
        return;
    }
    let int = |i: usize| i32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
    let pos = [int(1), int(5), int(9)];
    let mode = Security::from_index(data[13]);
    let named = |at: usize| -> Option<FreqId> {
        let owner = u128::from_le_bytes(data.get(at..at + 16)?.try_into().ok()?);
        Some(FreqId { name: String::from_utf8_lossy(data.get(at + 16..)?).chars().take(NAME_MAX).collect(), security: mode?, owner })
    };
    let name = super::mek_frequency::player_name(game, player);
    match data[0] {
        OP_TYPED => {
            let Some(mode) = mode else { return };
            let typed = String::from_utf8_lossy(&data[14..]).into_owned();
            if let Some(id) = super::mek_frequency::with(game, |f| f.get_or_create(KIND, &typed, mode, player, &name)) {
                set_frequency(game, pos, Some(id));
            }
        }
        OP_PICK => {
            if let Some(id) = named(14).and_then(|id| super::mek_frequency::with(game, |f| f.select(KIND, &id, player))) {
                set_frequency(game, pos, Some(id));
            }
        }
        OP_DELETE => {
            if let Some(id) = named(14) {
                super::mek_frequency::with(game, |f| f.remove(KIND, &id, player));
            }
        }
        OP_COLOR => {
            let Some(id) = game.native_with_mod_block_entity(pos, |m| frequency_of(&m.nbt)).flatten() else { return };
            super::mek_frequency::with(game, |f| f.step_color(KIND, &id, player, data[13] != 0));
        }
        OP_LIST => {
            let Some(mode) = mode else { return };
            let mut out = vec![OP_LIST, mode.index()];
            super::mek_frequency::with(game, |_| ());
            super::mek_frequency::read(|f| {
                let list = f.visible(KIND, mode, player);
                out.extend((list.len().min(200) as u16).to_le_bytes());
                for q in list.into_iter().take(200) {
                    out.extend(q.owner.to_le_bytes());
                    out.push(q.color);
                    for text in [&q.name, &q.owner_name] {
                        let b = &text.as_bytes()[..text.len().min(64)];
                        out.push(b.len() as u8);
                        out.extend(b);
                    }
                }
            });
            if let Err(e) = game.native_net_send_to_player(player, CHANNEL, &out) {
                log::warn!("[mekanism] the QIO frequency list wasn't sent: {e}");
            }
        }
        _ => {}
    }
}

fn set_frequency(game: &mut Game<'_>, pos: [i32; 3], id: Option<FreqId>) -> bool {
    game.native_with_mod_block_entity(pos, |m| {
        if !is_qio(&m.id) {
            return false;
        }
        write_frequency(&mut m.nbt, id.as_ref());
        true
    })
    .unwrap_or(false)
}

/// `/mek qio x y z freq [<name> [public|private|trusted]]`: the acting player's frequency of that name and mode
/// (made when there is none), or none.
pub(super) fn command(game: &mut Game<'_>, pos: [i32; 3], words: &[&str]) -> Value {
    match words {
        ["freq"] => json!({ "pos": pos, "set": set_frequency(game, pos, None) }),
        ["freq", name, rest @ ..] => {
            let mode = rest.first().map_or(Some(Security::Public), |m| Security::parse(m));
            let Some(mode) = mode else { return json!({ "error": "mode: public, private or trusted" }) };
            let player = game.native_acting_id();
            let owner = super::mek_frequency::player_name(game, player);
            let Some(id) = super::mek_frequency::with(game, |f| f.get_or_create(KIND, name, mode, player, &owner)) else {
                return json!({ "error": "no name" });
            };
            json!({ "pos": pos, "set": set_frequency(game, pos, Some(id)) })
        }
        _ => json!({ "error": "usage: /mek qio <x> <y> <z> freq [<name> [public|private|trusted]]" }),
    }
}

/// `/mek probe` on a QIO block: its frequency and that frequency's drives and items.
pub(super) fn probe(game: &mut Game<'_>, pos: [i32; 3], id: &str, nbt: &Compound, data: &[i32], items: &[Option<ItemStack>]) -> Value {
    let freq = frequency_of(nbt);
    let dim = game.native_dimension().to_owned();
    let listed = freq.as_ref().and_then(|f| super::mek_frequency::read(|r| r.find(KIND, f).map(|q| (q.owner_name.clone(), q.color))));
    let mut out = json!({ "pos": pos, "id": id, "frequency": freq.as_ref().map(|f| json!({ "name": f.name, "public": f.security == Security::Public,
        "security": f.security.name(), "owner": listed.as_ref().map(|l| l.0.clone()), "color": listed.as_ref().map(|l| COLORS[usize::from(l.1) % COLORS.len()].0) })) });
    if let Some(f) = &freq {
        find_freq(f, |q| {
            out["total"] = json!(q.storage.total);
            out["capacity"] = json!(q.storage.count_capacity());
            out["types"] = json!(q.storage.types.len());
            out["type_capacity"] = json!(q.storage.type_capacity());
            out["drives"] = json!(q.storage.drives.len());
            out["items"] = Value::Array(q.storage.types.iter().map(|t| json!({ "id": t.key.0.item.as_ref(), "count": t.count })).collect());
        });
    }
    if id == DRIVE_ARRAY {
        out["slots"] = Value::Array(
            items
                .iter()
                .take(DRIVE_SLOTS)
                .enumerate()
                .filter_map(|(i, s)| {
                    let s = s.as_ref()?;
                    let (d, _) = read_drive(s)?;
                    Some(json!({ "slot": i, "item": s.item.as_ref(), "count": d.count, "types": d.items.len() }))
                })
                .collect(),
        );
        out["registered"] = json!(ARRAYS.with(|a| a.borrow().get(&(dim, pos)).map_or(0, |r| r.iter().flatten().count())));
        out["lights"] = Value::Array(lights_of(nbt).into_iter().map(|l| json!(l.name())).collect());
    } else {
        let v = unpack(data);
        out["viewer"] = json!({ "has": v.has, "total": v.total, "types": v.items.len(), "words": data.len() });
    }
    out
}

/// The drive's tooltip (`ItemQIODrive.appendHoverText`): its items and types against its tier's.
pub(super) fn item_tooltip(stack: &ItemStack, lines: &mut Vec<(Text, u32)>) -> bool {
    let Some(tier) = DriveTier::of_item(&stack.item) else {
        return false;
    };
    let data = drive_data(stack).unwrap_or_default();
    let count = number(data.get(&key("count"))).unwrap_or(0);
    let types = number(data.get(&key("types"))).unwrap_or(0);
    let at = lines.len().min(1);
    lines.insert(at, (Text::legacy(&format!("§7Used Item Capacity: §f{count} / {}", tier.max_count())), u32::MAX));
    lines.insert(at + 1, (Text::legacy(&format!("§7Used Type Capacity: §f{types} / {}", tier.max_types())), u32::MAX));
    true
}

// ---- client: the screens ---------------------------------------------------------------------------------------

const WIDTH: f32 = 176.0;
const DASH_HEIGHT: f32 = 222.0;
const ARRAY_HEIGHT: f32 = 194.0;
const COLS: usize = 8;
const ROWS: usize = 4;
const GRID: [f32; 2] = [8.0, 43.0];
const SEARCH_FIELD: Rect = [8.0, 28.0, 124.0, 12.0];
const SORT_BUTTON: Rect = [134.0, 28.0, 34.0, 12.0];
const SCROLLBAR: Rect = [156.0, 43.0, 12.0, 72.0];
const FREQ_TAB: Rect = [-26.0, 6.0, 26.0, 26.0];
const SORTS: [&str; 3] = ["Name", "Size", "Mod"];

// `GuiQIOTileFrequencySelect` (176 x 177, `GuiFrequencySelector` from y 17 at x 27): the back button, the public,
// trusted and private buttons, the list (4 rows of 10), the frequency's name, owner and mode, the name field with its
// checkmark, Set, Delete and the colour button.
const SELECT_HEIGHT: f32 = 177.0;
const BACK: Rect = [6.0, 6.0, 14.0, 14.0];
const MODE_BUTTONS: [(Security, &str, Rect); 3] = [
    (Security::Public, "button/public", [27.0, 17.0, 38.0, 20.0]),
    (Security::Trusted, "button/trusted", [69.0, 17.0, 38.0, 20.0]),
    (Security::Private, "button/private", [111.0, 17.0, 38.0, 20.0]),
];
const LIST_BOX: Rect = [27.0, 39.0, 122.0, 42.0];
const LIST_ROWS: usize = 4;
const NAME_FIELD: Rect = [50.0, 116.0, 98.0, 11.0];
const CHECK: Rect = [137.0, 116.0, 11.0, 11.0];
const SET_BUTTON: Rect = [27.0, 130.0, 50.0, 18.0];
const DELETE_BUTTON: Rect = [79.0, 130.0, 50.0, 18.0];
const COLOR_BUTTON: Rect = [132.0, 131.0, 16.0, 16.0];

/// `native_screen`: the array's drive slots (`QIODriveSlot` at 34 + 18 column, 70 + 18 row) or the dashboard's
/// viewer over the inventory; the search bar takes the keys at once (`qioAutoFocusSearchBar`).
pub(super) fn screen(block: &str) -> NativeScreen {
    SELECT.with(|s| s.set(false));
    SCROLL.with(|s| s.set(0));
    if block == DRIVE_ARRAY {
        FOCUS.with(|f| f.set(0));
        let mut slots: Vec<Option<[f32; 2]>> = (0..DRIVE_SLOTS).map(|i| Some([34.0 + (i % 6) as f32 * 18.0, 70.0 + (i / 6) as f32 * 18.0])).collect();
        slots.extend([None; 6]);
        return NativeScreen { width: WIDTH, height: ARRAY_HEIGHT, slots, inventory_y: ARRAY_HEIGHT - 82.0, inventory_x: 8.0, own_labels: true, own_background: false };
    }
    FOCUS.with(|f| f.set(1));
    NativeScreen { width: WIDTH, height: DASH_HEIGHT, slots: vec![None; 9], inventory_y: DASH_HEIGHT - 82.0, inventory_x: 8.0, own_labels: true, own_background: false }
}

/// The types the viewer shows, by their index in the frequency: those the search matches (`SearchQueryParser`:
/// words in the name, `@mod`), in the chosen order (`ListSortType` name, size or mod; ascending or not).
fn shown(v: &View) -> Vec<usize> {
    let query = SEARCH.with(|s| s.borrow().to_lowercase());
    let mut out: Vec<usize> = (0..v.items.len())
        .filter(|&i| {
            let (id, _) = &v.items[i];
            let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
            query.split_whitespace().all(|w| match w.strip_prefix('@') {
                Some(m) => ns.contains(m),
                None => path.replace('_', " ").contains(w) || path.contains(w),
            })
        })
        .collect();
    let path = |i: usize| v.items[i].0.split_once(':').map_or(v.items[i].0.clone(), |(_, p)| p.to_owned());
    match SORT.with(Cell::get) {
        1 => out.sort_by(|&a, &b| v.items[b].1.cmp(&v.items[a].1).then_with(|| path(a).cmp(&path(b)))),
        2 => out.sort_by(|&a, &b| v.items[a].0.cmp(&v.items[b].0)),
        _ => out.sort_by_key(|&i| path(i)),
    }
    if DESCENDING.with(Cell::get) {
        out.reverse();
    }
    out
}

fn max_scroll(n: usize) -> usize {
    n.div_ceil(COLS).saturating_sub(ROWS)
}

fn cell_at(at: [f32; 2]) -> Option<usize> {
    let (x, y) = (at[0] - GRID[0], at[1] - GRID[1]);
    if x < 0.0 || y < 0.0 || x >= COLS as f32 * 18.0 || y >= ROWS as f32 * 18.0 {
        return None;
    }
    Some((y / 18.0) as usize * COLS + (x / 18.0) as usize)
}

pub(super) fn click(game: &Game<'_>, block: &str, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let v = unpack(data);
    FOCUS.with(|f| f.set(0));
    if mek_ui::inside(FREQ_TAB, at) {
        // `GuiQIOFrequencyTab`: the select screen, in the frequency's mode.
        SELECT.with(|s| s.set(true));
        MODE.with(|m| m.set(if v.has { v.security } else { 0 }));
        ROW.with(|r| r.set(None));
        LIST_SCROLL.with(|s| s.set(0));
        LIST_AGE.with(|a| a.set(u32::MAX));
        return;
    }
    if block != DASHBOARD {
        return;
    }
    if mek_ui::inside(SEARCH_FIELD, at) {
        FOCUS.with(|f| f.set(1));
        if right {
            SEARCH.with(|s| s.borrow_mut().clear());
        }
        return;
    }
    if mek_ui::inside(SORT_BUTTON, at) {
        if right {
            DESCENDING.with(|d| d.set(!d.get()));
        } else {
            SORT.with(|s| s.set((s.get() + 1) % 3));
        }
        return;
    }
    let list = shown(&v);
    if mek_ui::inside(SCROLLBAR, at) {
        let half = at[1] < SCROLLBAR[1] + SCROLLBAR[3] / 2.0;
        SCROLL.with(|s| s.set(if half { s.get().saturating_sub(ROWS) } else { (s.get() + ROWS).min(max_scroll(list.len())) }));
        return;
    }
    if let Some(cell) = cell_at(at) {
        let index = list.get(SCROLL.with(Cell::get) * COLS + cell).map_or(-1, |&i| i as i32);
        let mode = if game.native_has_shift_down() && !right { 2 } else { i32::from(right) };
        *button = Some(VIEWER + (index + 1) * 4 + mode);
    }
}

fn send_name(game: &Game<'_>) {
    let Some(pos) = game.native_screen_pos() else { return };
    let name: String = NAME.with(|n| n.borrow().trim().chars().take(NAME_MAX).collect());
    if name.is_empty() {
        return;
    }
    let mut msg = vec![1u8];
    for c in pos {
        msg.extend(c.to_le_bytes());
    }
    msg[0] = OP_TYPED;
    msg.push(MODE.with(Cell::get));
    msg.extend(name.as_bytes());
    OUTBOX.with(|o| o.borrow_mut().push(msg));
    NAME.with(|n| n.borrow_mut().clear());
    LIST_AGE.with(|a| a.set(u32::MAX));
}

/// A message to the server about the open screen's block: `[op, x, y, z, rest...]`.
fn send(game: &Game<'_>, op: u8, rest: &[u8]) {
    let Some(pos) = game.native_screen_pos() else { return };
    let mut msg = vec![op];
    for c in pos {
        msg.extend(c.to_le_bytes());
    }
    msg.extend(rest);
    OUTBOX.with(|o| o.borrow_mut().push(msg));
    LIST_AGE.with(|a| a.set(u32::MAX));
}

/// `native_tick` on the client: the select screen's messages go to the server, and its list is asked for again
/// every half second while it shows.
pub(super) fn client_tick(game: &mut Game<'_>) {
    if SELECT.with(Cell::get) && OPEN.with(Cell::get).is_some() && game.native_screen_pos() == OPEN.with(Cell::get) {
        let age = LIST_AGE.with(|a| {
            let n = a.get().saturating_add(1);
            a.set(n);
            n
        });
        if age >= 10 {
            LIST_AGE.with(|a| a.set(0));
            send(game, OP_LIST, &[MODE.with(Cell::get)]);
        }
    }
    let out = OUTBOX.with(|o| std::mem::take(&mut *o.borrow_mut()));
    for msg in out {
        if let Err(e) = game.native_net_send_to_server(CHANNEL, &msg) {
            log::warn!("[mekanism] the QIO frequency wasn't sent: {e}");
        }
    }
}

fn typing(game: &Game<'_>) -> u8 {
    let open = OPEN.with(Cell::get);
    if open.is_none() || game.native_screen_pos() != open {
        return 0;
    }
    // The search bar is the viewer's, the name field the select screen's.
    match (FOCUS.with(Cell::get), SELECT.with(Cell::get)) {
        (1, false) => 1,
        (2, true) => 2,
        _ => 0,
    }
}

/// `native_screen_char_typed_pre`: a character for the focused field.
pub(super) fn char_typed(game: &Game<'_>, c: char, canceled: &mut bool) {
    let focus = typing(game);
    if focus == 0 || c.is_control() {
        return;
    }
    *canceled = true;
    let target = if focus == 1 { &SEARCH } else { &NAME };
    target.with(|s| {
        let mut s = s.borrow_mut();
        if s.chars().count() < if focus == 1 { 50 } else { NAME_MAX } {
            s.push(c);
        }
    });
    SCROLL.with(|s| s.set(0));
}

/// `native_screen_key_pressed_pre`: Backspace, Enter (sets the typed frequency) and Escape for the focused field;
/// every other key stays the field's, so the inventory key doesn't close the screen.
pub(super) fn key_pressed(game: &Game<'_>, key_name: &str, canceled: &mut bool) {
    let focus = typing(game);
    if focus == 0 || key_name == "key.keyboard.escape" {
        return;
    }
    *canceled = true;
    let target = if focus == 1 { &SEARCH } else { &NAME };
    match key_name {
        "key.keyboard.backspace" => {
            target.with(|s| s.borrow_mut().pop());
        }
        "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
            if focus == 2 {
                send_name(game);
            }
            FOCUS.with(|f| f.set(0));
        }
        _ => {}
    }
}

/// `native_screen_mouse_scrolled_pre`: the wheel scrolls the viewer a row a notch.
pub(super) fn scrolled(game: &Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    let open = OPEN.with(Cell::get);
    if open.is_none() || game.native_screen_pos() != open {
        return;
    }
    if SELECT.with(Cell::get) {
        let max = listed().len().saturating_sub(LIST_ROWS);
        LIST_SCROLL.with(|s| s.set(if scroll[1] > 0.0 { s.get().saturating_sub(1) } else { (s.get() + 1).min(max) }));
        *canceled = true;
        return;
    }
    let max = MAX_SCROLL.with(Cell::get);
    SCROLL.with(|s| {
        let now = s.get();
        s.set(if scroll[1] > 0.0 { now.saturating_sub(1) } else { (now + 1).min(max) });
    });
    *canceled = true;
}

thread_local! {
    /// The scroll limit the last frame drew.
    static MAX_SCROLL: Cell<usize> = const { Cell::new(0) };
}

fn field(pen: &mut Pen<'_, '_>, r: Rect, text: &str, focused: bool, hint: &str) {
    pen.fill(r, 0xFFA0A0A0);
    pen.fill([r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], 0xFF000000);
    let shown = if text.is_empty() && !focused { hint.to_owned() } else { format!("{text}{}", if focused { "_" } else { "" }) };
    pen.text(&shown, [r[0] + 3.0, r[1] + 2.0], if text.is_empty() && !focused { 0x707070 } else { 0xE0E0E0 });
}

pub(super) fn draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    OPEN.with(|o| o.set(game.native_screen_pos()));
    LAST_DATA.with(|d| {
        let mut d = d.borrow_mut();
        d.clear();
        d.extend_from_slice(data);
    });
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let height = if block == DASHBOARD { DASH_HEIGHT } else { ARRAY_HEIGHT };
    let mut pen = Pen { game, batch, origin, cursor };
    let v = unpack(data);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, WIDTH, height]);
    pen.holder(FREQ_TAB, true);
    pen.whole("frequency", [18.0, 18.0], [FREQ_TAB[0] + 4.0, FREQ_TAB[1] + 4.0, 18.0, 18.0]);
    let title = game.native_screen_title().unwrap_or_else(|| if block == DASHBOARD { "QIO Dashboard".into() } else { "QIO Drive Array".into() });
    pen.centered(&title, 0.0, WIDTH, 5.0, mek_ui::TITLE);
    pen.text("Inventory", [8.0, height - 93.0], mek_ui::TITLE);
    for row in 0..4 {
        let y = height - 83.0 + if row == 3 { 58.0 } else { 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    if block == DRIVE_ARRAY {
        for i in 0..DRIVE_SLOTS {
            pen.slot("normal", None, 33.0 + (i % 6) as f32 * 18.0, 69.0 + (i / 6) as f32 * 18.0);
        }
    }
    let mode = Security::from_index(v.security).unwrap_or(Security::Public);
    let freq_line = if v.has { format!("Frequency: {} ({})", v.name, mode.name()) } else { "No frequency".to_owned() };
    pen.text(&freq_line, [8.0, 17.0], mek_ui::TITLE);
    if block == DRIVE_ARRAY {
        pen.text(&format!("Items: {} / {}", qio::short_count(v.total), qio::short_count(v.cap)), [8.0, 32.0], mek_ui::TITLE);
        pen.text(&format!("Types: {} / {}", v.types, v.type_cap), [8.0, 44.0], mek_ui::TITLE);
        return;
    }
    let focus = FOCUS.with(Cell::get) == 1;
    SEARCH.with(|s| field(&mut pen, SEARCH_FIELD, &s.borrow(), focus, "Search..."));
    pen.button(SORT_BUTTON, true);
    let sort = SORTS[SORT.with(Cell::get) as usize % 3];
    pen.centered(&format!("{sort}{}", if DESCENDING.with(Cell::get) { "-" } else { "+" }), SORT_BUTTON[0], SORT_BUTTON[2], SORT_BUTTON[1] + 2.0, 0xFFFFFF);
    let list = shown(&v);
    let max = max_scroll(list.len());
    MAX_SCROLL.with(|m| m.set(max));
    let scroll = SCROLL.with(|s| {
        s.set(s.get().min(max));
        s.get()
    });
    for cell in 0..COLS * ROWS {
        let (x, y) = (GRID[0] + (cell % COLS) as f32 * 18.0, GRID[1] + (cell / COLS) as f32 * 18.0);
        pen.slot("normal", None, x - 1.0, y - 1.0);
        if let Some(&i) = list.get(scroll * COLS + cell) {
            let (id, count) = &v.items[i];
            let stack = ItemStack::new(id, 1);
            game.native_gui_draw_item_with(&mut *pen.batch, &stack, [origin[0] + x, origin[1] + y], Some(&qio::short_count(*count)));
        }
    }
    pen.fill(SCROLLBAR, 0xFF8B8B8B);
    let track = SCROLLBAR[3] - 15.0;
    let knob = if max == 0 { 0.0 } else { track * scroll as f32 / max as f32 };
    pen.fill([SCROLLBAR[0] + 1.0, SCROLLBAR[1] + 1.0 + knob, SCROLLBAR[2] - 2.0, 13.0], if max == 0 { 0xFFC6C6C6 } else { 0xFFFFFFFF });
    if v.has {
        let line = format!("{} / {} items, {} / {} types", qio::short_count(v.total), qio::short_count(v.cap), v.types, v.type_cap);
        pen.text(&line, [8.0, 118.0], mek_ui::TITLE);
    }
}

/// The viewer's tooltip: the type under the pointer and how many the frequency holds.
pub(super) fn tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if SELECT.with(Cell::get) {
        return;
    }
    if block != DASHBOARD {
        if mek_ui::inside(FREQ_TAB, at) {
            lines.push("Frequency".into());
        }
        return;
    }
    let v = unpack(data);
    if let Some(cell) = cell_at(at)
        && let Some(&i) = shown(&v).get(SCROLL.with(Cell::get) * COLS + cell)
    {
        let (id, count) = &v.items[i];
        lines.push(id.clone());
        lines.push(format!("Stored: {count}"));
    } else if mek_ui::inside(SORT_BUTTON, at) {
        lines.push("Sort: click to change, right-click to reverse".into());
    } else if mek_ui::inside(FREQ_TAB, at) {
        lines.push("Frequency".into());
    }
}

// ---- client: the frequency select screen ----------------------------------------------------------------------

/// The list the server sent for the mode shown.
fn listed() -> Vec<Listed> {
    let mode = MODE.with(Cell::get);
    LIST.with(|l| {
        let l = l.borrow();
        if l.0 == mode { l.1.clone() } else { Vec::new() }
    })
}

/// `native_net_from_server`: the frequencies of a mode this player may see (`SyncableFrequencyList`).
pub(super) fn from_server(channel: &str, data: &[u8]) {
    if channel != CHANNEL || data.len() < 4 || data[0] != OP_LIST {
        return;
    }
    let n = usize::from(u16::from_le_bytes([data[2], data[3]]));
    let mut at = 4;
    let mut out = Vec::with_capacity(n);
    let text = |at: &mut usize| -> Option<String> {
        let len = usize::from(*data.get(*at)?);
        let s = String::from_utf8_lossy(data.get(*at + 1..*at + 1 + len)?).into_owned();
        *at += 1 + len;
        Some(s)
    };
    for _ in 0..n {
        let Some(owner) = data.get(at..at + 16).and_then(|b| b.try_into().ok()).map(u128::from_le_bytes) else { break };
        let Some(&color) = data.get(at + 16) else { break };
        at += 17;
        let (Some(name), Some(owner_name)) = (text(&mut at), text(&mut at)) else { break };
        out.push(Listed { owner, color, name, owner_name });
    }
    LIST.with(|l| *l.borrow_mut() = (data[1], out));
}

fn select_shown(game: &Game<'_>) -> bool {
    let open = OPEN.with(Cell::get);
    SELECT.with(Cell::get) && open.is_some() && game.native_screen_pos() == open
}

/// `native_screen_mouse_button_pressed_pre`: every press while the select screen shows is its own (nothing under
/// it is clicked): back, a mode, a row, the name field and its checkmark, Set, Delete, the colour (left on, right
/// back).
pub(super) fn select_pressed(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if !select_shown(game) {
        return;
    }
    *canceled = true;
    let c = game.native_cursor();
    let at = [c[0] - screen.rect[0], c[1] - screen.rect[1]];
    let right = button == 1;
    FOCUS.with(|f| f.set(0));
    let list = listed();
    let picked = ROW.with(Cell::get).and_then(|r| list.get(r).cloned());
    let mode = MODE.with(Cell::get);
    let target = |l: &Listed| {
        let mut rest = vec![mode];
        rest.extend(l.owner.to_le_bytes());
        rest.extend(l.name.as_bytes());
        rest
    };
    if mek_ui::inside(BACK, at) {
        SELECT.with(|s| s.set(false));
    } else if let Some((m, _, _)) = MODE_BUTTONS.iter().find(|(_, _, r)| mek_ui::inside(*r, at)) {
        MODE.with(|x| x.set(m.index()));
        ROW.with(|r| r.set(None));
        LIST_SCROLL.with(|s| s.set(0));
        LIST_AGE.with(|a| a.set(u32::MAX));
    } else if mek_ui::inside(LIST_BOX, at) {
        let row = ((at[1] - LIST_BOX[1] - 1.0).max(0.0) / 10.0) as usize + LIST_SCROLL.with(Cell::get);
        ROW.with(|r| r.set((row < list.len()).then_some(row)));
    } else if mek_ui::inside(CHECK, at) {
        send_name(game);
    } else if mek_ui::inside(NAME_FIELD, at) {
        FOCUS.with(|f| f.set(2));
    } else if mek_ui::inside(SET_BUTTON, at) {
        if let Some(l) = &picked {
            send(game, OP_PICK, &target(l));
        }
    } else if mek_ui::inside(DELETE_BUTTON, at) {
        if let Some(l) = &picked {
            send(game, OP_DELETE, &target(l));
            ROW.with(|r| r.set(None));
        }
    } else if mek_ui::inside(COLOR_BUTTON, at) {
        send(game, OP_COLOR, &[u8::from(!right)]);
    }
}

/// `native_screen_render_post`: the select screen over its block's screen, which it darkens.
pub(super) fn draw_select(game: &Game<'_>, batch: &mut HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    if !select_shown(game) {
        return;
    }
    let origin = [screen.rect[0], screen.rect[1]];
    let c = game.native_cursor();
    let mut pen = Pen { game, batch, origin, cursor: [c[0] - origin[0], c[1] - origin[1]] };
    let v = LAST_DATA.with(|d| unpack(&d.borrow()));
    pen.fill([-28.0, -2.0, screen.rect[2] + 30.0, screen.rect[3] + 4.0], 0xC0101010);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, WIDTH, SELECT_HEIGHT]);
    pen.button(BACK, true);
    pen.whole("button/back", [14.0, 14.0], BACK);
    pen.centered("QIO Frequency Select", 0.0, WIDTH, 6.0, mek_ui::TITLE);
    let mode = MODE.with(Cell::get);
    for (m, icon, r) in MODE_BUTTONS {
        pen.button(r, m.index() != mode);
        pen.whole(icon, [40.0, 16.0], [r[0] - 1.0, r[1] + 2.0, 40.0, 16.0]);
    }
    // `GuiTextScrollList`.
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], LIST_BOX);
    let list = listed();
    let scroll = LIST_SCROLL.with(|s| {
        s.set(s.get().min(list.len().saturating_sub(LIST_ROWS)));
        s.get()
    });
    let row = ROW.with(Cell::get);
    for (i, l) in list.iter().enumerate().skip(scroll).take(LIST_ROWS) {
        let y = LIST_BOX[1] + 1.0 + 10.0 * (i - scroll) as f32;
        if row == Some(i) {
            pen.fill([LIST_BOX[0] + 1.0, y, LIST_BOX[2] - 8.0, 10.0], 0xFF898989);
        }
        let shown: String = l.name.chars().take(18).collect();
        pen.text(&shown, [LIST_BOX[0] + 3.0, y + 1.0], 0xFFFFFF);
    }
    if list.len() > LIST_ROWS {
        let track = LIST_BOX[3] - 2.0;
        let knob = (track * LIST_ROWS as f32 / list.len() as f32).max(6.0);
        let y = LIST_BOX[1] + 1.0 + (track - knob) * scroll as f32 / (list.len() - LIST_ROWS) as f32;
        pen.fill([LIST_BOX[0] + LIST_BOX[2] - 7.0, y, 6.0, knob], 0xFFC6C6C6);
    }
    let none = "None";
    let security = Security::from_index(v.security).unwrap_or(Security::Public);
    pen.text(&format!("Frequency: {}", if v.has { v.name.as_str() } else { none }), [27.0, 84.0], mek_ui::TITLE);
    pen.text(&format!("Owner: {}", if v.has && !v.owner.is_empty() { v.owner.as_str() } else { none }), [27.0, 94.0], mek_ui::TITLE);
    pen.text(&format!("Security: {}", if v.has { security.label() } else { none }), [27.0, 104.0], mek_ui::TITLE);
    let set_w = game.native_text_width("Set:");
    pen.text("Set:", [NAME_FIELD[0] - 2.0 - set_w, NAME_FIELD[1] + 2.0], mek_ui::TITLE);
    let focus = FOCUS.with(Cell::get) == 2;
    NAME.with(|n| field(&mut pen, [NAME_FIELD[0], NAME_FIELD[1], NAME_FIELD[2] - CHECK[2], NAME_FIELD[3]], &n.borrow(), focus, ""));
    pen.button(CHECK, true);
    pen.whole("button/checkmark", [12.0, 12.0], [CHECK[0], CHECK[1] - 0.5, 11.0, 11.0]);
    let picked = row.is_some_and(|r| r < list.len());
    pen.button(SET_BUTTON, picked);
    pen.centered("Set", SET_BUTTON[0], SET_BUTTON[2], SET_BUTTON[1] + 5.0, if picked { 0xFFFFFF } else { 0xA0A0A0 });
    pen.button(DELETE_BUTTON, picked);
    pen.centered("Delete", DELETE_BUTTON[0], DELETE_BUTTON[2], DELETE_BUTTON[1] + 5.0, if picked { 0xFFFFFF } else { 0xA0A0A0 });
    pen.slot("normal", None, COLOR_BUTTON[0] - 1.0, COLOR_BUTTON[1] - 1.0);
    if v.has {
        pen.fill(COLOR_BUTTON, 0xFF00_0000 | COLORS[usize::from(v.color) % COLORS.len()].1);
    }
}
