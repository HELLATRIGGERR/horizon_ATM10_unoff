// FTB Quests' book screen (support module `crate::game::ftbquests_book` of the `ftbquests` native bundle), the
// mod screen `ftbquests`: the chapters by group down the left, the chosen chapter's quests on a grid that a drag
// pans and the wheel zooms (dependency lines, item icons, the state of each quest), and a quest's view over it
// (its text, its tasks with their progress and the checkmark button, its rewards with the claim button, a choice
// reward's picker), and "take all" over the grid. The server's notices (`ftbquests:notify`) become toasts: a task,
// quest or chapter completed, what a claim gave, a toast reward.
// It opens from the server (the book item, `/ftbquests open_book`) or with the quests key (unbound, as in FTB
// Quests). The team's progress comes from the server (`ftbquests:sync_team_data`); `/ftbquestsbook` (FTBTEAMS_DEV=1)
// puts `FTBQB {json}` in chat for the tests. FTB Quests is All Rights Reserved: written from its behaviour, drawn
// with fills and the game's item icons; labels are translation keys with our own fallback wording.

use super::{Game, ftbquests_world};
use ftbquests_core::{
    CLAIM_ALL, CLAIM_REWARD, NOTIFY, Notice, OPEN_BOOK, Request, SUBMIT_TASK, SYNC, Sync,
    file::{AutoClaim, QuestFile, Reward, RewardKind, TaskKind, code, parse_id},
};
use horizon_client::{hud::HudBatch, text::Text};
use mod_api::{Command, ScreenView};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

/// The width of `text` in GUI pixels (upstream's `native_text_width` takes the batch it would draw into).
fn text_width(game: &Game<'_>, text: &str) -> f32 {
    horizon_client::hud::HudBatch::text_width(&game.gui.atlas, text)
}

const MOD: &str = "ftbquests";
pub(super) const SCREEN: &str = "ftbquests";
const PANEL: f32 = 130.0;
const ROW: f32 = 14.0;
/// A quest of size 1 at zoom 1, in GUI pixels.
const CELL: f32 = 24.0;

fn tr(key: &str, fallback: &str, args: &[String]) -> String {
    let args: Vec<Text> = args.iter().map(|a| Text::literal(a.clone())).collect();
    Text::translate_with_fallback(key, fallback, &args).plain()
}
fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}
fn dev() -> bool {
    std::env::var("FTBTEAMS_DEV").is_ok_and(|v| v == "1")
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Chapter(usize),
    Quest(u64),
    Check(u64),
    Claim(u64),
    CloseView,
    /// Opens a choice reward's picker.
    Choose(u64),
    /// The picker's entry `.1` for the choice reward `.0`.
    Pick(u64, usize),
    ClosePicker,
    ClaimAll,
}

#[derive(Default)]
struct Book {
    /// The team's progress as the server sent it.
    team: String,
    tasks: HashMap<u64, i64>,
    completed: HashSet<u64>,
    claimed: HashSet<u64>,
    blocked: bool,
    syncs: u32,
    /// The server's notices, shown as toasts on the next frame; the toasts' titles, for the tests.
    notices: Vec<Notice>,
    toasts: Vec<String>,
    // The screen.
    chapter: usize,
    centered: Option<usize>,
    pan: [f32; 2],
    zoom: f32,
    view: Option<u64>,
    picking: Option<u64>,
    scroll: f32,
    rect: [f32; 4],
    last_mouse: Option<[f32; 2]>,
    hits: Vec<([f32; 4], Action)>,
    pending: Vec<Action>,
    open_requested: bool,
    sent: Vec<String>,
    drawn: usize,
}

thread_local! {
    static BOOK: RefCell<Book> = RefCell::new(Book { zoom: 1.0, ..Book::default() });
}

pub(super) fn screen_open(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(SCREEN)
}

fn open(game: &mut Game<'_>) -> bool {
    if ftbquests_world::quest_file().is_none() || !game.native_mod_screen_open(SCREEN) {
        return false;
    }
    BOOK.with(|b| {
        let mut b = b.borrow_mut();
        b.view = None;
        b.picking = None;
    });
    true
}

