// The quest book's screen (support module `crate::game::ftbq_ui` of the
// `ftbquests` native bundle), laid out as FTB Quests' `QuestScreen`: the chapter
// list on the left, the chapter's quest graph with its lines, scroll and zoom,
// and `ViewQuestPanel` over it with a quest's tasks, rewards and description.
// Numbers and colours come from the mod's source and `ftb_quests_theme.txt`; its
// textures come from the player's jars through `ftbquests-content`.
//
// The screen hooks take `&Game`, so what must change the game (sending to the
// server, closing) waits in `actions` for `native_frame`. Each frame's draw leaves
// the buttons it drew in `hits`; a click tests them, topmost first.

use super::{ftbq_world, native_hooks::NativeScreenView, Game};
use ftbquests_core::{
    book::{Book, Chapter, Id, ItemRef, Progression, Quest, Reward, RewardKind, Task, TaskKind, id_string},
    graph::{Bounds, Node, View, line_quad},
    names::{self, Names},
    progress::{Progress, VisibleMemo},
    text::{self, Line, Run, Style},
    wire::{TO_SERVER, ToClient, ToServer},
};
use horizon_client::{
    hud::HudBatch,
    items::ItemStack,
    text::{Style as TextStyle, Text},
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    sync::{Arc, OnceLock},
    time::Instant,
};

pub(super) const MOD_ID: &str = ftbq_world::MOD_ID;

type Rect = [f32; 4];

// `ftb_quests_theme.txt`.
const TEXT: u32 = 0xFFFFFF;
const HOVER_TEXT: u32 = 0xFFFFA0;
const DISABLED_TEXT: u32 = 0x999999;
const BORDER: u32 = 0xFF1B1D1E;
const WIDGET_BACKGROUND: u32 = 0x44000000;
const TASKS_TEXT: u32 = 0x5555FF;
const REWARDS_TEXT: u32 = 0xFFAA00;
const TITLE_TEXT: u32 = 0xAAAAAA;
const COMPLETED: u32 = 0xC856FF56;
const STARTED: u32 = 0xC800FFFF;
const NOT_STARTED: u32 = 0x96FFFFFF;
const LOCKED: u32 = 0xFF999999;
const LINE_COMPLETED: u32 = 0xFF64DC64;
const LINE_UNCOMPLETED: u32 = 0xB4CCA3A3;
const LINE_UNAVAILABLE: u32 = 0x64CCA3A3;
const LINE_REQUIRES: u32 = 0xFF00C8C8;
const LINE_REQUIRED_FOR: u32 = 0xFFC8C800;
const LINE_THICKNESS: f64 = 0.17;

const SHAPE_FILL: u32 = 0xFF212121;
const SHAPE_BACK: u32 = 0x96FFFFFF;

const MISSING_ITEM: &str = "ftbquests:item/missing_item";
const BACKGROUND: &str = "ftbquests_gui:gui/background_squares";
const DEPENDENCY: &str = "ftbquests_gui:gui/dependency";
const ALERT: &str = "ftbquests_gui:gui/alert";
const CLOSE: &str = "ftbquests_gui:gui/close";
const LOCK: &str = "ftbquests_gui:gui/quest_locked";
const PIN: &str = "ftbquests_gui:gui/pin";
const COLLECT: &str = "ftbquests_gui:gui/collect_rewards";
const ARROW_LEFT: &str = "ftbquests_gui:gui/arrow_left";
const ARROW_RIGHT: &str = "ftbquests_gui:gui/arrow_right";
const ARROW_COLLAPSED: &str = "ftbquests_gui:gui/arrow_collapsed";
const ARROW_EXPANDED: &str = "ftbquests_gui:gui/arrow_expanded";
const CHECK: &str = "ftbquests_gui:gui/icons/accept";
const CHECK_GRAY: &str = "ftbquests_gui:gui/icons/accept_gray";

const ROW: f32 = 14.0;
const HEADER: f32 = 18.0;
const SIDE: f32 = 20.0;

/// What a click can land on.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    /// Nothing that acts: it only keeps clicks from what's under it.
    Blocker,
    Chapter(usize),
    Group(Id),
    Pin,
    Collect,
    Quest(Id),
    ClosePanel,
    PrevPage,
    NextPage,
    Task(Id),
    Reward(Id),
}

/// What waits for `native_frame`.
enum Action {
    Send(ToServer),
    Close,
    Click,
}

