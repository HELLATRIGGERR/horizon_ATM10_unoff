// The quest book's server half (support module `crate::game::ftbq_world` of the
// `ftbquests` native bundle): loads the book from the player's own instance,
// keeps each player's progress in the world's folder, and works what a player's
// client asks for. What FTB Quests does per team, this does per player.
//
// All of it runs on the thread that ticks the world. The rules are in
// `ftbquests_core`; here is only what touches the game.

use super::{
    Game,
    native_lifecycle::WorldPhase,
    native_server::{NativeCommand, NativeServerPlayer},
};
use ftbquests_core::{
    book::{Book, Id, MatchComponents, RewardKind, TaskKind, id_string, parse_hex},
    filter::ItemFacts,
    progress::{Change, Progress, detected},
    snbt::{Compound, Value},
    wire::{MAX_ENTRIES, TO_CLIENT, TO_SERVER, ToClient, ToServer},
};
use horizon_client::{items::ItemStack, text::Text};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    hash::{BuildHasher, Hasher},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) const MOD_ID: &str = "ftbquests";
pub(super) const BOOK_ITEM: &str = "ftbquests:book";
/// How often the inventory is looked at for a change, in ticks.
const LOOK_EVERY: u32 = 10;

// ---- The book --------------------------------------------------------------

/// `config/ftbquests/quests` of this game, or of the ATM10 instance named by
/// `ATM10_DIR`: the player's own files, never a copy of ours.
fn quests_folder() -> Option<PathBuf> {
    let own = Game::native_config_dir().ok().map(|c| c.join("ftbquests/quests"));
    let atm10 = std::env::var_os("ATM10_DIR").map(|d| PathBuf::from(d).join("config/ftbquests/quests"));
    own.into_iter().chain(atm10).find(|p| p.join("data.snbt").is_file())
}

/// The book in a language, loaded once a run.
pub(super) fn book(locale: &str) -> Option<Arc<Book>> {
    static BOOKS: Mutex<Vec<(String, Option<Arc<Book>>)>> = Mutex::new(Vec::new());
    let mut books = BOOKS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, found)) = books.iter().find(|(l, _)| l == locale) {
        return found.clone();
    }
    let loaded = quests_folder().and_then(|folder| match Book::load(&folder, locale) {
        Ok(book) => {
            log::info!(
                "FTB Quests: {} chapters, {} quests from {}",
                book.chapters.len(),
                book.quests().count(),
                folder.display()
            );
            Some(Arc::new(book))
        }
        Err(e) => {
            log::error!("FTB Quests: can't read the quest files: {e}");
            None
        }
    });
    books.push((locale.to_owned(), loaded.clone()));
    loaded
}

/// What the server works on doesn't depend on the language.
fn server_book() -> Option<Arc<Book>> {
    book("en_us")
}

// ---- Players ---------------------------------------------------------------

#[derive(Default)]
struct Player {
    name: String,
    progress: Progress,
    dirty: bool,
    /// The client asked for its progress, so it gets every change from now on.
    listening: bool,
    inbox: Vec<ToServer>,
    outbox: Vec<Change>,
    /// Tasks whose items go in at the next inventory tick.
    handing_in: Vec<Id>,
    /// Quests just completed, to tell the player.
    completed_now: Vec<Id>,
    ticks: u32,
    /// What the last look at the inventory saw, and when to act on a change.
    seen: u64,
    detect_at: Option<u32>,
}

#[derive(Default)]
struct Server {
    players: HashMap<u128, Player>,
    /// Messages that came before the player's first tick.
    early: Vec<(u128, ToServer)>,
}