/// `native_net_from_server`: the team's progress, and the server's word to open the book.
pub(super) fn from_server(channel: &str, data: &[u8]) {
    match channel {
        SYNC => match serde_json::from_slice::<Sync>(data) {
            Ok(part) => BOOK.with(|b| {
                let mut b = b.borrow_mut();
                if part.reset {
                    b.tasks.clear();
                    b.completed.clear();
                    b.claimed.clear();
                }
                b.team = part.team;
                b.blocked = part.rewards_blocked;
                b.tasks.extend(part.tasks.iter().filter_map(|(k, v)| Some((parse_id(k)?, *v))));
                b.completed.extend(part.completed.iter().filter_map(|k| parse_id(k)));
                b.claimed.extend(part.claimed.iter().filter_map(|k| parse_id(k)));
                b.syncs += 1;
            }),
            Err(e) => log::warn!("[ftbquests] a bad {SYNC} message: {e}"),
        },
        OPEN_BOOK => BOOK.with(|b| b.borrow_mut().open_requested = true),
        NOTIFY => match serde_json::from_slice::<Notice>(data) {
            Ok(n) => BOOK.with(|b| b.borrow_mut().notices.push(n)),
            Err(e) => log::warn!("[ftbquests] a bad {NOTIFY} message: {e}"),
        },
        _ => {}
    }
}

/// A notice's toast: (icon, title, line, challenge frame).
fn toast_of(file: &QuestFile, n: &Notice) -> Option<(String, String, String, bool)> {
    let id = parse_id(&n.id).unwrap_or(0);
    let xp = "minecraft:experience_bottle".to_owned();
    Some(match n.kind.as_str() {
        "quest" => (file.quest_icon(file.quests.get(&id)?).unwrap_or_default(), tr("ftbquests.port.toast.quest", "Quest done", &[]), quest_title(file, id), false),
        "chapter" => {
            let ch = file.chapter(id)?;
            let name = file.text("chapter", id, "title").unwrap_or_else(|| ch.filename.clone());
            (ch.icon.clone().unwrap_or_default(), tr("ftbquests.port.toast.chapter", "Chapter done", &[]), name, true)
        }
        "task" => {
            let t = file.tasks.get(&id)?;
            let (icon, label) = match &t.kind {
                TaskKind::Item { item, count, .. } => (item.clone(), format!("{count} x {}", item_name(item))),
                _ => (String::new(), file.text("task", id, "title").unwrap_or_else(|| quest_title(file, t.quest))),
            };
            (icon, tr("ftbquests.port.toast.task", "Task done", &[]), label, false)
        }
        "reward" => {
            let mut parts: Vec<String> = n.items.iter().map(|(i, c)| format!("{c} x {}", item_name(i))).collect();
            if n.xp > 0 {
                parts.push(tr("ftbquests.port.reward.xp", "%s experience", &[n.xp.to_string()]));
            }
            if n.levels != 0 {
                parts.push(tr("ftbquests.port.reward.levels", "%s levels of experience", &[n.levels.to_string()]));
            }
            let icon = n.items.first().map_or(xp, |i| i.0.clone());
            (icon, tr("ftbquests.port.toast.reward", "Rewards received", &[]), parts.join(", "), false)
        }
        "toast" => {
            let r = file.rewards.get(&id)?;
            let title = file.text("reward", id, "title").unwrap_or_else(|| quest_title(file, r.quest));
            let icon = r.icon.clone().or_else(|| file.quests.get(&r.quest).and_then(|q| file.quest_icon(q))).unwrap_or_default();
            (icon, title, n.text.clone(), false)
        }
        _ => return None,
    })
}

/// `native_key`: the quests key opens and closes the book (unbound unless the player binds it, as in FTB Quests).
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed {
        return;
    }
    let book_key = game.native_mod_key(MOD, "quests", "key.keyboard.unknown");
    if book_key == "key.keyboard.unknown" || key != book_key {
        return;
    }
    if screen_open(game) {
        game.native_mod_screen_close(SCREEN);
    } else if game.native_playing() {
        open(game);
    }
}

/// `native_frame`: opens the book when the server said so; sends what the clicks asked for.
pub(super) fn frame(game: &mut Game<'_>) {
    let (open_now, actions, notices) = BOOK.with(|b| {
        let mut b = b.borrow_mut();
        (std::mem::take(&mut b.open_requested), std::mem::take(&mut b.pending), std::mem::take(&mut b.notices))
    });
    if open_now && !screen_open(game) {
        open(game);
    }
    if let Some(file) = ftbquests_world::quest_file() {
        for n in &notices {
            let Some((icon, title, line, challenge)) = toast_of(&file, n) else { continue };
            let id = format!("ftbquests:{}/{}", n.kind, n.id);
            if game.native_add_toast(&id, &icon, Text::literal(title.clone()), Text::literal(line), challenge) {
                BOOK.with(|b| b.borrow_mut().toasts.push(format!("{}: {title}", n.kind)));
            }
        }
    }
    for action in actions {
        let (channel, id, index) = match action {
            Action::Check(id) => (SUBMIT_TASK, id, None),
            Action::Claim(id) => (CLAIM_REWARD, id, None),
            Action::Pick(id, i) => (CLAIM_REWARD, id, Some(i)),
            Action::ClaimAll => (CLAIM_ALL, 0, None),
            _ => continue,
        };
        let data = serde_json::to_vec(&Request { id: code(id), index }).unwrap_or_default();
        match game.native_net_send_to_server(channel, &data) {
            Ok(()) => BOOK.with(|b| b.borrow_mut().sent.push(format!("{channel} {}", code(id)))),
            Err(e) => log::debug!("[ftbquests] {channel}: {e}"),
        }
    }
}