/// An icon: an item, or a sprite of the GUI atlas.
#[derive(Clone, Debug, PartialEq)]
enum Icon {
    Item(String),
    Sprite(&'static str),
    Missing,
}

#[derive(Default)]
struct PanelState {
    quest: Id,
    scroll: f32,
    page: usize,
}

/// What a whole book's questions come to, until the progress next changes.
#[derive(Default)]
struct Worked {
    fresh: bool,
    memo: VisibleMemo,
    /// By chapter: it shows; a reward waits in it.
    chapter_shown: Vec<bool>,
    chapter_waiting: Vec<bool>,
    any_waiting: bool,
}

#[derive(Default)]
struct Ui {
    book: Option<Arc<Book>>,
    locale: String,
    progress: Progress,
    worked: Worked,
    chapter: usize,
    chapter_chosen: bool,
    /// Each chapter's zoom and scroll, for the session.
    views: HashMap<Id, (i32, f64, f64)>,
    view: Option<View>,
    view_of: Option<Id>,
    panel: Option<PanelState>,
    collapsed: HashSet<Id>,
    unpinned: bool,
    expanded: bool,
    list_scroll: f32,
    /// A left-button drag of the graph is on.
    dragging: bool,
    actions: Vec<Action>,
    hits: Vec<(Rect, Hit)>,
    /// Where the quest panel was last drawn: what's beside it doesn't react.
    panel_rect: Option<Rect>,
    size: [f32; 2],
    mouse: [f32; 2],
}

thread_local! {
    static UI: RefCell<Ui> = RefCell::new(Ui::default());
    /// A filter task's valid items, for its icon.
    static FILTER_ITEMS: RefCell<HashMap<Id, Arc<Vec<String>>>> = RefCell::new(HashMap::new());
}

fn elapsed() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

fn inside(r: Rect, at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

/// The game's text for the mod's names.
struct GameNames<'a, 'g>(&'a Game<'g>);

impl Names for GameNames<'_, '_> {
    fn tr(&self, key: &str, fallback: &str) -> String {
        self.0.native_translate(key, fallback)
    }
    fn item(&self, id: &str) -> String {
        if self.0.native_item_known(id) {
            self.0.native_item_name(id)
        } else {
            names::pretty(id)
        }
    }
}

// ---- Icons -----------------------------------------------------------------

fn item_icon(item: &ItemRef) -> Icon {
    Icon::Item(item.id.clone())
}

/// `TaskTypes`' icons, the vanilla items they use.
fn kind_icon(kind: &str) -> Icon {
    Icon::Item(
        match kind {
            "kill" => "minecraft:diamond_sword",
            "dimension" => "minecraft:ender_eye",
            "biome" => "minecraft:oak_sapling",
            "structure" | "location" => "minecraft:compass",
            "advancement" => "minecraft:wheat",
            "observation" => "minecraft:spyglass",
            "forge_energy" => "minecraft:redstone",
            "xp" | "xp_levels" => "minecraft:experience_bottle",
            "random" | "loot" | "choice" | "all_table" => "minecraft:chest",
            "command" => "minecraft:command_block",
            "toast" => "minecraft:paper",
            _ => "minecraft:book",
        }
        .to_owned(),
    )
}

fn task_icon(game: &Game<'_>, task: &Task, done: bool) -> Icon {
    if let Some(icon) = &task.icon {
        return item_icon(icon);
    }
    match &task.kind {
        TaskKind::Item { item, filter, .. } => match filter {
            Some(filter) => {
                // `IconAnimation`: the valid items in turn, a second each.
                let items = FILTER_ITEMS.with(|cache| {
                    cache
                        .borrow_mut()
                        .entry(task.id)
                        .or_insert_with(|| {
                            let mut named = Vec::new();
                            filter.named_items(&mut named);
                            let mut tags = Vec::new();
                            filter.named_tags(&mut tags);
                            tags.iter().for_each(|t| named.extend(game.native_items_in_tag(t)));
                            named.retain(|id| game.native_item_known(id));
                            Arc::new(named)
                        })
                        .clone()
                });
                if items.is_empty() {
                    Icon::Missing
                } else {
                    Icon::Item(items[elapsed() as usize % items.len()].clone())
                }
            }
            None => item_icon(item),
        },
        TaskKind::Checkmark => Icon::Sprite(if done { CHECK } else { CHECK_GRAY }),
        TaskKind::Other { kind, .. } => kind_icon(kind),
    }
}

fn reward_icon(reward: &Reward) -> Icon {
    if let Some(icon) = &reward.icon {
        return item_icon(icon);
    }
    match &reward.kind {
        RewardKind::Item { item, .. } => item_icon(item),
        RewardKind::Other { kind, .. } => kind_icon(kind),
    }
}

/// `Quest.getIcon`: its own, or its tasks' in turn.
fn quest_icon(game: &Game<'_>, quest: &Quest, progress: &Progress) -> Icon {
    if let Some(icon) = &quest.icon {
        return item_icon(icon);
    }
    match quest.tasks.len() {
        0 => Icon::Item("minecraft:book".to_owned()),
        n => {
            let task = &quest.tasks[elapsed() as usize % n];
            task_icon(game, task, progress.is_completed(task.id))
        }
    }
}

fn draw_icon(game: &Game<'_>, batch: &mut HudBatch, icon: &Icon, x: f32, y: f32, size: f32) {
    let drawn = match icon {
        Icon::Item(id) => game.native_item(batch, id, [x, y], size),
        Icon::Sprite(name) => sprite(game, batch, name, [x, y, size, size], u32::MAX),
        Icon::Missing => false,
    };
    if !drawn {
        draw_placeholder(game, batch, x, y, size);
    }
}

/// An item of a mod that isn't carried yet, as FTB Quests draws its missing item.
fn draw_placeholder(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, size: f32) {
    if !sprite(game, batch, MISSING_ITEM, [x, y, size, size], u32::MAX) {
        game.native_fill(batch, [x, y, size, size], 0xFF7F007F);
        game.native_text(batch, "?", [x + size / 2.0 - 2.0, y + size / 2.0 - 4.0], 0xFFFFFF, true);
    }
}

fn sprite(game: &Game<'_>, batch: &mut HudBatch, name: &str, dst: Rect, argb: u32) -> bool {
    match game.native_sprite_size(name) {
        Some([w, h]) => game.native_sprite_tinted(batch, name, [0.0, 0.0, w, h], dst, argb),
        None => false,
    }
}

// ---- Text ------------------------------------------------------------------

fn styled(run: &Run) -> Text {
    let style = TextStyle {
        color: run.style.color,
        bold: run.style.bold.then_some(true),
        italic: run.style.italic.then_some(true),
        underlined: run.style.underlined.then_some(true),
        strikethrough: run.style.strikethrough.then_some(true),
        obfuscated: run.style.obfuscated.then_some(true),
        ..TextStyle::default()
    };
    Text::styled(run.text.clone(), style)
}

/// A translator for `{key}` in the book's texts.
fn translator<'a>(game: &'a Game<'_>) -> impl Fn(&str) -> Option<String> + 'a {
    move |key: &str| {
        let found = game.native_translate(key, "\u{1}");
        (found != "\u{1}").then_some(found)
    }
}

fn run_width(game: &Game<'_>, text: &str, style: Style) -> f32 {
    let bold = if style.bold { text.chars().count() as f32 } else { 0.0 };
    game.native_text_width(text) + bold
}

fn line_width(game: &Game<'_>, line: &[Run]) -> f32 {
    line.iter().map(|r| run_width(game, &r.text, r.style)).sum()
}

fn draw_line(game: &Game<'_>, batch: &mut HudBatch, line: &[Run], x: f32, y: f32, rgb: u32, shadow: bool) {
    let mut at = x;
    for run in line {
        game.native_text_styled(batch, &styled(run), [at, y], rgb, shadow);
        at += run_width(game, &run.text, run.style);
    }
}

fn plain_runs(text: &str) -> Vec<Run> {
    vec![Run { text: text.to_owned(), style: Style::default() }]
}

/// A title in plain text with codes and keys read.
fn title_of(game: &Game<'_>, raw: &str, otherwise: impl FnOnce() -> String) -> String {
    if raw.is_empty() {
        otherwise()
    } else {
        text::plain(raw, &translator(game))
    }
}

fn quest_title(game: &Game<'_>, quest: &Quest) -> String {
    title_of(game, &quest.title, || {
        // `Quest.getAltTitle`: its first task's title.
        quest.tasks.first().map_or_else(String::new, |t| task_title(game, t))
    })
}

fn task_title(game: &Game<'_>, task: &Task) -> String {
    title_of(game, &task.title, || names::task_title(&GameNames(game), task))
}

fn reward_title(game: &Game<'_>, book: &Book, reward: &Reward) -> String {
    title_of(game, &reward.title, || {
        let table = |id: i64| {
            book.tables.iter().find(|t| t.id == id as u64).map(|t| text::plain(&t.title, &translator(game)))
        };
        names::reward_title(&GameNames(game), reward, &table)
    })
}

// ---- Opening ----------------------------------------------------------------

/// `ClientQuestFile.openGui`.
pub(super) fn open(game: &mut Game<'_>) {
    if !game.native_mod_screen_open(MOD_ID) {
        return;
    }
    let locale = horizon_client::lang::code();
    UI.with(|ui| {
        let mut ui = ui.borrow_mut();
        if ui.book.is_none() || ui.locale != locale {
            ui.book = ftbq_world::book(&locale);
            ui.locale = locale;
            ui.view = None;
            ui.view_of = None;
            ui.worked.fresh = false;
        }
        ui.dragging = false;
    });
    let _ = game.native_net_send_to_server(TO_SERVER, &ToServer::Hello.encode());
}

/// `native_key`: the mod's key, which it leaves unbound until the player binds it.
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if pressed && game.native_playing() && key == game.native_mod_key(MOD_ID, "quests", "key.keyboard.unknown") {
        open(game);
    }
}

/// `native_net_from_server`.
pub(super) fn from_server(game: &mut Game<'_>, channel: &str, data: &[u8]) {
    if channel != ftbquests_core::wire::TO_CLIENT {
        return;
    }
    match ToClient::decode(data) {
        Some(ToClient::Open) => open(game),
        Some(ToClient::Reset) => UI.with(|ui| {
            let mut ui = ui.borrow_mut();
            ui.progress = Progress::default();
            ui.worked.fresh = false;
        }),
        Some(ToClient::Changes(all)) => UI.with(|ui| {
            let mut ui = ui.borrow_mut();
            all.into_iter().for_each(|c| ui.progress.apply(c));
            ui.worked.fresh = false;
        }),
        None => {}
    }
}

/// `native_frame`: what the `&Game` hooks left to do.
pub(super) fn frame(game: &mut Game<'_>) {
    let actions = UI.with(|ui| std::mem::take(&mut ui.borrow_mut().actions));
    for action in actions {
        match action {
            Action::Send(message) => {
                let _ = game.native_net_send_to_server(TO_SERVER, &message.encode());
            }
            Action::Close => game.native_mod_screen_close(MOD_ID),
            Action::Click => game.native_click_sound(),
        }
    }
    dump_view(game);
}

// ---- Drawing -----------------------------------------------------------------

/// The panel's position in the graph for a quest or link.
fn nodes_of(chapter: &Chapter) -> impl Iterator<Item = Node> + '_ {
    let quests = chapter.quests.iter().map(|q| Node { x: q.x, y: q.y, w: q.size, h: q.size });
    let links = chapter.links.iter().map(|l| Node { x: l.x, y: l.y, w: l.size, h: l.size });
    quests.chain(links)
}

impl Ui {
    /// Works out what the book shows, once per change of progress.
    fn work_out(&mut self, book: &Book) {
        if self.worked.fresh {
            return;
        }
        let worked = &mut self.worked;
        worked.fresh = true;
        worked.memo.clear();
        worked.chapter_shown = book.chapters.iter().map(|c| self.progress.chapter_visible_in(book, c, &mut worked.memo)).collect();
        worked.chapter_waiting = book.chapters.iter().map(|c| c.quests.iter().any(|q| self.progress.has_unclaimed(q))).collect();
        worked.any_waiting = worked.chapter_waiting.iter().any(|w| *w);
    }
    /// The view of the chapter on show, made or remembered.
    fn ensure_view(&mut self, book: &Book, panel: Rect) {
        let Some(chapter) = book.chapters.get(self.chapter) else { return };
        if self.view_of == Some(chapter.id) {
            if let Some(view) = &mut self.view {
                if view.panel != panel {
                    let (x, y) = view.centre_position();
                    view.panel = panel;
                    view.scroll_to(x, y);
                }
            }
            return;
        }
        if let (Some(old), Some(view)) = (self.view_of, &self.view) {
            self.views.insert(old, (view.zoom, view.scroll_x, view.scroll_y));
        }
        let mut view = View::new(panel, Bounds::of(nodes_of(chapter)));
        match self.views.get(&chapter.id) {
            Some(&(zoom, x, y)) => {
                view.zoom = zoom;
                view.scroll_x = x;
                view.scroll_y = y;
            }
            None => {
                if let Some(quest) = chapter.autofocus.and_then(|id| book.quest(id)) {
                    view.scroll_to(quest.x, quest.y);
                }
            }
        }
        self.view = Some(view);
        self.view_of = Some(chapter.id);
    }
}