thread_local! {
    static SERVER: RefCell<Server> = RefCell::new(Server::default());
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn file_of(game: &Game<'_>, uuid: u128) -> Option<PathBuf> {
    Some(game.native_world_dir()?.join("ftbquests").join(format!("{uuid:032x}.snbt")))
}

fn load_player(game: &Game<'_>, uuid: u128, name: &str) -> Player {
    let mut player = Player { name: name.to_owned(), ..Player::default() };
    if let Some(path) = file_of(game, uuid)
        && let Ok(text) = fs::read_to_string(&path)
    {
        match Progress::from_snbt(&text) {
            Ok(progress) => player.progress = progress,
            // Kept, not overwritten: a player's progress is worth a look by hand.
            Err(e) => {
                log::error!("FTB Quests: {}: {e}", path.display());
                let _ = fs::rename(&path, path.with_extension("snbt.broken"));
            }
        }
    }
    player
}

fn save_player(game: &Game<'_>, uuid: u128, player: &mut Player) {
    if !player.dirty {
        return;
    }
    let Some(path) = file_of(game, uuid) else { return };
    let text = player.progress.to_snbt(&format!("{uuid:032x}"), &player.name);
    let tmp = path.with_extension("snbt.tmp");
    let written = path.parent().map_or(Ok(()), fs::create_dir_all).and_then(|()| fs::write(&tmp, text)).and_then(|()| fs::rename(&tmp, &path));
    match written {
        Ok(()) => player.dirty = false,
        Err(e) => log::error!("FTB Quests: can't save {}: {e}", path.display()),
    }
}

// ---- Items ----------------------------------------------------------------

/// A stack as the rules ask about it.
struct Facts<'a, 'g> {
    game: &'a Game<'g>,
    stack: &'a ItemStack,
}

impl ItemFacts for Facts<'_, '_> {
    fn id(&self) -> &str {
        &self.stack.item
    }
    fn in_tag(&self, tag: &str) -> bool {
        self.game.native_server_item_in_tag(self.stack, tag).unwrap_or(false)
    }
    fn is_block_item(&self) -> bool {
        self.game.native_item_is_block(&self.stack.item)
    }
    fn has_components(&self, _mode: &str, json: &str) -> bool {
        let Ok(serde_json::Value::Object(wanted)) = serde_json::from_str::<serde_json::Value>(json) else {
            return false;
        };
        let Some(found) = horizon_items::item_data::encode_components(self.stack) else {
            return false;
        };
        wanted.iter().all(|(key, value)| {
            let Some(tag) = found.get(key) else { return false };
            match value {
                serde_json::Value::String(s) => horizon_items::item_data::canon_tag(tag).trim_matches('"') == s,
                other => horizon_items::item_data::canon_tag(tag) == other.to_string(),
            }
        })
    }
}

/// SNBT for a compound of the quest files, as the game's parser reads it.
fn snbt_of(value: &Value, out: &mut String) {
    use std::fmt::Write;
    match value {
        Value::Byte(n) => drop(write!(out, "{n}b")),
        Value::Short(n) => drop(write!(out, "{n}s")),
        Value::Int(n) => drop(write!(out, "{n}")),
        Value::Long(n) => drop(write!(out, "{n}L")),
        Value::Float(n) => drop(write!(out, "{n}f")),
        Value::Double(n) => drop(write!(out, "{n}d")),
        Value::Bool(b) => out.push_str(if *b { "1b" } else { "0b" }),
        Value::Str(s) => {
            out.push('"');
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    _ => out.push(c),
                }
            }
            out.push('"');
        }
        Value::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                snbt_of(item, out);
            }
            out.push(']');
        }
        Value::Compound(c) => compound_snbt(c, out),
        Value::ByteArray(items) => array(out, 'B', items.iter().map(|n| format!("{n}b"))),
        Value::IntArray(items) => array(out, 'I', items.iter().map(i32::to_string)),
        Value::LongArray(items) => array(out, 'L', items.iter().map(|n| format!("{n}L"))),
    }
}

fn array(out: &mut String, kind: char, items: impl Iterator<Item = String>) {
    out.push('[');
    out.push(kind);
    out.push(';');
    out.push_str(&items.collect::<Vec<_>>().join(","));
    out.push(']');
}

fn compound_snbt(c: &Compound, out: &mut String) {
    out.push('{');
    for (i, (key, value)) in c.0.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(key);
        out.push_str("\":");
        snbt_of(value, out);
    }
    out.push('}');
}