/// What "take all" would take: the rewards of completed quests not yet claimed, but the excluded ones and the
/// choices (the server decides; this is the button's count).
fn claim_all_count(file: &QuestFile, b: &Book) -> usize {
    if b.blocked {
        return 0;
    }
    file.rewards
        .values()
        .filter(|r| !r.exclude_from_claim_all && b.completed.contains(&r.quest) && !b.claimed.contains(&r.id))
        .filter(|r| !matches!(r.kind, RewardKind::Choice { .. } | RewardKind::Other(_)))
        .count()
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Done,
    Open,
    Locked,
}

fn quest_state(file: &QuestFile, b: &Book, id: u64) -> State {
    let q = &file.quests[&id];
    if b.completed.contains(&id) {
        State::Done
    } else if q.dependencies.iter().all(|d| !file.quests.contains_key(d) || b.completed.contains(d)) {
        State::Open
    } else {
        State::Locked
    }
}

/// An item's name from the game's lang (`item.<ns>.<path>`, else the block's), else its id.
fn item_name(item: &str) -> String {
    let (ns, path) = item.split_once(':').unwrap_or(("minecraft", item));
    for kind in ["item", "block"] {
        let key = format!("{kind}.{ns}.{}", path.replace('/', "."));
        let name = Text::translate_with_fallback(&key, "", &[]).plain();
        if !name.is_empty() {
            return name;
        }
    }
    item.to_owned()
}

pub(super) fn quest_title(file: &QuestFile, id: u64) -> String {
    if let Some(t) = file.text("quest", id, "title") {
        return t;
    }
    let q = &file.quests[&id];
    match q.tasks.first().and_then(|t| file.tasks.get(t)).map(|t| &t.kind) {
        Some(TaskKind::Item { item, .. }) => item_name(item),
        _ => tr("ftbquests.port.unnamed_quest", "A quest", &[]),
    }
}

/// `text` cut to `width`, with an ellipsis when cut.
fn fit(game: &Game<'_>, text: &str, width: f32) -> String {
    if text_width(game, text) <= width {
        return text.to_owned();
    }
    let mut out: String = text.to_owned();
    while !out.is_empty() && text_width(game, &format!("{out}...")) > width {
        out.pop();
    }
    format!("{}...", out.trim_end())
}

/// A reward's row in the quest view: its icon and line (its own title from the lang first).
fn reward_line(file: &QuestFile, r: &Reward) -> (Option<String>, String) {
    let title = file.text("reward", r.id, "title");
    let table = |t: &u64| file.reward_tables.get(t).and_then(|t| t.icon.clone()).or_else(|| Some("minecraft:chest".into()));
    let (icon, line) = match &r.kind {
        RewardKind::Item { item, count, random_bonus, .. } => {
            let n = if *random_bonus > 0 { format!("{count}-{}", count + random_bonus) } else { count.to_string() };
            (Some(item.clone()), format!("{n} x {}", item_name(item)))
        }
        RewardKind::Xp { points } => (Some("minecraft:experience_bottle".into()), tr("ftbquests.port.reward.xp", "%s experience", &[points.to_string()])),
        RewardKind::XpLevels { levels } => (Some("minecraft:experience_bottle".into()), tr("ftbquests.port.reward.levels", "%s levels of experience", &[levels.to_string()])),
        RewardKind::Random { table: t } => (table(t), tr("ftbquests.port.reward.random", "A random pick from %s", &[file.table_title(*t)])),
        RewardKind::Loot { table: t } => {
            let n = file.reward_tables.get(t).map_or(1, |t| t.loot_size);
            (table(t), tr("ftbquests.port.reward.loot", "%s draws from %s", &[n.to_string(), file.table_title(*t)]))
        }
        RewardKind::Choice { table: t } => (table(t), tr("ftbquests.port.reward.choice", "Your pick from %s", &[file.table_title(*t)])),
        RewardKind::Command { .. } => (Some("minecraft:command_block".into()), tr("ftbquests.port.reward.command", "A server command", &[])),
        RewardKind::Toast { .. } => (Some("minecraft:paper".into()), tr("ftbquests.port.reward.toast", "A note in the corner", &[])),
        RewardKind::Other(kind) => (None, tr("ftbquests.port.reward_not_yet", "(%s: not in the port yet)", &[kind.clone()])),
    };
    (r.icon.clone().or(icon), title.unwrap_or(line))
}