/// `native_screen_render_post`.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    UI.with(|ui| draw_all(game, batch, screen, &mut ui.borrow_mut()));
}

fn draw_all(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>, ui: &mut Ui) {
    let [_, _, w, h] = screen.rect;
    ui.size = [w, h];
    ui.mouse = screen.mouse;
    ui.hits.clear();
    draw_background(game, batch, [0.0, 0.0, w, h], 0xDCFFFFFF);
    let Some(book) = ui.book.clone() else {
        let message = game.native_translate("ftbquests.gui.no_quest_file", "No FTB Quests files in config/ftbquests/quests");
        game.native_text(batch, &message, [(w - game.native_text_width(&message)) / 2.0, h / 2.0 - 4.0], DISABLED_TEXT, true);
        return;
    };
    let panel: Rect = [SIDE, 1.0, w - 2.0 * SIDE, h - 2.0];
    ui.work_out(&book);
    if !ui.chapter_chosen {
        ui.chapter_chosen = true;
        ui.chapter = ui.worked.chapter_shown.iter().position(|s| *s).unwrap_or(0);
    }
    ui.ensure_view(&book, panel);

    let graph_top = ui.panel.is_none();
    draw_sides(game, batch, w, h);
    draw_graph(game, batch, &book, ui, graph_top);
    draw_chapters(game, batch, &book, ui, h);
    draw_right(game, batch, ui, w);
    if ui.panel.is_some() {
        draw_quest_panel(game, batch, &book, ui, w, h);
    }
    let tooltip = hovered_tooltip(game, &book, ui);
    if !tooltip.is_empty() {
        game.native_tooltip_box(batch, &tooltip, screen.mouse);
    }
}

/// `background_squares.png` tiled at 64 pixels, as the theme draws it.
fn draw_background(game: &Game<'_>, batch: &mut HudBatch, r: Rect, argb: u32) {
    let start = batch.vertices.len();
    if game.native_sprite_size(BACKGROUND).is_none() {
        game.native_fill(batch, r, 0xDC2A2A2A);
        return;
    }
    let mut y = r[1];
    while y < r[1] + r[3] {
        let mut x = r[0];
        while x < r[0] + r[2] {
            sprite(game, batch, BACKGROUND, [x, y, 64.0, 64.0], argb);
            x += 64.0;
        }
        y += 64.0;
    }
    batch.clip_since(start, r[0], r[1], r[0] + r[2], r[1] + r[3]);
}

fn hollow(game: &Game<'_>, batch: &mut HudBatch, r: Rect, argb: u32) {
    game.native_fill(batch, [r[0], r[1], r[2], 1.0], argb);
    game.native_fill(batch, [r[0], r[1] + r[3] - 1.0, r[2], 1.0], argb);
    game.native_fill(batch, [r[0], r[1] + 1.0, 1.0, r[3] - 2.0], argb);
    game.native_fill(batch, [r[0] + r[2] - 1.0, r[1] + 1.0, 1.0, r[3] - 2.0], argb);
}

/// `context_menu`: the rectangle with its rounded border, squares tinted #B4B4B4.
fn draw_box(game: &Game<'_>, batch: &mut HudBatch, r: Rect) {
    draw_background(game, batch, [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], 0xFFB4B4B4);
    game.native_fill(batch, [r[0] + 1.0, r[1], r[2] - 2.0, 1.0], BORDER);
    game.native_fill(batch, [r[0] + 1.0, r[1] + r[3] - 1.0, r[2] - 2.0, 1.0], BORDER);
    game.native_fill(batch, [r[0], r[1] + 1.0, 1.0, r[3] - 2.0], BORDER);
    game.native_fill(batch, [r[0] + r[2] - 1.0, r[1] + 1.0, 1.0, r[3] - 2.0], BORDER);
}

/// The strips at each side (`QuestScreen.drawBackground`).
fn draw_sides(game: &Game<'_>, batch: &mut HudBatch, w: f32, h: f32) {
    game.native_fill(batch, [SIDE - 1.0, 1.0, 1.0, h - 2.0], BORDER);
    game.native_fill(batch, [1.0, 1.0, SIDE - 2.0, h - 2.0], WIDGET_BACKGROUND);
    game.native_fill(batch, [w - SIDE, 1.0, 1.0, h - 2.0], BORDER);
    game.native_fill(batch, [w - SIDE + 1.0, 1.0, SIDE - 2.0, h - 2.0], WIDGET_BACKGROUND);
}

/// One dependency line, in repeats of the texture as FTB Quests' GL_REPEAT draws it.
#[allow(clippy::too_many_arguments)]
fn draw_connection(game: &Game<'_>, batch: &mut HudBatch, view: &View, from: [f32; 2], to: [f32; 2], half: f32, argb: u32, moving: f32) {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return;
    }
    // Outside the panel there is nothing to draw, however long the line is.
    let piece_shows = |a: [f32; 2], b: [f32; 2]| {
        let (x0, y0) = (a[0].min(b[0]) - half, a[1].min(b[1]) - half);
        let (x1, y1) = (a[0].max(b[0]) + half, a[1].max(b[1]) + half);
        view.shows([x0, y0, x1 - x0, y1 - y0])
    };
    if !piece_shows(from, to) {
        return;
    }
    // Too thin for the texture to show: a plain line, as far fewer quads.
    if half < 1.5 || game.native_sprite_size(DEPENDENCY).is_none() {
        game.native_quad(batch, line_quad(from, to, half), argb);
        return;
    }
    let repeat = (half * 2.0).max(1.0);
    let (ux, uy) = (dx / len, dy / len);
    // Along the line, `u` falls from `len / repeat + moving` at `from` to `moving`
    // at `to`: each quad spans one whole texture, cut at integer `u`.
    let mut at = 0.0f32;
    let phase = moving.rem_euclid(1.0);
    while at < len {
        let u_here = (len - at) / repeat + phase;
        let frac = u_here - u_here.floor();
        let frac = if frac <= 1e-3 { 1.0 } else { frac };
        let step = (frac * repeat).min(len - at).max(0.05);
        let next = at + step;
        let (a, b) = ([from[0] + ux * at, from[1] + uy * at], [from[0] + ux * next, from[1] + uy * next]);
        if piece_shows(a, b) {
            let quad = line_quad(a, b, half);
            let (u0, u1) = (frac, frac - step / repeat);
            game.native_sprite_quad(batch, DEPENDENCY, quad, [[u0, 1.0], [u0, 0.0], [u1.max(0.0), 0.0], [u1.max(0.0), 1.0]], argb);
        }
        at = next;
    }
}