/// A real stack of an item of the files.
fn make_stack(id: &str, components: Option<&Compound>, count: u32) -> Option<ItemStack> {
    let Some(components) = components else {
        return Some(ItemStack::new(id, count));
    };
    let mut text = format!("{{id:\"{id}\",count:{},components:", count.clamp(1, 99));
    compound_snbt(components, &mut text);
    text.push('}');
    match horizon_items::item_data::parse_stack(&text) {
        Ok(Some(mut stack)) => {
            stack.count = count;
            Some(stack)
        }
        Ok(None) => None,
        Err(e) => {
            log::warn!("FTB Quests: a stack of {id} didn't parse ({e}); giving it plain");
            Some(ItemStack::new(id, count))
        }
    }
}

/// `ItemMatchingSystem.doesItemMatch` by a task's `match_components`.
fn components_match(task: &ftbquests_core::book::ItemRef, mode: MatchComponents, stack: &ItemStack) -> bool {
    if mode == MatchComponents::None {
        return true;
    }
    let wanted = make_stack(&task.id, task.components.as_ref(), 1);
    let wanted = wanted.as_ref().and_then(horizon_items::item_data::encode_components);
    let found = horizon_items::item_data::encode_components(stack);
    let canon = |tag: &Option<horizon_world::nbt::Tag>| tag.as_ref().map(horizon_items::item_data::canon_tag);
    match mode {
        MatchComponents::Strict => canon(&wanted) == canon(&found),
        _ => {
            let Some(horizon_world::nbt::Tag::Compound(wanted)) = &wanted else { return true };
            wanted.iter().all(|(key, value)| {
                let Ok(name) = key.to_string() else { return false };
                found.as_ref().and_then(|f| f.get(&name)).map(horizon_items::item_data::canon_tag)
                    == Some(horizon_items::item_data::canon_tag(value))
            })
        }
    }
}

/// `ItemTask.test`.
fn task_takes(game: &Game<'_>, kind: &TaskKind, stack: &ItemStack) -> bool {
    let TaskKind::Item { item, filter, match_components, .. } = kind else { return false };
    if let Some(filter) = filter {
        return filter.matches(&Facts { game, stack });
    }
    *stack.item == *item.id && components_match(item, *match_components, stack)
}

/// The slots a player carries, by `native_inventory_tick`'s numbers.
fn carried_slots() -> impl Iterator<Item = i32> {
    (0..36).chain(100..=103).chain(std::iter::once(-106))
}

fn carried(game: &mut Game<'_>) -> Vec<ItemStack> {
    carried_slots().filter_map(|slot| game.native_carried_stack(slot).map(|s| s.clone())).collect()
}

/// A fingerprint of what a player carries.
fn fingerprint(stacks: &[ItemStack]) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    use std::hash::Hash;
    for s in stacks {
        s.item.hash(&mut hash);
        s.count.hash(&mut hash);
    }
    hash.finish()
}

/// `FTBQuestsInventoryListener.detect`: every task that doesn't take items,
/// against what the player carries now.
fn detect(game: &mut Game<'_>, book: &Book, player: &mut Player, stacks: &[ItemStack]) {
    let time = now();
    for chapter in &book.chapters {
        for quest in &chapter.quests {
            for task in &quest.tasks {
                let TaskKind::Item { consume, only_from_crafting, task_screen_only, .. } = &task.kind else { continue };
                if *task_screen_only
                    || only_from_crafting.get(false)
                    || book.consumes(chapter, *consume)
                    || player.progress.is_completed(task.id)
                    || !player.progress.can_start_tasks(book, chapter, quest)
                    || !player.progress.sequence_allows(quest, chapter, task.id)
                {
                    continue;
                }
                let found: u64 = stacks.iter().filter(|s| task_takes(game, &task.kind, s)).map(|s| u64::from(s.count)).sum();
                if let Some(value) = detected(task, &player.progress, found) {
                    let mut changes = Vec::new();
                    player.progress.set_task(book, task.id, value, time, &mut changes);
                    note(player, changes);
                }
            }
        }
    }
}