fn wrap(game: &Game<'_>, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let next = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if !line.is_empty() && text_width(game, &next) > width {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = next;
            }
        }
        lines.push(line);
    }
    lines
}

/// A square of `size` with a border.
fn tile(game: &Game<'_>, batch: &mut HudBatch, r: [f32; 4], border: u32, fill: u32) {
    game.native_fill(batch, r, border);
    game.native_fill(batch, [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], fill);
}

fn icon(game: &Game<'_>, batch: &mut HudBatch, item: Option<&str>, at: [f32; 2], size: f32) {
    if !item.is_some_and(|i| game.native_item(batch, i, at, size)) {
        game.native_fill(batch, [at[0] + size * 0.25, at[1] + size * 0.25, size * 0.5, size * 0.5], 0xFF80_8080);
    }
}

/// A line of dots from `a` to `b`.
fn line(game: &Game<'_>, batch: &mut HudBatch, a: [f32; 2], b: [f32; 2], argb: u32, clip: [f32; 4]) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let steps = ((dx * dx + dy * dy).sqrt() / 2.0).ceil().max(1.0) as usize;
    for k in 0..=steps.min(2000) {
        let t = k as f32 / steps as f32;
        let p = [a[0] + dx * t - 1.0, a[1] + dy * t - 1.0];
        if inside(clip, p) {
            game.native_fill(batch, [p[0], p[1], 2.0, 2.0], argb);
        }
    }
}