fn draw_graph(game: &Game<'_>, batch: &mut HudBatch, book: &Book, ui: &mut Ui, interactive: bool) {
    let Some(view) = ui.view else { return };
    let Some(chapter) = book.chapters.get(ui.chapter) else { return };
    let progress = &ui.progress;
    let selected = ui.panel.as_ref().map(|p| p.quest);
    let start = batch.vertices.len();
    let mouse = ui.mouse;
    let over_graph = interactive && inside(view.panel, mouse);

    // The position of each quest shown, its own or a link's.
    let mut places: HashMap<Id, Node> = HashMap::new();
    let mut hovered: Option<Id> = None;
    let mut shown: Vec<(&Quest, Node)> = Vec::new();
    for quest in &chapter.quests {
        if progress.visible_in(book, chapter, quest, &mut ui.worked.memo) {
            let node = Node { x: quest.x, y: quest.y, w: quest.size, h: quest.size };
            places.insert(quest.id, node);
            shown.push((quest, node));
            if over_graph && inside(view.place(node), mouse) {
                hovered = Some(quest.id);
            }
        }
    }
    for link in &chapter.links {
        if let Some((linked, linked_chapter)) = book.quest(link.quest).zip(book.chapter_of(link.quest)) {
            if progress.visible_in(book, linked_chapter, linked, &mut ui.worked.memo) {
                let node = Node { x: link.x, y: link.y, w: link.size, h: link.size };
                places.entry(linked.id).or_insert(node);
                shown.push((linked, node));
                if over_graph && inside(view.place(node), mouse) {
                    hovered = Some(linked.id);
                }
            }
        }
    }

    // The lines under everything (`QuestPanel.drawOffsetBackground`).
    let half = (f64::from(view.zoom) * LINE_THICKNESS / 4.0 * 3.0) as f32;
    let time = elapsed() as f32;
    let hide_default = chapter.default_hide_dependency_lines;
    for &(quest, node) in &shown {
        if quest.hide_dependency_lines.get(hide_default) && hovered != Some(quest.id) {
            continue;
        }
        let chapter_of_quest = book.chapter_of(quest.id).unwrap_or(chapter);
        let unavailable = !progress.can_start_tasks(book, chapter_of_quest, quest);
        let complete = !unavailable && progress.is_completed(quest.id);
        let base = if complete {
            LINE_COMPLETED
        } else if unavailable {
            LINE_UNAVAILABLE
        } else {
            LINE_UNCOMPLETED
        };
        for dep in &quest.dependencies {
            let Some(&from) = places.get(dep) else { continue };
            if book.quest(*dep).is_some_and(|d| d.hide_dependent_lines) {
                continue;
            }
            let (a, b) = (view.centre(node), view.centre(from));
            let is_required = selected == Some(*dep) || hovered == Some(*dep);
            let is_requires = selected == Some(quest.id) || hovered == Some(quest.id);
            let (argb, moving) = if is_required {
                (LINE_REQUIRED_FOR, -time)
            } else if is_requires {
                (LINE_REQUIRES, -time)
            } else {
                (base, 0.0)
            };
            draw_connection(game, batch, &view, a, b, half.max(1.0), argb, moving);
        }
    }

    // Images, under the quests (`ChapterImageButton`), when their texture is known.
    for image in &chapter.images {
        if image.dev || image.dependency.is_some_and(|d| !progress.is_completed(d)) {
            continue;
        }
        let name = names::sprite_name(&image.image);
        let Some(size) = game.native_sprite_size(&name) else { continue };
        let r = view.place(Node { x: image.x, y: image.y, w: image.width, h: image.height });
        if !view.shows(r) {
            continue;
        }
        let tint = ((image.alpha.clamp(0, 255) as u32) << 24) | 0xFFFFFF;
        if image.rotation == 0.0 {
            game.native_sprite_tinted(batch, &name, [0.0, 0.0, size[0], size[1]], r, tint);
        } else {
            let (c, s) = (image.rotation.to_radians().cos() as f32, image.rotation.to_radians().sin() as f32);
            let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
            let corner = |dx: f32, dy: f32| [cx + dx * c - dy * s, cy + dx * s + dy * c];
            let (hw, hh) = (r[2] / 2.0, r[3] / 2.0);
            let points = [corner(-hw, -hh), corner(-hw, hh), corner(hw, hh), corner(hw, -hh)];
            game.native_sprite_quad(batch, &name, points, [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]], tint);
        }
    }

    // The quests (`QuestButton.draw`).
    for &(quest, node) in &shown {
        let r = view.place(node);
        if !view.shows(r) {
            continue;
        }
        let chapter_of_quest = book.chapter_of(quest.id).unwrap_or(chapter);
        draw_quest(game, batch, book, chapter_of_quest, quest, progress, r, selected == Some(quest.id), hovered == Some(quest.id));
        ui.hits.push((r, Hit::Quest(quest.id)));
    }
    batch.clip_since(start, view.panel[0], view.panel[1], view.panel[0] + view.panel[2], view.panel[1] + view.panel[3]);
}

#[allow(clippy::too_many_arguments)]
fn draw_quest(
    game: &Game<'_>,
    batch: &mut HudBatch,
    book: &Book,
    chapter: &Chapter,
    quest: &Quest,
    progress: &Progress,
    r: Rect,
    selected: bool,
    hovered: bool,
) {
    let completed = progress.is_completed(quest.id);
    let started = completed || progress.is_started(quest.id);
    let can_start = progress.can_start_tasks(book, chapter, quest);
    let deps_done = progress.deps_complete(quest);
    let mut outline = NOT_STARTED;
    let mut corner: Option<&str> = None;
    if can_start {
        if completed {
            corner = Some(if progress.has_unclaimed(quest) { ALERT } else { CHECK });
            outline = COMPLETED;
        } else if started {
            if deps_done {
                outline = STARTED;
            }
            let flexible = book.progression_of(chapter, quest) == Progression::Flexible;
            if flexible && quest.tasks.iter().all(|t| progress.task_count(t.id) >= t.max_progress()) {
                corner = Some(CHECK_GRAY);
            }
        }
    } else {
        outline = LOCKED;
    }

    let shape = book.quest_shape(chapter, quest);
    let part = |name: &str| format!("ftbquests_gui:gui/shapes/{shape}/{name}");
    let draws_shape = shape != "none";
    if draws_shape {
        sprite(game, batch, &part("shape"), r, SHAPE_FILL);
        sprite(game, batch, &part("background"), r, SHAPE_BACK);
        sprite(game, batch, &part("outline"), r, outline);
    }

    let s = (r[2] * (2.0 / 3.0) * quest.icon_scale as f32).floor();
    let icon = quest_icon(game, quest, progress);
    draw_icon(game, batch, &icon, r[0] + (r[2] - s) / 2.0, r[1] + (r[3] - s) / 2.0, s);

    if selected && draws_shape {
        let pulse = (190.0 + (elapsed() * 3.0).sin() * 50.0) as u32;
        let tint = (pulse << 24) | 0xFFFFFF;
        sprite(game, batch, &part("outline"), r, tint);
        sprite(game, batch, &part("background"), r, tint);
    }
    let mut lock = None;
    if !can_start || !deps_done {
        if draws_shape {
            sprite(game, batch, &part("shape"), r, 0x64000000);
        }
        if book.settings.show_lock_icons && !quest.hide_lock_icon {
            lock = Some(LOCK);
        }
    }
    if hovered && draws_shape {
        sprite(game, batch, &part("shape"), r, 0x64FFFFFF);
    }
    let cs = (r[2] / 8.0 * 3.0).floor();
    if let Some(name) = corner {
        sprite(game, batch, name, [r[0] + r[2] - cs, r[1], cs, cs], u32::MAX);
    }
    if let Some(name) = lock {
        sprite(game, batch, name, [r[0] + r[2] - cs, r[1] + r[3] - 1.0 - cs, cs, cs], u32::MAX);
    }
}