fn note(player: &mut Player, changes: Vec<Change>) {
    if !changes.is_empty() {
        player.dirty = true;
        for change in &changes {
            if let Change::Completed(id, _) = change {
                player.completed_now.push(*id);
            }
        }
        player.outbox.extend(changes);
    }
}

/// FTB Quests' toast, as a line over the hotbar: the last quest that completed.
fn tell_completed(game: &mut Game<'_>, book: &Book, uuid: u128, player: &mut Player) {
    let last = player
        .completed_now
        .drain(..)
        .filter_map(|id| book.quest(id))
        .filter(|q| !q.disable_toast)
        .last();
    if let Some(quest) = last {
        let label = game.native_translate("ftbquests.quest.completed", "Quest completed!");
        let raw = if quest.title.is_empty() { None } else { Some(quest.title.as_str()) };
        let name = raw.map(|t| ftbquests_core::text::plain(t, &|_| None));
        let line = name.map_or_else(|| label.clone(), |n| format!("{label} {n}"));
        game.native_action_bar(uuid, Text::literal(line));
    }
}

/// `ItemReward.claim`, for one player: gives the items, the rest at their feet.
fn give_reward(game: &mut Game<'_>, book: &Book, player: &mut Player, reward: Id) {
    if !player.progress.can_claim(book, reward) {
        return;
    }
    let Some((found, _)) = book.reward(reward) else { return };
    let RewardKind::Item { item, count, random_bonus, only_one } = &found.kind else { return };
    if !game.native_item_known(&item.id) {
        // A mod that isn't carried yet: the reward waits, so nothing is lost.
        if let Some(me) = game.native_acting_player() {
            game.native_action_bar(me.uuid, Text::literal(format!("{} isn't in the game yet; the reward waits", item.id)));
        }
        return;
    }
    if *only_one && carried(game).iter().any(|s| *s.item == *item.id) {
        return;
    }
    let bonus = if *random_bonus > 0 {
        let mut hash = std::collections::hash_map::RandomState::new().build_hasher();
        hash.write_u64(now());
        (hash.finish() % (*random_bonus as u64 + 1)) as i64
    } else {
        0
    };
    let total = (*count + bonus).max(1) as u32;
    if let Some(stack) = make_stack(&item.id, item.components.as_ref(), 1) {
        game.native_give_stack(&stack, total);
        let mut changes = Vec::new();
        player.progress.mark_claimed(reward, now(), &mut changes);
        note(player, changes);
    }
}

/// Works one message from the player's client, in their own tick.
fn handle(game: &mut Game<'_>, book: &Book, player: &mut Player, message: ToServer, stacks: &mut Vec<ItemStack>) {
    match message {
        ToServer::Hello => {
            player.listening = true;
            player.outbox.clear();
            let mut all = player.progress.entries();
            // The client forgets first; the rest are `Changes` as they come.
            player.outbox.append(&mut all);
            player.detect_at = Some(player.ticks);
            send_reset(game, player);
        }
        ToServer::Submit(id) => {
            let Some((task, _, chapter)) = book.task(id) else { return };
            match &task.kind {
                TaskKind::Checkmark => {
                    let mut changes = Vec::new();
                    player.progress.submit_checkmark(book, id, now(), &mut changes);
                    note(player, changes);
                }
                TaskKind::Item { consume, .. } if book.consumes(chapter, *consume) => {
                    if !player.handing_in.contains(&id) {
                        player.handing_in.push(id);
                    }
                }
                TaskKind::Item { .. } => {
                    *stacks = carried(game);
                    detect(game, book, player, stacks);
                }
                TaskKind::Other { .. } => {}
            }
        }
        ToServer::Claim(id) => give_reward(game, book, player, id),
        ToServer::ClaimAll => {
            let waiting: Vec<Id> = book
                .quests()
                .filter(|q| player.progress.is_completed(q.id))
                .flat_map(|q| q.rewards.iter())
                .filter(|r| r.is_supported() && !r.exclude_from_claim_all && !player.progress.is_claimed(r.id))
                .map(|r| r.id)
                .collect();
            for reward in waiting {
                give_reward(game, book, player, reward);
            }
        }
    }
}