/// `native_screen_render_post`: the book.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &ScreenView<'_>) {
    if !screen_open(game) {
        return;
    }
    let Some(file) = ftbquests_world::quest_file() else { return };
    let rect = screen.rect;
    let mouse = screen.mouse;
    BOOK.with(|b| {
        let mut b = b.borrow_mut();
        b.rect = rect;
        b.hits.clear();
        game.native_fill(batch, rect, 0xF012_1418);
        // The chapters, by group.
        let panel = [rect[0], rect[1], PANEL, rect[3]];
        game.native_fill(batch, panel, 0xFF1C_2026);
        let title = file.text("file", 1, "title").unwrap_or_else(|| tr("ftbquests.port.book", "Book of quests", &[]));
        icon(game, batch, file.icon.as_deref(), [panel[0] + 3.0, panel[1] + 3.0], 12.0);
        game.native_text(batch, &title, [panel[0] + 18.0, panel[1] + 5.0], 0xFFD7_00, true);
        let mut y = panel[1] + 20.0 - b.scroll;
        let mut group = u64::MAX;
        for (i, ch) in file.chapters.iter().enumerate() {
            if ch.group != group {
                group = ch.group;
                if group != 0 {
                    if y > panel[1] + 16.0 {
                        let name = file.text("chapter_group", group, "title").unwrap_or_default();
                        game.native_text(batch, &name, [panel[0] + 4.0, y + 3.0], 0x8F9F_AF, false);
                    }
                    y += ROW;
                }
            }
            let row = [panel[0] + 2.0, y, PANEL - 4.0, ROW];
            if y > panel[1] + 16.0 && y < panel[1] + panel[3] - ROW {
                let selected = i == b.chapter;
                if selected || inside(row, mouse) {
                    game.native_fill(batch, row, if selected { 0xFF3A_4656 } else { 0xFF2A_323C });
                }
                icon(game, batch, ch.icon.as_deref(), [row[0] + 2.0, row[1] + 1.0], 12.0);
                let name = file.text("chapter", ch.id, "title").unwrap_or_else(|| ch.filename.clone());
                let done = b.completed.contains(&ch.id);
                game.native_text(batch, &name, [row[0] + 17.0, row[1] + 3.0], if done { 0x7F_E07F } else { 0xFF_FFFF }, false);
                b.hits.push((row, Action::Chapter(i)));
            }
            y += ROW;
        }
        // The chapter's quests.
        let Some(ch) = file.chapters.get(b.chapter) else { return };
        let area = [rect[0] + PANEL, rect[1], rect[2] - PANEL, rect[3]];
        if b.centered != Some(b.chapter) {
            // Centred on the chapter's quests, zoomed to fit them.
            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
            for q in ch.quests.iter().filter_map(|q| file.quests.get(q)) {
                lo = [lo[0].min(q.x as f32), lo[1].min(q.y as f32)];
                hi = [hi[0].max(q.x as f32), hi[1].max(q.y as f32)];
            }
            if lo[0] > hi[0] {
                lo = [0.0; 2];
                hi = [0.0; 2];
            }
            b.pan = [-(lo[0] + hi[0]) / 2.0, -(lo[1] + hi[1]) / 2.0];
            let fit = ((area[2] - 40.0) / ((hi[0] - lo[0] + 2.0) * CELL)).min((area[3] - 40.0) / ((hi[1] - lo[1] + 2.0) * CELL));
            b.zoom = fit.clamp(0.4, 1.5);
            b.centered = Some(b.chapter);
        }
        let (zoom, pan) = (b.zoom, b.pan);
        let center = [area[0] + area[2] / 2.0, area[1] + area[3] / 2.0];
        let at = |x: f64, y: f64| [center[0] + (x as f32 + pan[0]) * CELL * zoom, center[1] + (y as f32 + pan[1]) * CELL * zoom];
        let shown: Vec<u64> = ch.quests.iter().copied().filter(|q| file.quests.get(q).is_some_and(|q| !q.invisible || b.completed.contains(&q.id))).collect();
        for id in &shown {
            let q = &file.quests[id];
            if q.hide_dependency_lines {
                continue;
            }
            for d in q.dependencies.iter().filter_map(|d| file.quests.get(d)).filter(|d| d.chapter == ch.id) {
                let colour = if b.completed.contains(&d.id) { 0xFF4C_AF50 } else { 0xFF60_6870 };
                line(game, batch, at(d.x, d.y), at(q.x, q.y), colour, area);
            }
        }
        let mut hover = None;
        let mut drawn = 0;
        for id in &shown {
            let q = &file.quests[id];
            let s = CELL * zoom * q.size as f32 * 0.85;
            let c = at(q.x, q.y);
            let r = [c[0] - s / 2.0, c[1] - s / 2.0, s, s];
            if r[0] + s < area[0] || r[1] + s < area[1] || r[0] > area[0] + area[2] || r[1] > area[1] + area[3] {
                continue;
            }
            let state = quest_state(&file, &b, *id);
            let (border, fill) = match state {
                State::Done => (0xFF4C_AF50, 0xFF1E_3A22),
                State::Open => (0xFFB0_B8C0, 0xFF2C_3440),
                State::Locked => (0xFF50_5458, 0xFF1A_1C20),
            };
            let hovered = inside(r, mouse) && !inside(panel, mouse);
            tile(game, batch, r, if hovered { 0xFFFF_FFFF } else { border }, fill);
            icon(game, batch, file.quest_icon(q).as_deref(), [r[0] + s * 0.15, r[1] + s * 0.15], s * 0.7);
            // A reward waiting: a gold corner.
            if state == State::Done && q.rewards.iter().any(|r| !b.claimed.contains(r)) {
                game.native_fill(batch, [r[0] + s - 5.0, r[1], 5.0, 5.0], 0xFFFF_C107);
            }
            if hovered {
                hover = Some((*id, r));
            }
            b.hits.push((r, Action::Quest(*id)));
            drawn += 1;
        }
        b.drawn = drawn;
        let name = file.text("chapter", ch.id, "title").unwrap_or_else(|| ch.filename.clone());
        game.native_text(batch, &name, [area[0] + 6.0, area[1] + 5.0], 0xFF_FFFF, true);
        // "Take all": the waiting rewards of every completed quest, in the grid's top right corner.
        let waiting = claim_all_count(&file, &b);
        if waiting > 0 {
            let label = tr("ftbquests.port.claim_all", "Take all (%s)", &[waiting.to_string()]);
            let bw = text_width(game, &label) + 8.0;
            let button = [area[0] + area[2] - bw - 4.0, area[1] + 3.0, bw, 12.0];
            tile(game, batch, button, if inside(button, mouse) { 0xFFFF_FFFF } else { 0xFFFF_C107 }, 0xFF2E_6B30);
            game.native_text(batch, &label, [button[0] + 4.0, button[1] + 2.0], 0xFF_FFFF, false);
            b.hits.push((button, Action::ClaimAll));
        }
        if let (Some((id, r)), None) = (hover, b.view) {
            let t = quest_title(&file, id);
            let w = text_width(game, &t);
            game.native_fill(batch, [r[0] + r[2] + 2.0, r[1], w + 6.0, 12.0], 0xF010_0010);
            game.native_text(batch, &t, [r[0] + r[2] + 5.0, r[1] + 2.0], 0xFF_FFFF, true);
        }
        if let Some(id) = b.view {
            view(game, batch, &file, &mut b, id, rect, mouse);
        }
        if let Some(id) = b.picking {
            picker(game, batch, &file, &mut b, id, rect, mouse);
        }
    });
}