/// A row of the chapter list.
enum Row<'a> {
    Group(&'a str, Id, bool),
    Chapter(usize, &'a Chapter, bool),
}

/// The chapter list (`ChapterPanel`).
fn draw_chapters(game: &Game<'_>, batch: &mut HudBatch, book: &Book, ui: &mut Ui, h: f32) {
    let mouse = ui.mouse;
    let progress = &ui.progress;
    // The rows, with their groups.
    let mut rows: Vec<Row<'_>> = Vec::new();
    let shown = |i: usize| ui.worked.chapter_shown.get(i).copied().unwrap_or(false);
    for (i, chapter) in book.chapters.iter().enumerate() {
        if chapter.group == 0 && shown(i) {
            rows.push(Row::Chapter(i, chapter, false));
        }
    }
    for group in book.groups.iter().filter(|g| g.id != 0) {
        let visible: Vec<usize> = book
            .chapters
            .iter()
            .enumerate()
            .filter(|(i, c)| c.group == group.id && shown(*i))
            .map(|(i, _)| i)
            .collect();
        if visible.is_empty() {
            continue;
        }
        let collapsed = ui.collapsed.contains(&group.id);
        rows.push(Row::Group(&group.title, group.id, collapsed));
        if !collapsed {
            rows.extend(visible.into_iter().map(|i| Row::Chapter(i, &book.chapters[i], true)));
        }
    }
    // The width that fits the longest row (`ChapterPanel.alignWidgets`).
    let mut wd = 100.0f32;
    let tr = translator(game);
    let title_width = |raw: &str| game.native_text_width(&text::plain(raw, &tr));
    wd = wd.max(title_width(&book.title) + 44.0);
    for row in &rows {
        let w = match row {
            Row::Group(title, _, _) => title_width(title) + 34.0,
            Row::Chapter(_, c, grouped) => title_width(&c.title) + 34.0 + if *grouped { 7.0 } else { 0.0 },
        };
        wd = wd.max(w);
    }
    let wd = wd.min(ui.size[0] / 2.0);
    let r: Rect = [0.0, 0.0, wd, h];

    // `ExpandChaptersButton`: pinned it stays, else the side strip opens it while the mouse is near.
    let near = if ui.expanded { inside(r, mouse) } else { mouse[0] < SIDE };
    ui.expanded = !ui.unpinned || (ui.panel.is_none() && near);
    if !ui.expanded {
        // The arrow on the strip.
        sprite(game, batch, ARROW_RIGHT, [(SIDE - 12.0) / 2.0, (h - 12.0) / 2.0, 12.0, 12.0], u32::MAX);
        return;
    }

    draw_box(game, batch, r);
    ui.hits.push((r, Hit::Blocker));
    let over = inside(r, mouse);

    // The book's row (`ModpackButton`).
    let book_title = text::plain(&book.title, &tr);
    let icon = book.settings.icon.as_ref().map_or(Icon::Item(ftbq_world::BOOK_ITEM.to_owned()), item_icon);
    draw_icon(game, batch, &icon, 2.0, 1.0, 16.0);
    game.native_text(batch, &book_title, [21.0, 5.0], TEXT, false);
    let pin: Rect = [wd - 16.0, 3.0, 12.0, 12.0];
    sprite(game, batch, PIN, pin, if ui.unpinned { 0xFFDBDBDB } else { 0xFFF8BD1C });
    game.native_fill(batch, [1.0, HEADER - 1.0, wd - 2.0, 1.0], BORDER);
    ui.hits.push((pin, Hit::Pin));

    // The rows, scrolled and clipped.
    let list: Rect = [1.0, HEADER, wd - 2.0, h - HEADER - 1.0];
    let total = rows.len() as f32 * ROW;
    ui.list_scroll = ui.list_scroll.clamp(0.0, (total - list[3]).max(0.0));
    let start = batch.vertices.len();
    let mut y = list[1] - ui.list_scroll;
    for row in &rows {
        let row_rect: Rect = [0.0, y, wd, ROW];
        let hover = over && inside([0.0, y, wd, ROW], mouse) && inside(list, mouse);
        if y + ROW > list[1] && y < list[1] + list[3] {
            match row {
                Row::Group(title, id, collapsed) => {
                    if hover {
                        game.native_fill(batch, [1.0, y, wd - 2.0, ROW], 0x28FFFFFF);
                    }
                    let arrow = if *collapsed { ARROW_COLLAPSED } else { ARROW_EXPANDED };
                    sprite(game, batch, arrow, [3.0, y + 3.0, 8.0, 8.0], u32::MAX);
                    let title = text::plain(title, &tr);
                    let rgb = if hover { HOVER_TEXT } else { TEXT };
                    game.native_text(batch, &title, [13.0, y + 3.0], rgb, false);
                    game.native_fill(batch, [1.0, y + ROW - 1.0, wd - 2.0, 1.0], 0x28999999);
                    ui.hits.push((row_rect, Hit::Group(*id)));
                }
                Row::Chapter(index, chapter, grouped) => {
                    let x_off = if *grouped { 7.0 } else { 0.0 };
                    if *index == ui.chapter {
                        game.native_fill(batch, [1.0, y, wd - 2.0, ROW], 0x44999999);
                        hollow(game, batch, [1.0, y, wd - 2.0, ROW], 0xC0999999);
                    } else if hover {
                        game.native_fill(batch, [1.0, y, wd - 2.0, ROW], 0x28FFFFFF);
                    }
                    let icon = chapter.icon.as_ref().map_or(Icon::Item("minecraft:book".to_owned()), item_icon);
                    draw_icon(game, batch, &icon, 2.0 + x_off, y + 1.0, 12.0);
                    let rgb = if progress.is_completed(chapter.id) {
                        0x56FF56
                    } else if progress.is_started(chapter.id) {
                        0x00FFFF
                    } else {
                        TEXT
                    };
                    game.native_text(batch, &text::plain(&chapter.title, &tr), [16.0 + x_off, y + 3.0], rgb, false);
                    if ui.worked.chapter_waiting.get(*index).copied().unwrap_or(false) {
                        sprite(game, batch, ALERT, [wd - 12.0, y + 3.0, 8.0, 8.0], u32::MAX);
                    }
                    ui.hits.push((row_rect, Hit::Chapter(*index)));
                }
            }
        }
        y += ROW;
    }
    batch.clip_since(start, list[0], list[1], list[0] + list[2], list[1] + list[3]);
}

/// The right strip's Collect button (`CollectRewardsButton`).
fn draw_right(game: &Game<'_>, batch: &mut HudBatch, ui: &mut Ui, w: f32) {
    let r: Rect = [w - SIDE, 1.0, SIDE, SIDE];
    let waiting = ui.worked.any_waiting;
    let tint = if waiting { 0xFFEF9E1A } else { 0xFF6E6E6E };
    sprite(game, batch, COLLECT, [r[0] + 2.0, r[1] + 2.0, 16.0, 16.0], tint);
    if inside(r, ui.mouse) {
        game.native_fill(batch, [r[0] + 1.0, r[1], SIDE - 2.0, SIDE], 0x28FFFFFF);
    }
    if waiting {
        sprite(game, batch, ALERT, [r[0] + SIDE - 9.0, r[1] + 1.0, 8.0, 8.0], u32::MAX);
    }
    ui.hits.push((r, Hit::Collect));
}

// ---- The quest panel ---------------------------------------------------------

/// `CompactGridLayout`: rows that keep a grid near square.
fn grid_rows(count: usize) -> Vec<usize> {
    match count {
        0 => Vec::new(),
        1..=3 => vec![count],
        4 => vec![2, 2],
        _ => {
            let per = (count as f32).sqrt().ceil() as usize;
            let mut rows = Vec::new();
            let mut left = count;
            while left > 0 {
                let n = left.min(per);
                rows.push(n);
                left -= n;
            }
            rows
        }
    }
}

const BUTTON: f32 = 18.0;
const BUTTON_GAP: f32 = 2.0;

/// The buttons of one side of the panel, centred in `area`.
fn grid_places(count: usize, area: Rect) -> Vec<Rect> {
    let rows = grid_rows(count);
    let cell = BUTTON + BUTTON_GAP;
    let height = rows.len() as f32 * cell;
    let mut out = Vec::with_capacity(count);
    for (i, &n) in rows.iter().enumerate() {
        let row_width = n as f32 * cell;
        let x0 = area[0] + (area[2] - row_width) / 2.0 + BUTTON_GAP / 2.0;
        let y = area[1] + (area[3] - height) / 2.0 + i as f32 * cell + BUTTON_GAP / 2.0;
        for j in 0..n {
            out.push([x0 + j as f32 * cell, y, BUTTON, BUTTON]);
        }
    }
    out
}

/// One line of the panel's text.
enum PanelLine {
    Text(Vec<Run>, f32),
    Image(String, f32, f32, f32),
    Gap(f32),
}

/// The description's pages (`ViewQuestPanel.buildPageIndices`): lines between page breaks.
fn pages_of(description: &[String]) -> Vec<&[String]> {
    let mut pages = Vec::new();
    let mut start = 0;
    for (i, line) in description.iter().enumerate() {
        if line.trim() == text::PAGE_BREAK {
            pages.push(&description[start..i]);
            start = i + 1;
        }
    }
    pages.push(&description[start..]);
    pages
}

fn draw_quest_panel(game: &Game<'_>, batch: &mut HudBatch, book: &Book, ui: &mut Ui, w: f32, h: f32) {
    let Some(state) = &ui.panel else { return };
    let Some(quest) = book.quest(state.quest) else {
        ui.panel = None;
        return;
    };
    let chapter = book.chapter_of(quest.id).unwrap_or(&book.chapters[0]);
    let progress = &ui.progress;
    let tr = translator(game);
    let measure = |t: &str, s: Style| run_width(game, t, s);

    // The width (`ViewQuestPanel.addWidgets`).
    let title = quest_title(game, quest);
    let title_runs = text::parse_runs(if quest.title.is_empty() { &title } else { &quest.title }, &tr);
    let title_lines = text::wrap(&title_runs, 500.0_f32.min(w - 80.0), &measure);
    let title_w = title_lines.iter().map(|l| line_width(game, l)).fold(0.0, f32::max);
    let mut pw = 200.0f32.max(title_w.max(150.0) + 54.0);
    let min_width = if quest.min_width > 0 { quest.min_width } else { chapter.default_min_width };
    pw = pw.max(min_width as f32);
    pw = pw.min(w - 8.0).floor();
    if (pw as i32) % 2 == 0 {
        pw += 1.0;
    }
    let half = (pw / 2.0).floor();

    // Tasks and rewards.
    let task_rows = grid_rows(quest.tasks.len()).len() as f32;
    let reward_rows = grid_rows(quest.rewards.len()).len() as f32;
    let grid_h = (task_rows.max(reward_rows).max(1.0)) * (BUTTON + BUTTON_GAP);

    // The text, with its top where FTB Quests puts it.
    let show_text = !quest.hide_text_until_complete.get(false) || progress.is_completed(quest.id);
    let hide_details = quest.hide_details_until_startable.get(chapter.hide_quest_details_until_startable)
        && !progress.can_start_tasks(book, chapter, quest);
    let pages = pages_of(&quest.description);
    let page = state.page.min(pages.len().saturating_sub(1));
    let text_w = pw - 6.0;
    let mut lines: Vec<PanelLine> = Vec::new();
    if !quest.subtitle.is_empty() {
        let mut runs = text::parse_runs(&quest.subtitle, &tr);
        for run in &mut runs {
            run.style.italic = true;
            run.style.color = run.style.color.or(Some(0xAAAAAA));
        }
        for line in text::wrap(&runs, text_w, &measure) {
            let width = line_width(game, &line);
            lines.push(PanelLine::Text(line, (text_w - width) / 2.0));
        }
        lines.push(PanelLine::Gap(7.0));
    }
    if show_text && !hide_details {
        for raw in pages.get(page).copied().unwrap_or(&[]) {
            match text::parse_line(raw, &tr) {
                Line::Text(runs) => {
                    for line in text::wrap(&runs, text_w, &measure) {
                        lines.push(PanelLine::Text(line, 0.0));
                    }
                    lines.push(PanelLine::Gap(1.0));
                }
                Line::Image(image) => {
                    let name = names::sprite_name(&image.image);
                    if game.native_sprite_size(&name).is_some() {
                        let (iw, ih) = (image.width as f32, image.height as f32);
                        let scale = (text_w / iw).min(1.0);
                        let (iw, ih) = (iw * scale, ih * scale);
                        let x = match image.align.as_str() {
                            "left" => 0.0,
                            "right" => text_w - iw,
                            _ => (text_w - iw) / 2.0,
                        };
                        lines.push(PanelLine::Image(name, x, iw, ih));
                        lines.push(PanelLine::Gap(1.0));
                    }
                }
                Line::PageBreak => {}
            }
        }
    } else if !show_text || hide_details {
        let hidden = game.native_translate("ftbquests.quest.text_hidden", "Text hidden");
        let width = game.native_text_width(&hidden);
        lines.push(PanelLine::Text(plain_runs(&hidden), (text_w - width) / 2.0));
    }
    let line_h = |l: &PanelLine| match l {
        PanelLine::Text(..) => 9.0,
        PanelLine::Image(_, _, _, h) => *h,
        PanelLine::Gap(h) => *h,
    };
    let text_h: f32 = lines.iter().map(line_h).sum();
    let paged = pages.len() > 1 && show_text && !hide_details;
    let pager_h = if paged { 14.0 } else { 0.0 };

    let title_h = title_lines.len() as f32 * 10.0;
    let content_top = 16.0f32.max(title_h + 8.0);
    let body_h = 16.0 + grid_h + 12.0 + text_h + pager_h + 4.0;
    let has_text = !lines.is_empty();
    let inner_h = if has_text { body_h } else { 16.0 + grid_h + 8.0 };
    let ph = (content_top + inner_h).min(h - 10.0).floor();
    let r: Rect = [((w - pw) / 2.0).floor(), ((h - ph) / 2.0).floor(), pw, ph];
    let content: Rect = [r[0], r[1] + content_top, pw, ph - content_top];

    draw_box(game, batch, r);
    ui.hits.push((r, Hit::Blocker));
    ui.panel_rect = Some(r);

    // The title bar.
    let icon_size = 16.0f32.min(title_h + 2.0);
    let icon = quest_icon(game, quest, progress);
    draw_icon(game, batch, &icon, r[0] + 4.0, r[1] + 4.0, icon_size);
    for (i, line) in title_lines.iter().enumerate() {
        let lw = line_width(game, line);
        draw_line(game, batch, line, r[0] + (pw - lw) / 2.0, r[1] + 4.0 + i as f32 * 10.0, TITLE_TEXT, false);
    }
    let close: Rect = [r[0] + pw - icon_size - 2.0, r[1] + 4.0, icon_size, icon_size];
    sprite(game, batch, CLOSE, close, u32::MAX);
    ui.hits.push((close, Hit::ClosePanel));
    game.native_fill(batch, [r[0] + 1.0, content[1], pw - 2.0, 1.0], BORDER);

    // What scrolls.
    let max_scroll = (inner_h - content[3]).max(0.0);
    let scroll = {
        let state = ui.panel.as_mut().expect("checked above");
        state.page = page;
        state.scroll = state.scroll.clamp(0.0, max_scroll);
        state.scroll
    };
    let start = batch.vertices.len();
    let top = content[1] - scroll;
    let center = |t: &str| game.native_text_width(t) / 2.0;
    let tasks_label = game.native_translate("ftbquests.tasks", "Tasks");
    let rewards_label = game.native_translate("ftbquests.rewards", "Rewards");
    let mid = r[0] + half;
    game.native_text(batch, &tasks_label, [r[0] + half / 2.0 - center(&tasks_label), top + 5.0], TASKS_TEXT, false);
    game.native_text(batch, &rewards_label, [mid + half / 2.0 - center(&rewards_label), top + 5.0], REWARDS_TEXT, false);
    let dividers_h = 16.0 + grid_h + 6.0;
    game.native_fill(batch, [mid, top, 1.0, dividers_h], BORDER);
    if has_text {
        game.native_fill(batch, [r[0] + 1.0, top + dividers_h, pw - 2.0, 1.0], BORDER);
    }

    let tasks_area: Rect = [r[0] + 2.0, top + 16.0, half - 3.0, grid_h];
    let rewards_area: Rect = [mid + 2.0, top + 16.0, half - 3.0, grid_h];
    let can_start = progress.can_start_tasks(book, chapter, quest);
    let mouse = ui.mouse;
    if quest.tasks.is_empty() {
        let none = game.native_translate("ftbquests.gui.no_tasks", "No tasks");
        game.native_text(batch, &none, [tasks_area[0] + tasks_area[2] / 2.0 - center(&none), tasks_area[1] + 5.0], DISABLED_TEXT, false);
    }
    if quest.rewards.is_empty() {
        let none = game.native_translate("ftbquests.gui.no_rewards", "No rewards");
        game.native_text(batch, &none, [rewards_area[0] + rewards_area[2] / 2.0 - center(&none), rewards_area[1] + 5.0], DISABLED_TEXT, false);
    }
    let mut hits = Vec::new();
    for (task, place) in quest.tasks.iter().zip(grid_places(quest.tasks.len(), tasks_area)) {
        let hover = inside(place, mouse) && inside(content, mouse);
        draw_task(game, batch, task, progress, can_start, place, hover);
        hits.push((place, Hit::Task(task.id)));
    }
    for (reward, place) in quest.rewards.iter().zip(grid_places(quest.rewards.len(), rewards_area)) {
        let hover = inside(place, mouse) && inside(content, mouse);
        draw_reward(game, batch, reward, progress, quest, place, hover);
        hits.push((place, Hit::Reward(reward.id)));
    }

    // The description.
    let mut y = top + 16.0 + grid_h + 12.0;
    for line in &lines {
        match line {
            PanelLine::Text(runs, x) => draw_line(game, batch, runs, r[0] + 3.0 + *x, y, TEXT, false),
            PanelLine::Image(name, x, iw, ih) => {
                sprite(game, batch, name, [r[0] + 3.0 + *x, y, *iw, *ih], u32::MAX);
            }
            PanelLine::Gap(_) => {}
        }
        y += line_h(line);
    }
    // Pages.
    if paged {
        let label = format!("{} / {}", page + 1, pages.len());
        let lw = game.native_text_width(&label);
        let cx = r[0] + pw / 2.0;
        game.native_text(batch, &label, [cx - lw / 2.0, y + 2.0], DISABLED_TEXT, false);
        let prev: Rect = [cx - lw / 2.0 - 14.0, y + 1.0, 10.0, 10.0];
        let next: Rect = [cx + lw / 2.0 + 4.0, y + 1.0, 10.0, 10.0];
        if page > 0 {
            sprite(game, batch, ARROW_LEFT, prev, u32::MAX);
            hits.push((prev, Hit::PrevPage));
        }
        if page + 1 < pages.len() {
            sprite(game, batch, ARROW_RIGHT, next, u32::MAX);
            hits.push((next, Hit::NextPage));
        }
    }
    batch.clip_since(start, content[0] + 1.0, content[1] + 1.0, content[0] + content[2] - 1.0, content[1] + content[3] - 1.0);
    // Only what shows can be clicked.
    for (place, hit) in hits {
        if place[1] + place[3] > content[1] && place[1] < content[1] + content[3] {
            ui.hits.push((place, hit));
        }
    }
}

/// `TaskButton.draw`.
fn draw_task(game: &Game<'_>, batch: &mut HudBatch, task: &Task, progress: &Progress, can_start: bool, r: Rect, hover: bool) {
    if hover {
        game.native_fill(batch, [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], WIDGET_BACKGROUND);
        hollow(game, batch, r, BORDER);
    }
    let done = progress.is_completed(task.id);
    draw_icon(game, batch, &task_icon(game, task, done), r[0] + 1.0, r[1] + 1.0, 16.0);
    if done {
        sprite(game, batch, CHECK, [r[0] + r[2] - 9.0, r[1] + 1.0, 8.0, 8.0], u32::MAX);
    } else if !task.is_supported() {
        // A type this port can't complete yet: its text shows in the tooltip.
        sprite(game, batch, LOCK, [r[0] + r[2] - 9.0, r[1] + r[3] - 9.0, 8.0, 8.0], u32::MAX);
    } else if task.max_progress() > 1 {
        // `Task.getButtonText`: its count, at half size.
        let label = format!("{}/{}", progress.task_count(task.id).min(task.max_progress()), task.max_progress());
        half_text(game, batch, &label, r[0] + 19.0, r[1] + 15.0);
    } else if !can_start {
        sprite(game, batch, LOCK, [r[0] + r[2] - 9.0, r[1] + r[3] - 9.0, 8.0, 8.0], u32::MAX);
    }
}

/// `RewardButton.draw`.
fn draw_reward(game: &Game<'_>, batch: &mut HudBatch, reward: &Reward, progress: &Progress, quest: &Quest, r: Rect, hover: bool) {
    if hover {
        game.native_fill(batch, [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], WIDGET_BACKGROUND);
        hollow(game, batch, r, BORDER);
    }
    draw_icon(game, batch, &reward_icon(reward), r[0] + 1.0, r[1] + 1.0, 16.0);
    if progress.is_claimed(reward.id) {
        sprite(game, batch, CHECK, [r[0] + r[2] - 9.0, r[1] + 1.0, 8.0, 8.0], 0xC0FFFFFF);
    } else if !reward.is_supported() {
        sprite(game, batch, LOCK, [r[0] + r[2] - 9.0, r[1] + r[3] - 9.0, 8.0, 8.0], u32::MAX);
    } else {
        if let RewardKind::Item { count, .. } = &reward.kind
            && *count > 1
        {
            half_text(game, batch, &count.to_string(), r[0] + 19.0, r[1] + 15.0);
        }
        if progress.is_completed(quest.id) {
            sprite(game, batch, ALERT, [r[0] + r[2] - 9.0, r[1] + 1.0, 8.0, 8.0], u32::MAX);
        }
    }
}

/// Text at half size, its middle at `x`.
fn half_text(game: &Game<'_>, batch: &mut HudBatch, label: &str, right: f32, y: f32) {
    let x = right - game.native_text_width(label) / 2.0;
    let start = batch.vertices.len();
    game.native_text(batch, label, [x, y], 0xFFFFFF, true);
    batch.scale_since(start, x, y, 0.5, 0.5);
}

// ---- Tooltips ----------------------------------------------------------------

fn gray(line: String) -> (Text, u32) {
    (Text::colored(line, 0xAAAAAA), u32::MAX)
}

fn hovered_tooltip(game: &Game<'_>, book: &Book, ui: &Ui) -> Vec<(Text, u32)> {
    if ui.panel.is_some() && ui.panel_rect.is_some_and(|r| !inside(r, ui.mouse)) {
        return Vec::new();
    }
    let Some(&(_, hit)) = ui.hits.iter().rev().find(|(r, _)| inside(*r, ui.mouse)) else {
        return Vec::new();
    };
    let tr = translator(game);
    let progress = &ui.progress;
    let mut lines: Vec<(Text, u32)> = Vec::new();
    match hit {
        Hit::Quest(id) => {
            let Some(quest) = book.quest(id) else { return lines };
            lines.push((Text::literal(quest_title(game, quest)), u32::MAX));
            if !quest.subtitle.is_empty() {
                lines.push(gray(text::plain(&quest.subtitle, &tr)));
            }
            let percent = progress.relative(quest);
            if percent > 0 && percent < 100 {
                lines.push(gray(format!("{percent}%")));
            }
            if quest.optional {
                lines.push(gray(game.native_translate("ftbquests.quest.misc.optional", "Optional")));
            }
        }
        Hit::Chapter(index) => {
            let Some(chapter) = book.chapters.get(index) else { return lines };
            lines.push((Text::literal(text::plain(&chapter.title, &tr)), u32::MAX));
            for line in &chapter.subtitle {
                lines.push(gray(text::plain(line, &tr)));
            }
            lines.push(gray(format!("{}%", progress.chapter_relative(chapter))));
        }
        Hit::Task(id) => {
            let Some((task, quest, chapter)) = book.task(id) else { return lines };
            lines.push((Text::literal(task_title(game, task)), u32::MAX));
            if let TaskKind::Item { item, count, filter, .. } = &task.kind {
                if filter.is_none() && game.native_item_known(&item.id) {
                    let stack = ItemStack::new(&*item.id, 1);
                    lines.extend(game.native_stack_tooltip(&stack).into_iter().skip(1));
                }
                if !game.native_item_known(&item.id) && filter.is_none() {
                    lines.push(gray(format!("{} (not in the game yet)", item.id)));
                }
                if *count > 1 {
                    lines.push(gray(format!("{} / {count}", progress.task_count(id).min(*count))));
                }
            }
            if progress.is_completed(id) {
                lines.push((Text::colored(game.native_translate("ftbquests.task.completed", "Task completed!"), 0x55FF55), u32::MAX));
            } else if !task.is_supported() {
                lines.push((Text::colored("Not supported yet", 0xFF5555), u32::MAX));
            } else if !progress.can_start_tasks(book, chapter, quest) {
                lines.push((Text::colored(game.native_translate("ftbquests.task.locked", "Locked"), 0xFF5555), u32::MAX));
            } else if let TaskKind::Item { consume, .. } = &task.kind {
                if book.consumes(chapter, *consume) {
                    lines.push((Text::colored(game.native_translate("ftbquests.task.click_to_submit", "Click to submit"), 0xFFFF55), u32::MAX));
                }
            } else {
                lines.push((Text::colored(game.native_translate("ftbquests.task.click_to_submit", "Click to submit"), 0xFFFF55), u32::MAX));
            }
        }
        Hit::Reward(id) => {
            let Some((reward, quest)) = book.reward(id) else { return lines };
            lines.push((Text::literal(reward_title(game, book, reward)), u32::MAX));
            if let RewardKind::Item { item, .. } = &reward.kind {
                if game.native_item_known(&item.id) {
                    let stack = ItemStack::new(&*item.id, 1);
                    lines.extend(game.native_stack_tooltip(&stack).into_iter().skip(1));
                } else {
                    lines.push(gray(format!("{} (not in the game yet)", item.id)));
                }
            }
            if progress.is_claimed(id) {
                lines.push((Text::colored(game.native_translate("ftbquests.reward.claimed", "Claimed"), 0x55FF55), u32::MAX));
            } else if !reward.is_supported() {
                lines.push((Text::colored("Not supported yet", 0xFF5555), u32::MAX));
            } else if progress.is_completed(quest.id) {
                lines.push((Text::colored(game.native_translate("ftbquests.reward.click_to_claim", "Click to claim"), 0xFFFF55), u32::MAX));
            }
        }
        Hit::Collect => {
            lines.push((Text::literal(game.native_translate("ftbquests.gui.collect_rewards", "Click to collect all rewards!")), u32::MAX));
        }
        Hit::Pin => {
            lines.push((Text::literal(game.native_translate("ftbquests.gui.pin", "Pin")), u32::MAX));
        }
        Hit::Blocker | Hit::Group(_) | Hit::ClosePanel | Hit::PrevPage | Hit::NextPage => {}
    }
    lines
}

// ---- Input --------------------------------------------------------------------

/// `native_screen_mouse_button_pressed_pre`.
pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    *canceled = true;
    UI.with(|ui| {
        let mut ui = ui.borrow_mut();
        let at = screen.mouse;
        let hit = ui.hits.iter().rev().find(|(r, _)| inside(*r, at)).map(|&(_, h)| h);
        // A click beside the quest panel closes it (`ModalPanel`).
        if ui.panel.is_some() && ui.panel_rect.is_some_and(|r| !inside(r, at)) {
            ui.panel = None;
            return;
        }
        if button != 0 {
            return;
        }
        let Some(book) = ui.book.clone() else { return };
        match hit {
            Some(Hit::Chapter(index)) => {
                ui.actions.push(Action::Click);
                ui.chapter = index;
                ui.panel = None;
            }
            Some(Hit::Group(id)) => {
                ui.actions.push(Action::Click);
                if !ui.collapsed.remove(&id) {
                    ui.collapsed.insert(id);
                }
            }
            Some(Hit::Pin) => {
                ui.actions.push(Action::Click);
                ui.unpinned = !ui.unpinned;
            }
            Some(Hit::Collect) => {
                ui.actions.push(Action::Click);
                ui.actions.push(Action::Send(ToServer::ClaimAll));
            }
            Some(Hit::Quest(id)) => {
                ui.actions.push(Action::Click);
                ui.panel = Some(PanelState { quest: id, scroll: 0.0, page: 0 });
                ui.panel_rect = None;
            }
            Some(Hit::ClosePanel) => {
                ui.actions.push(Action::Click);
                ui.panel = None;
            }
            Some(Hit::PrevPage) => {
                ui.actions.push(Action::Click);
                if let Some(p) = &mut ui.panel {
                    p.page = p.page.saturating_sub(1);
                    p.scroll = 0.0;
                }
            }
            Some(Hit::NextPage) => {
                ui.actions.push(Action::Click);
                if let Some(p) = &mut ui.panel {
                    p.page += 1;
                    p.scroll = 0.0;
                }
            }
            Some(Hit::Task(id)) => {
                let can_click = book.task(id).is_some_and(|(task, quest, chapter)| {
                    task.is_supported() && !ui.progress.is_completed(id) && ui.progress.can_start_tasks(&book, chapter, quest)
                });
                if can_click {
                    ui.actions.push(Action::Click);
                    ui.actions.push(Action::Send(ToServer::Submit(id)));
                }
            }
            Some(Hit::Reward(id)) => {
                if ui.progress.can_claim(&book, id) {
                    ui.actions.push(Action::Click);
                    ui.actions.push(Action::Send(ToServer::Claim(id)));
                }
            }
            Some(Hit::Blocker) => {}
            None => {
                if ui.view.is_some_and(|v| inside(v.panel, at)) {
                    ui.dragging = true;
                }
            }
        }
    });
}