fn send_reset(game: &mut Game<'_>, player: &Player) {
    let _ = player;
    if let Some(me) = game.native_acting_player() {
        let _ = game.native_net_send_to_player(me.uuid, TO_CLIENT, &ToClient::Reset.encode());
    }
}

fn flush(game: &mut Game<'_>, uuid: u128, player: &mut Player) {
    if !player.listening {
        player.outbox.clear();
        return;
    }
    while !player.outbox.is_empty() {
        let take = player.outbox.len().min(MAX_ENTRIES);
        let part: Vec<Change> = player.outbox.drain(..take).collect();
        if let Err(e) = game.native_net_send_to_player(uuid, TO_CLIENT, &ToClient::Changes(part.clone()).encode()) {
            // The allowance for this tick is spent: the rest go next tick.
            log::debug!("FTB Quests: {e}");
            player.outbox.splice(0..0, part);
            break;
        }
    }
}

// ---- Hooks -----------------------------------------------------------------

/// `native_player_tick`: messages, the inventory's changes, and the save.
pub(super) fn player_tick(game: &mut Game<'_>) {
    let Some(me) = game.native_acting_player() else { return };
    let Some(book) = server_book() else { return };
    let mut player = SERVER.with(|s| {
        let mut s = s.borrow_mut();
        s.players.remove(&me.uuid)
    });
    let mut player = player.take().unwrap_or_else(|| load_player(game, me.uuid, &me.name));
    player.ticks = player.ticks.wrapping_add(1);
    let early: Vec<ToServer> = SERVER.with(|s| {
        let mut s = s.borrow_mut();
        let mut mine = Vec::new();
        s.early.retain(|(uuid, m)| {
            if *uuid == me.uuid {
                mine.push(m.clone());
                false
            } else {
                true
            }
        });
        mine
    });
    player.inbox.extend(early);
    // What `native_inventory_tick` took in just now is done with.
    player.handing_in.clear();

    let mut stacks = Vec::new();
    for message in std::mem::take(&mut player.inbox) {
        handle(game, &book, &mut player, message, &mut stacks);
    }
    if player.ticks % LOOK_EVERY == 1 {
        stacks = carried(game);
        let seen = fingerprint(&stacks);
        if seen != player.seen {
            player.seen = seen;
            let delay = book.settings.detection_delay;
            let at = player.ticks.wrapping_add(delay);
            player.detect_at = Some(player.detect_at.map_or(at, |t| t.min(at)));
        }
    }
    if player.detect_at.is_some_and(|t| player.ticks.wrapping_sub(t) < u32::MAX / 2) {
        player.detect_at = None;
        if stacks.is_empty() {
            stacks = carried(game);
        }
        detect(game, &book, &mut player, &stacks);
    }
    tell_completed(game, &book, me.uuid, &mut player);
    flush(game, me.uuid, &mut player);
    save_player(game, me.uuid, &mut player);
    SERVER.with(|s| s.borrow_mut().players.insert(me.uuid, player));
}

/// `native_inventory_tick`: a task that takes items takes them here, where an
/// emptied slot clears itself (`ItemTask.submitTask`, main inventory only).
pub(super) fn inventory_tick(game: &mut Game<'_>, slot: i32) {
    if !(0..36).contains(&slot) {
        return;
    }
    let pending = SERVER.with(|s| s.borrow().players.values().any(|p| !p.handing_in.is_empty()));
    if !pending {
        return;
    }
    let Some(me) = game.native_acting_player() else { return };
    let Some(book) = server_book() else { return };
    let Some(mut player) = SERVER.with(|s| s.borrow_mut().players.remove(&me.uuid)) else { return };
    for id in player.handing_in.clone() {
        let Some((task, quest, chapter)) = book.task(id) else { continue };
        let TaskKind::Item { task_screen_only, .. } = &task.kind else { continue };
        if *task_screen_only
            || player.progress.is_completed(id)
            || !player.progress.can_start_tasks(&book, chapter, quest)
            || !player.progress.sequence_allows(quest, chapter, id)
        {
            continue;
        }
        let Some(stack) = game.native_carried_stack(slot).map(|s| s.clone()) else { break };
        if !task_takes(game, &task.kind, &stack) {
            continue;
        }
        let have = player.progress.task_count(id);
        let add = u64::from(stack.count).min(task.max_progress().saturating_sub(have));
        if add == 0 {
            continue;
        }
        if let Some(stack) = game.native_carried_stack(slot) {
            stack.count -= add as u32;
        }
        let mut changes = Vec::new();
        player.progress.set_task(&book, id, have + add, now(), &mut changes);
        note(&mut player, changes);
    }
    SERVER.with(|s| s.borrow_mut().players.insert(me.uuid, player));
}