/// A choice reward's picker, over the quest's view: the table's rewards in columns, a click takes one.
fn picker(game: &Game<'_>, batch: &mut HudBatch, file: &QuestFile, b: &mut Book, id: u64, rect: [f32; 4], mouse: [f32; 2]) {
    let Some(RewardKind::Choice { table }) = file.rewards.get(&id).map(|r| &r.kind) else { return };
    let Some(t) = file.reward_tables.get(table) else { return };
    const COL: f32 = 150.0;
    let cols = (((rect[2] - 60.0) / COL).floor() as usize).clamp(1, 4).min(t.entries.len().max(1));
    let rows_fit = (((rect[3] - 70.0) / ROW).floor() as usize).max(1);
    let rows = t.entries.len().div_ceil(cols).min(rows_fit);
    let w = cols as f32 * COL + 12.0;
    let h = rows as f32 * ROW + 30.0;
    let v = [rect[0] + (rect[2] - w) / 2.0, rect[1] + (rect[3] - h) / 2.0, w, h];
    b.hits.retain(|_| false);
    b.hits.push((rect, Action::ClosePicker));
    tile(game, batch, v, 0xFFFF_C107, 0xFA18_1C24);
    let title = tr("ftbquests.port.pick_one", "Pick one: %s", &[file.table_title(*table)]);
    game.native_text(batch, &fit(game, &title, w - 12.0), [v[0] + 6.0, v[1] + 6.0], 0xFF_D54F, true);
    for (i, e) in t.entries.iter().enumerate().take(rows * cols) {
        let (col, row) = (i / rows, i % rows);
        let cell = [v[0] + 6.0 + col as f32 * COL, v[1] + 22.0 + row as f32 * ROW, COL - 4.0, ROW - 1.0];
        let hovered = inside(cell, mouse);
        if hovered {
            game.native_fill(batch, cell, 0xFF3A_4656);
        }
        let entry = Reward { id: e.id, quest: 0, kind: e.kind.clone(), team: None, exclude_from_claim_all: false, auto: AutoClaim::Default, ignore_blocking: false, icon: None };
        let (icon_id, line) = reward_line(file, &entry);
        icon(game, batch, icon_id.as_deref(), [cell[0] + 1.0, cell[1] + 1.0], 12.0);
        game.native_text(batch, &fit(game, &line, COL - 22.0), [cell[0] + 16.0, cell[1] + 3.0], if hovered { 0xFF_FFFF } else { 0xDD_DDDD }, false);
        b.hits.push((cell, Action::Pick(id, i)));
    }
}