/// `native_screen_mouse_button_released_pre`.
pub(super) fn released(game: &Game<'_>, button: i32) {
    if game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    if button == 0 {
        UI.with(|ui| ui.borrow_mut().dragging = false);
    }
}

/// `native_screen_mouse_dragged_pre`: the graph follows the mouse.
pub(super) fn dragged(game: &Game<'_>, button: i32, delta: [f32; 2]) {
    if button != 0 || game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    UI.with(|ui| {
        let mut ui = ui.borrow_mut();
        if ui.dragging
            && let Some(view) = &mut ui.view
        {
            view.drag(f64::from(delta[0]), f64::from(delta[1]));
        }
    });
}

/// `native_screen_mouse_scrolled_pre`: the wheel zooms the graph
/// (`QuestPanel.scrollPanel`), or scrolls the list or the panel under it.
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) {
    if game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    UI.with(|ui| {
        let mut ui = ui.borrow_mut();
        let at = screen.mouse;
        let steps = scroll[1] as f32;
        if let Some(panel) = &mut ui.panel {
            panel.scroll -= steps * 12.0;
            return;
        }
        let on_list = ui.expanded && ui.hits.iter().any(|(r, h)| *h == Hit::Blocker && inside(*r, at));
        if on_list {
            ui.list_scroll -= steps * 14.0;
            return;
        }
        if let Some(view) = &mut ui.view
            && inside(view.panel, at)
        {
            view.add_zoom(scroll[1]);
        }
    });
}

