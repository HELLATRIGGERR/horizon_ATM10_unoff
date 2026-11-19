// FTB Quests' book screen (support module `crate::game::ftbquests_book` of the `ftbquests` native bundle), the
// mod screen `ftbquests`: the chapters by group down the left, the chosen chapter's quests on a grid that a drag
// pans and the wheel zooms (dependency lines, item icons, the state of each quest), and a quest's view over it
// (its text, its tasks with their progress and the checkmark button, its rewards with the claim button).
// It opens from the server (the book item, `/ftbquests open_book`) or with the quests key (unbound, as in FTB
// Quests). The team's progress comes from the server (`ftbquests:sync_team_data`); `/ftbquestsbook` (FTBTEAMS_DEV=1)
// puts `FTBQB {json}` in chat for the tests. FTB Quests is All Rights Reserved: written from its behaviour, drawn
// with fills and the game's item icons; labels are translation keys with our own fallback wording.

use super::{Game, ftbquests_world};
use ftbquests_core::{
    CLAIM_REWARD, OPEN_BOOK, Request, SUBMIT_TASK, SYNC, Sync,
    file::{QuestFile, RewardKind, TaskKind, code, parse_id},
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
}

#[derive(Default)]
struct Book {
    /// The team's progress as the server sent it.
    team: String,
    tasks: HashMap<u64, i64>,
    completed: HashSet<u64>,
    claimed: HashSet<u64>,
    syncs: u32,
    // The screen.
    chapter: usize,
    centered: Option<usize>,
    pan: [f32; 2],
    zoom: f32,
    view: Option<u64>,
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
    BOOK.with(|b| b.borrow_mut().view = None);
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
                b.tasks.extend(part.tasks.iter().filter_map(|(k, v)| Some((parse_id(k)?, *v))));
                b.completed.extend(part.completed.iter().filter_map(|k| parse_id(k)));
                b.claimed.extend(part.claimed.iter().filter_map(|k| parse_id(k)));
                b.syncs += 1;
            }),
            Err(e) => log::warn!("[ftbquests] a bad {SYNC} message: {e}"),
        },
        OPEN_BOOK => BOOK.with(|b| b.borrow_mut().open_requested = true),
        _ => {}
    }
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
    let (open_now, actions) = BOOK.with(|b| {
        let mut b = b.borrow_mut();
        (std::mem::take(&mut b.open_requested), std::mem::take(&mut b.pending))
    });
    if open_now && !screen_open(game) {
        open(game);
    }
    for action in actions {
        let (channel, id) = match action {
            Action::Check(id) => (SUBMIT_TASK, id),
            Action::Claim(id) => (CLAIM_REWARD, id),
            _ => continue,
        };
        let data = serde_json::to_vec(&Request { id: code(id) }).unwrap_or_default();
        match game.native_net_send_to_server(channel, &data) {
            Ok(()) => BOOK.with(|b| b.borrow_mut().sent.push(format!("{channel} {}", code(id)))),
            Err(e) => log::debug!("[ftbquests] {channel}: {e}"),
        }
    }
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
        if let (Some((id, r)), None) = (hover, b.view) {
            let t = quest_title(&file, id);
            let w = text_width(game, &t);
            game.native_fill(batch, [r[0] + r[2] + 2.0, r[1], w + 6.0, 12.0], 0xF010_0010);
            game.native_text(batch, &t, [r[0] + r[2] + 5.0, r[1] + 2.0], 0xFF_FFFF, true);
        }
        if let Some(id) = b.view {
            view(game, batch, &file, &mut b, id, rect, mouse);
        }
    });
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
    for r in q.rewards.iter().filter_map(|r| file.rewards.get(r)) {
        let claimed = b.claimed.contains(&r.id);
        match &r.kind {
            RewardKind::Item { item, count, .. } => {
                icon(game, batch, Some(item), [x, ty], 12.0);
                let line = format!("{count} x {}", item_name(item));
                game.native_text(batch, &line, [x + 15.0, ty + 2.0], 0xFF_FFFF, false);
                let label = if claimed { tr("ftbquests.port.claimed", "taken", &[]) } else { tr("ftbquests.port.claim", "Take", &[]) };
                let bw = text_width(game, &label) + 8.0;
                let right = v[0] + v[2] - bw - 6.0;
                // The button sits at the view's edge; a name that would run under it puts the button on its own row.
                let button = if x + 15.0 + text_width(game, &line) + 6.0 <= right {
                    [right, ty, bw, 12.0]
                } else {
                    ty += 14.0;
                    [x + 15.0, ty, bw, 12.0]
                };
                let can = state == State::Done && !claimed;
                tile(game, batch, button, if can && inside(button, mouse) { 0xFFFF_FFFF } else { 0xFF70_7070 }, if can { 0xFF2E_6B30 } else { 0xFF30_3030 });
                game.native_text(batch, &label, [button[0] + 4.0, ty + 2.0], if can { 0xFF_FFFF } else { 0x9F_9F9F }, false);
                if can {
                    b.hits.push((button, Action::Claim(r.id)));
                }
            }
            RewardKind::Other(kind) => {
                game.native_text(batch, &tr("ftbquests.port.reward_not_yet", "(%s: not in the port yet)", &[kind.clone()]), [x + 15.0, ty + 2.0], 0x9F_9F9F, false);
            }
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
            Action::Check(_) | Action::Claim(_) => b.pending.push(action),
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
                };
                serde_json::json!({ "kind": kind, "id": id, "rect": r })
            })
            .collect();
        serde_json::json!({
            "open": open, "team": b.team, "syncs": b.syncs, "tasks": b.tasks.len(), "completed": b.completed.len(),
            "claimed": b.claimed.iter().map(|c| code(*c)).collect::<Vec<_>>(),
            "chapter": file.as_ref().and_then(|f| f.chapters.get(b.chapter)).map(|c| c.filename.clone()),
            "view": b.view.map(code), "drawn": b.drawn, "hits": hits, "sent": b.sent, "rect": b.rect,
        })
    });
    game.native_chat(&format!("FTBQB {json}"));
}