/// The quest's view, over the grid: title, text, tasks, rewards.
fn view(game: &Game<'_>, batch: &mut HudBatch, file: &QuestFile, b: &mut Book, id: u64, rect: [f32; 4], mouse: [f32; 2]) {
    let Some(q) = file.quests.get(&id) else { return };
    let w = (rect[2] - 40.0).min(300.0);
    let h = (rect[3] - 40.0).min(240.0);
    let v = [rect[0] + (rect[2] - w) / 2.0, rect[1] + (rect[3] - h) / 2.0, w, h];
    // A click outside closes it.
    b.hits.retain(|_| false);
    b.hits.push((rect, Action::CloseView));
    tile(game, batch, v, 0xFF8A_96A4, 0xF820_2630);
    let state = quest_state(file, b, id);
    let mut y = v[1] + 6.0;
    icon(game, batch, file.quest_icon(q).as_deref(), [v[0] + 6.0, y], 16.0);
    game.native_text(batch, &quest_title(file, id), [v[0] + 26.0, y + 4.0], 0xFF_FFFF, true);
    y += 20.0;
    if let Some(sub) = file.text("quest", id, "quest_subtitle") {
        game.native_text(batch, &sub, [v[0] + 8.0, y], 0xAF_AFAF, false);
        y += 11.0;
    }
    let half = v[2] / 2.0;
    // Text on the left, tasks and rewards on the right.
    if let Some(desc) = file.text("quest", id, "quest_desc") {
        for l in wrap(game, &desc, half - 14.0).into_iter().take(((v[1] + v[3] - y - 6.0) / 10.0).max(0.0) as usize) {
            game.native_text(batch, &l, [v[0] + 8.0, y], 0xDD_DDDD, false);
            y += 10.0;
        }
    }
    let x = v[0] + half;
    let mut ty = v[1] + 30.0;
    game.native_text(batch, &tr("ftbquests.port.tasks", "To do", &[]), [x, ty], 0x9F_C7FF, true);
    ty += 12.0;
    for t in q.tasks.iter().filter_map(|t| file.tasks.get(t)) {
        let done = b.completed.contains(&t.id);
        let (item, label) = match &t.kind {
            TaskKind::Item { item, count, .. } => {
                let have = b.tasks.get(&t.id).copied().unwrap_or(0);
                (Some(item.clone()), format!("{} {}/{}", item_name(item), if done { *count as i64 } else { have }, count))
            }
            TaskKind::Checkmark => (None, file.text("task", t.id, "title").unwrap_or_else(|| tr("ftbquests.port.checkmark", "Tick it when done", &[]))),
            TaskKind::Other(kind) => (None, tr("ftbquests.port.task_not_yet", "(%s: not in the port yet)", &[kind.clone()])),
        };
        let label = fit(game, &label, half - 23.0);
        let row = [x, ty, half - 8.0, 14.0];
        if let TaskKind::Checkmark = t.kind {
            let mark = [x, ty, 12.0, 12.0];
            tile(game, batch, mark, if inside(mark, mouse) { 0xFFFF_FFFF } else { 0xFFA0_A0A0 }, if done { 0xFF4C_AF50 } else { 0xFF20_2020 });
            if !done && state != State::Locked {
                b.hits.push((mark, Action::Check(t.id)));
            }
        } else {
            icon(game, batch, item.as_deref(), [x, ty], 12.0);
        }
        game.native_text(batch, &label, [x + 15.0, ty + 2.0], if done { 0x7F_E07F } else { 0xFF_FFFF }, false);
        ty += row[3];
    }
    ty += 6.0;
    game.native_text(batch, &tr("ftbquests.port.rewards", "You get", &[]), [x, ty], 0xFF_D54F, true);
    ty += 12.0;
    if b.blocked {
        game.native_text(batch, &fit(game, &tr("ftbquests.port.rewards_blocked", "Held back for your team", &[]), half - 8.0), [x, ty], 0xFF_6F6F, false);
        ty += 11.0;
    }
    let bottom = v[1] + v[3] - 14.0;
    // An auto-claimed `invisible` reward isn't listed.
    for r in q.rewards.iter().filter_map(|r| file.rewards.get(r)).filter(|r| file.auto_claim(r) != AutoClaim::Invisible) {
        if ty > bottom {
            game.native_text(batch, "...", [x + 15.0, ty - 4.0], 0x9F_9F9F, false);
            break;
        }
        let claimed = b.claimed.contains(&r.id);
        let (item, line) = reward_line(file, r);
        icon(game, batch, item.as_deref(), [x, ty], 12.0);
        if let RewardKind::Other(_) = r.kind {
            game.native_text(batch, &fit(game, &line, half - 23.0), [x + 15.0, ty + 2.0], 0x9F_9F9F, false);
            ty += 14.0;
            continue;
        }
        let choice = matches!(r.kind, RewardKind::Choice { .. });
        let label = if claimed {
            tr("ftbquests.port.claimed", "taken", &[])
        } else if choice {
            tr("ftbquests.port.choose", "Pick", &[])
        } else {
            tr("ftbquests.port.claim", "Take", &[])
        };
        let bw = text_width(game, &label) + 8.0;
        let right = v[0] + v[2] - bw - 6.0;
        // The button sits at the view's edge; a line that would run under it puts the button on its own row.
        let (line, button) = if x + 15.0 + text_width(game, &line) + 6.0 <= right {
            (line, [right, ty, bw, 12.0])
        } else {
            (fit(game, &line, v[0] + v[2] - 6.0 - x - 15.0), [x + 15.0, ty + 14.0, bw, 12.0])
        };
        game.native_text(batch, &line, [x + 15.0, ty + 2.0], 0xFF_FFFF, false);
        if button[1] > ty {
            ty += 14.0;
        }
        let can = state == State::Done && !claimed && (!b.blocked || r.ignore_blocking);
        tile(game, batch, button, if can && inside(button, mouse) { 0xFFFF_FFFF } else { 0xFF70_7070 }, if can { 0xFF2E_6B30 } else { 0xFF30_3030 });
        game.native_text(batch, &label, [button[0] + 4.0, button[1] + 2.0], if can { 0xFF_FFFF } else { 0x9F_9F9F }, false);
        if can {
            b.hits.push((button, if choice { Action::Choose(r.id) } else { Action::Claim(r.id) }));
        }
        ty += 14.0;
    }
}