/// `native_screen_key_pressed_pre`: the mod's keys over the screen
/// (`FTBQuestsKeyMappings`), and Escape backs out of a quest first.
pub(super) fn key_over(game: &Game<'_>, key: &str, canceled: &mut bool) {
    if game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    UI.with(|ui| {
        let mut ui = ui.borrow_mut();
        match key {
            "key.keyboard.escape" => {
                if ui.panel.take().is_some() {
                    *canceled = true;
                }
            }
            "key.keyboard.equal" | "key.keyboard.minus" | "key.keyboard.0" | "key.keyboard.space" => {
                *canceled = true;
                if let Some(view) = &mut ui.view {
                    match key {
                        "key.keyboard.equal" => {
                            view.add_zoom(1.0);
                        }
                        "key.keyboard.minus" => {
                            view.add_zoom(-1.0);
                        }
                        "key.keyboard.0" => {
                            let back = (f64::from(ftbquests_core::graph::ZOOM_DEFAULT) - f64::from(view.zoom)) / 4.0;
                            view.add_zoom(back);
                        }
                        _ => view.recentre(),
                    }
                }
            }
            "key.keyboard.tab" => {
                *canceled = true;
                let Some(book) = ui.book.clone() else { return };
                let count = book.chapters.len();
                if count > 0 {
                    let mut next = (ui.chapter + 1) % count;
                    while !ui.worked.chapter_shown.get(next).copied().unwrap_or(false) && next != ui.chapter {
                        next = (next + 1) % count;
                    }
                    ui.chapter = next;
                }
            }
            // The key that opens the book, or the inventory's, closes it (`QuestScreen.keyPressed`).
            "key.keyboard.e" => {
                *canceled = true;
                ui.actions.push(Action::Close);
            }
            _ => {}
        }
    });
}