/// `native_net_from_client`: kept for the player's own tick, where the helpers read them.
pub(super) fn from_client(player: u128, channel: &str, data: &[u8]) {
    if channel != TO_SERVER {
        return;
    }
    let Some(message) = ToServer::decode(data) else { return };
    SERVER.with(|s| {
        let mut s = s.borrow_mut();
        match s.players.get_mut(&player) {
            Some(p) => p.inbox.push(message),
            None => {
                // At most a few messages wait for a player's first tick.
                if s.early.len() < 256 {
                    s.early.push((player, message));
                }
            }
        }
    });
}

/// `QuestBookItem.use`: the book opens on the client that used it.
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if &*stack.item != BOOK_ITEM {
        return;
    }
    *claimed = true;
    if let Some(me) = game.native_acting_player() {
        let _ = game.native_net_send_to_player(me.uuid, TO_CLIENT, &ToClient::Open.encode());
    }
}

/// A player left: save and forget them, so a save that comes later reads their file.
pub(super) fn logged_out(game: &mut Game<'_>, player: u128) {
    let Some(mut state) = SERVER.with(|s| s.borrow_mut().players.remove(&player)) else { return };
    save_player(game, player, &mut state);
}

/// `native_world_lifecycle`: nothing of one world stays for the next.
pub(super) fn lifecycle(game: &mut Game<'_>, phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenReady) {
        let players = SERVER.with(|s| {
            let mut s = s.borrow_mut();
            s.early.clear();
            std::mem::take(&mut s.players)
        });
        for (uuid, mut player) in players {
            save_player(game, uuid, &mut player);
        }
    }
}

pub(super) fn commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::new("ftbquests open_book"));
    commands.push(NativeCommand::new("ftbquests status <id:word>"));
}

/// `native_command`: the same when the server's console runs them for a player
/// (`/execute`, a test's control socket), with that player swapped in.
pub(super) fn console(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    if game.native_window() {
        return;
    }
    if let Some(me) = game.native_acting_player() {
        perform(game, &me, command, ran);
    }
}

/// `/ftbquests open_book` (as the mod has) and `/ftbquests status <id>`, a line
/// saying where a quest, task or reward stands.
pub(super) fn perform(game: &mut Game<'_>, player: &NativeServerPlayer, command: &str, ran: &mut bool) {
    let mut words = command.split_whitespace();
    if words.next() != Some("ftbquests") {
        return;
    }
    match words.next() {
        Some("open_book") => {
            *ran = true;
            let _ = game.native_net_send_to_player(player.uuid, TO_CLIENT, &ToClient::Open.encode());
        }
        Some("status") => {
            *ran = true;
            let id = words.next().and_then(parse_hex).unwrap_or(0);
            let progress = SERVER.with(|s| s.borrow().players.get(&player.uuid).map(|p| p.progress.clone()));
            let progress = progress.unwrap_or_else(|| load_player(game, player.uuid, &player.name).progress);
            let line = format!(
                "ftbquests {}: count={} started={} completed={} claimed={}",
                id_string(id),
                progress.task_count(id),
                progress.is_started(id),
                progress.is_completed(id),
                progress.is_claimed(id),
            );
            game.native_feedback(Text::literal(line));
        }
        _ => {}
    }
}