/// `native_screen_mouse_button_pressed_pre`: chapters, quests, the view's buttons; a drag starts on the grid.
pub(super) fn pressed(game: &Game<'_>, screen: &ScreenView<'_>, button: i32, canceled: &mut bool) {
    if !screen_open(game) || button != 0 {
        return;
    }
    *canceled = true;
    BOOK.with(|b| {
        let mut b = b.borrow_mut();
        b.last_mouse = Some(screen.mouse);
        // The last region drawn is on top.
        let Some(action) = b.hits.iter().rev().find(|(r, _)| inside(*r, screen.mouse)).map(|(_, a)| *a) else { return };
        match action {
            Action::Chapter(i) => {
                b.chapter = i;
                b.view = None;
            }
            Action::Quest(id) => b.view = Some(id),
            Action::CloseView => b.view = None,
            Action::Choose(id) => b.picking = Some(id),
            Action::ClosePicker => b.picking = None,
            Action::Pick(..) => {
                b.picking = None;
                b.pending.push(action);
            }
            Action::Check(_) | Action::Claim(_) | Action::ClaimAll => b.pending.push(action),
        }
        b.last_mouse = None;
    });
}

/// `native_screen_mouse_dragged_pre`: the grid follows the pointer.
pub(super) fn dragged(game: &Game<'_>, screen: &ScreenView<'_>, canceled: &mut bool) {
    if !screen_open(game) {
        return;
    }
    *canceled = true;
    BOOK.with(|b| {
        let mut b = b.borrow_mut();
        if let Some(last) = b.last_mouse.replace(screen.mouse) {
            let k = CELL * b.zoom;
            b.pan[0] += (screen.mouse[0] - last[0]) / k;
            b.pan[1] += (screen.mouse[1] - last[1]) / k;
        }
    });
}

/// `native_screen_mouse_scrolled_pre`: the wheel scrolls the chapters, or zooms the grid.
pub(super) fn scrolled(game: &Game<'_>, screen: &ScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if !screen_open(game) || scroll[1] == 0.0 {
        return;
    }
    *canceled = true;
    BOOK.with(|b| {
        let mut b = b.borrow_mut();
        if screen.mouse[0] < b.rect[0] + PANEL {
            b.scroll = (b.scroll - scroll[1] as f32 * ROW * 3.0).max(0.0);
        } else {
            b.zoom = (b.zoom * if scroll[1] > 0.0 { 1.25 } else { 0.8 }).clamp(0.25, 3.0);
        }
    });
}

pub(super) fn client_commands(commands: &mut Vec<Command>) {
    if dev() {
        commands.push(Command::new("ftbquestsbook [line:text]"));
    }
}

/// `native_command`: `/ftbquestsbook [chapter <i>]` puts `FTBQB {json}` in chat: the screen and its clickable
/// regions, and the progress this client knows (tests).
pub(super) fn client_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let words: Vec<&str> = command.split_whitespace().collect();
    if *ran || !dev() || words.first() != Some(&"ftbquestsbook") {
        return;
    }
    *ran = true;
    if let ["ftbquestsbook", "chapter", name] = words.as_slice() {
        if let Some(file) = ftbquests_world::quest_file() {
            if let Some(i) = file.chapters.iter().position(|c| c.filename == *name) {
                BOOK.with(|b| {
                    let mut b = b.borrow_mut();
                    b.chapter = i;
                    b.view = None;
                });
            }
        }
    }
    let open = screen_open(game);
    let json = BOOK.with(|b| {
        let b = b.borrow();
        let file = ftbquests_world::quest_file();
        let hits: Vec<serde_json::Value> = b
            .hits
            .iter()
            .filter(|(_, a)| !matches!(a, Action::Chapter(_)))
            .map(|(r, a)| {
                let (kind, id) = match a {
                    Action::Quest(id) => ("quest", code(*id)),
                    Action::Check(id) => ("check", code(*id)),
                    Action::Claim(id) => ("claim", code(*id)),
                    Action::CloseView => ("close", String::new()),
                    Action::Chapter(i) => ("chapter", i.to_string()),
                    Action::Choose(id) => ("choose", code(*id)),
                    Action::Pick(id, i) => ("pick", format!("{}:{i}", code(*id))),
                    Action::ClosePicker => ("close_picker", String::new()),
                    Action::ClaimAll => ("claim_all", String::new()),
                };
                serde_json::json!({ "kind": kind, "id": id, "rect": r })
            })
            .collect();
        serde_json::json!({
            "open": open, "team": b.team, "syncs": b.syncs, "tasks": b.tasks.len(), "completed": b.completed.len(),
            "claimed": b.claimed.iter().map(|c| code(*c)).collect::<Vec<_>>(),
            "chapter": file.as_ref().and_then(|f| f.chapters.get(b.chapter)).map(|c| c.filename.clone()),
            "view": b.view.map(code), "drawn": b.drawn, "hits": hits, "sent": b.sent, "rect": b.rect,
            "blocked": b.blocked, "picking": b.picking.map(code), "toasts": b.toasts,
        })
    });
    game.native_chat(&format!("FTBQB {json}"));
}