// ---- A view for tests ----------------------------------------------------------

/// With `FTBQ_VIEW_FILE` set, what the screen shows goes to that file as it
/// changes, for a test to read: the clickable places, and which is what.
fn dump_view(game: &Game<'_>) {
    static FILE: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    let Some(file) = FILE.get_or_init(|| std::env::var_os("FTBQ_VIEW_FILE").map(Into::into)) else {
        return;
    };
    thread_local! {
        static LAST: RefCell<String> = const { RefCell::new(String::new()) };
    }
    let text = view_json(game);
    LAST.with(|last| {
        let mut last = last.borrow_mut();
        if *last != text {
            let tmp = file.with_extension("tmp");
            if std::fs::write(&tmp, &text).and_then(|()| std::fs::rename(&tmp, file)).is_ok() {
                *last = text;
            }
        }
    });
}

fn view_json(game: &Game<'_>) -> String {
    let mut out = serde_json::Map::new();
    let open = game.native_mod_screen() == Some(MOD_ID);
    out.insert("open".into(), open.into());
    UI.with(|ui| {
        let ui = ui.borrow();
        out.insert("size".into(), serde_json::json!(ui.size));
        out.insert("chapter".into(), ui.chapter.into());
        if let Some(book) = &ui.book {
            out.insert("chapters".into(), book.chapters.len().into());
            out.insert("quests".into(), book.quests().count().into());
            if let Some(chapter) = book.chapters.get(ui.chapter) {
                out.insert("chapter_file".into(), chapter.filename.clone().into());
            }
        }
        if let Some(view) = &ui.view {
            out.insert("zoom".into(), view.zoom.into());
            out.insert("scroll".into(), serde_json::json!([view.scroll_x, view.scroll_y]));
            out.insert("panel_rect".into(), serde_json::json!(view.panel));
        }
        out.insert("quest".into(), ui.panel.as_ref().map(|p| id_string(p.quest)).into());
        let hits: Vec<serde_json::Value> = ui
            .hits
            .iter()
            .map(|(r, h)| {
                let (kind, id) = match h {
                    Hit::Blocker => ("blocker", String::new()),
                    Hit::Chapter(i) => ("chapter", i.to_string()),
                    Hit::Group(id) => ("group", id_string(*id)),
                    Hit::Pin => ("pin", String::new()),
                    Hit::Collect => ("collect", String::new()),
                    Hit::Quest(id) => ("quest", id_string(*id)),
                    Hit::ClosePanel => ("close", String::new()),
                    Hit::PrevPage => ("prev", String::new()),
                    Hit::NextPage => ("next", String::new()),
                    Hit::Task(id) => ("task", id_string(*id)),
                    Hit::Reward(id) => ("reward", id_string(*id)),
                };
                serde_json::json!({ "kind": kind, "id": id, "rect": r })
            })
            .collect();
        out.insert("hits".into(), hits.into());
        // What each item on show is, for a test to pick its way through real data.
        if let Some(book) = &ui.book {
            let quests: Vec<serde_json::Value> = ui
                .hits
                .iter()
                .filter_map(|(_, h)| if let Hit::Quest(id) = h { book.quest(*id) } else { None })
                .map(|q| {
                    let tasks: Vec<serde_json::Value> = q
                        .tasks
                        .iter()
                        .map(|t| {
                            let item = if let TaskKind::Item { item, count, .. } = &t.kind {
                                serde_json::json!({ "item": item.id, "count": count })
                            } else {
                                serde_json::Value::Null
                            };
                            serde_json::json!({ "id": id_string(t.id), "type": t.type_name(), "detail": item, "done": ui.progress.is_completed(t.id) })
                        })
                        .collect();
                    let rewards: Vec<serde_json::Value> = q
                        .rewards
                        .iter()
                        .map(|r| {
                            let item = if let RewardKind::Item { item, count, .. } = &r.kind {
                                serde_json::json!({ "item": item.id, "count": count })
                            } else {
                                serde_json::Value::Null
                            };
                            serde_json::json!({ "id": id_string(r.id), "type": r.type_name(), "detail": item, "claimed": ui.progress.is_claimed(r.id) })
                        })
                        .collect();
                    serde_json::json!({
                        "id": id_string(q.id),
                        "tasks": tasks,
                        "rewards": rewards,
                        "dependencies": q.dependencies.iter().map(|d| id_string(*d)).collect::<Vec<_>>(),
                        "completed": ui.progress.is_completed(q.id),
                        "startable": book.chapter_of(q.id).is_some_and(|c| ui.progress.can_start_tasks(book, c, q)),
                        "deps_done": ui.progress.deps_complete(q),
                    })
                })
                .collect();
            out.insert("shown".into(), quests.into());
        }
    });
    serde_json::Value::Object(out).to_string()
}

/// `native_mods`: the row in Options > Mods, with the mod's key, unbound as the mod ships it.
pub(super) fn listed(mods: &mut Vec<super::native_hooks::NativeMod>) {
    mods.push(super::native_hooks::NativeMod {
        id: MOD_ID.into(),
        title: "FTB Quests 0.1.0".into(),
        description: "The quest book, from your instance's config/ftbquests. By the FTB Team's FTB Quests.".into(),
        keys: vec![["quests".into(), "Open Quests".into(), "key.keyboard.unknown".into()]],
    });
}
