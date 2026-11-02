//! The chapter graph's geometry (`QuestPanel`, `QuestScreen`): where a quest
//! sits at a zoom and a scroll, how far the panel scrolls, and which button is
//! under the mouse. Zoom goes in steps of 4 from 4 to 28, and starts at 16.

/// A quest or link: its grid position and size, in quest units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub const ZOOM_MIN: i32 = 4;
pub const ZOOM_MAX: i32 = 28;
pub const ZOOM_DEFAULT: i32 = 16;

/// `QuestPanel.updateMinMax`: the box the nodes span plus its margin, in quest units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn of(nodes: impl IntoIterator<Item = Node>) -> Self {
        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for n in nodes {
            min_x = min_x.min(n.x - n.w / 2.0);
            min_y = min_y.min(n.y - n.h / 2.0);
            max_x = max_x.max(n.x + n.w / 2.0);
            max_y = max_y.max(n.y + n.h / 2.0);
        }
        if min_x == f64::INFINITY {
            (min_x, min_y, max_x, max_y) = (0.0, 0.0, 0.0, 0.0);
        }
        Bounds { min_x: min_x - 40.0, min_y: min_y - 30.0, max_x: max_x + 40.0, max_y: max_y + 30.0 }
    }
}

/// The panel's view: its rectangle on screen, zoom and scroll, all in GUI pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub panel: [f32; 4],
    pub zoom: i32,
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub bounds: Bounds,
}

impl View {
    pub fn new(panel: [f32; 4], bounds: Bounds) -> Self {
        let mut view = View { panel, zoom: ZOOM_DEFAULT, scroll_x: 0.0, scroll_y: 0.0, bounds };
        view.recentre();
        view
    }
    /// `QuestScreen.getQuestButtonSize`.
    pub fn button_size(&self) -> f64 {
        f64::from(self.zoom) * 3.0 / 2.0
    }
    /// `QuestScreen.getQuestButtonSpacing`, at the default spacing of 1.
    pub fn spacing(&self) -> f64 {
        f64::from(self.zoom) / 4.0
    }
    fn cell(&self) -> f64 {
        self.button_size() + self.spacing()
    }
    /// `scrollWidth`, `scrollHeight`.
    pub fn content(&self) -> (f64, f64) {
        let cell = self.cell();
        ((self.bounds.max_x - self.bounds.min_x) * cell, (self.bounds.max_y - self.bounds.min_y) * cell)
    }
    /// A node's rectangle on screen: `QuestPanel.alignWidgets`, offset by the scroll.
    pub fn place(&self, n: Node) -> [f32; 4] {
        let (bs, bp) = (self.button_size(), self.spacing());
        let x = (n.x - self.bounds.min_x - n.w / 2.0) * (bs + bp) + bp / 2.0 + bp * (n.w - 1.0) / 2.0;
        let y = (n.y - self.bounds.min_y - n.h / 2.0) * (bs + bp) + bp / 2.0 + bp * (n.h - 1.0) / 2.0;
        [
            self.panel[0] + (x.round() - self.scroll_x) as f32,
            self.panel[1] + (y.round() - self.scroll_y) as f32,
            (bs * n.w).round() as f32,
            (bs * n.h).round() as f32,
        ]
    }
    /// The centre of a node on screen.
    pub fn centre(&self, n: Node) -> [f32; 2] {
        let r = self.place(n);
        [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
    }
    /// `QuestPanel.resetScroll`: the content's middle in the panel's.
    pub fn recentre(&mut self) {
        let (w, h) = self.content();
        self.scroll_x = (w - f64::from(self.panel[2])) / 2.0;
        self.scroll_y = (h - f64::from(self.panel[3])) / 2.0;
    }
    /// A drag by `dx`, `dy` pixels: clamped to the content, or centred when it
    /// fits (`QuestScreen.drawBackground`).
    pub fn drag(&mut self, dx: f64, dy: f64) {
        let (w, h) = self.content();
        let (pw, ph) = (f64::from(self.panel[2]), f64::from(self.panel[3]));
        self.scroll_x = if w > pw { (self.scroll_x - dx).clamp(0.0, w - pw) } else { (w - pw) / 2.0 };
        self.scroll_y = if h > ph { (self.scroll_y - dy).clamp(0.0, h - ph) } else { (h - ph) / 2.0 };
    }
    /// The quest-grid position at the panel's centre.
    pub fn centre_position(&self) -> (f64, f64) {
        let (w, h) = self.content();
        let (dx, dy) = (self.bounds.max_x - self.bounds.min_x, self.bounds.max_y - self.bounds.min_y);
        (
            (self.scroll_x + f64::from(self.panel[2]) / 2.0) / w * dx + self.bounds.min_x,
            (self.scroll_y + f64::from(self.panel[3]) / 2.0) / h * dy + self.bounds.min_y,
        )
    }
    /// `QuestPanel.scrollTo`: a grid position at the panel's centre.
    pub fn scroll_to(&mut self, x: f64, y: f64) {
        let (w, h) = self.content();
        let (dx, dy) = (self.bounds.max_x - self.bounds.min_x, self.bounds.max_y - self.bounds.min_y);
        self.scroll_x = (x - self.bounds.min_x) / dx * w - f64::from(self.panel[2]) / 2.0;
        self.scroll_y = (y - self.bounds.min_y) / dy * h - f64::from(self.panel[3]) / 2.0;
    }
    /// `QuestScreen.addZoom`: keeps the middle of the view where it was.
    pub fn add_zoom(&mut self, up: f64) -> bool {
        let next = (f64::from(self.zoom) + up * 4.0).clamp(f64::from(ZOOM_MIN), f64::from(ZOOM_MAX)) as i32;
        if next == self.zoom {
            return false;
        }
        let (x, y) = self.centre_position();
        self.zoom = next;
        self.scroll_to(x, y);
        true
    }
    /// Whether a screen rectangle shows in the panel at all.
    pub fn shows(&self, r: [f32; 4]) -> bool {
        r[0] < self.panel[0] + self.panel[2] && r[1] < self.panel[1] + self.panel[3] && r[0] + r[2] > self.panel[0] && r[1] + r[3] > self.panel[1]
    }
}

/// A connection's quad between two points, as thick as `half` to each side.
pub fn line_quad(from: [f32; 2], to: [f32; 2], half: f32) -> [[f32; 2]; 4] {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (nx, ny) = (-dy / len * half, dx / len * half);
    [
        [from[0] + nx, from[1] + ny],
        [from[0] - nx, from[1] - ny],
        [to[0] - nx, to[1] - ny],
        [to[0] + nx, to[1] + ny],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(x: f64, y: f64) -> Node {
        Node { x, y, w: 1.0, h: 1.0 }
    }

    fn view() -> View {
        View::new([20.0, 1.0, 440.0, 268.0], Bounds::of([node(0.0, 0.0), node(10.0, 4.0)]))
    }

    #[test]
    fn bounds_span_the_nodes_with_the_mods_margin() {
        let b = Bounds::of([node(0.0, 0.0), node(10.0, 4.0)]);
        assert_eq!((b.min_x, b.max_x), (-40.5, 50.5));
        assert_eq!((b.min_y, b.max_y), (-30.5, 34.5));
        let empty = Bounds::of([]);
        assert_eq!((empty.min_x, empty.max_y), (-40.0, 30.0));
    }

    #[test]
    fn a_quest_is_as_big_as_the_mod_makes_it_at_each_zoom() {
        let mut v = view();
        assert_eq!(v.button_size(), 24.0);
        assert_eq!(v.spacing(), 4.0);
        assert_eq!(v.place(node(0.0, 0.0))[2], 24.0);
        v.zoom = 28;
        assert_eq!(v.place(node(0.0, 0.0))[2], 42.0);
        let big = Node { x: 0.0, y: 0.0, w: 2.0, h: 2.0 };
        assert_eq!(v.place(big)[2], 84.0);
    }

    #[test]
    fn neighbours_sit_a_cell_apart() {
        let v = view();
        let (a, b) = (v.place(node(0.0, 0.0)), v.place(node(1.0, 0.0)));
        assert_eq!(b[0] - a[0], 28.0);
    }

    #[test]
    fn a_view_starts_centred() {
        let v = view();
        let c = v.centre_position();
        assert!((c.0 - 5.0).abs() < 1e-6, "{c:?}");
        assert!((c.1 - 2.0).abs() < 1e-6, "{c:?}");
    }

    #[test]
    fn zoom_keeps_the_middle_and_stops_at_its_limits() {
        let mut v = view();
        v.scroll_to(3.0, 1.0);
        assert!(v.add_zoom(1.0));
        assert_eq!(v.zoom, 20);
        let c = v.centre_position();
        assert!((c.0 - 3.0).abs() < 1e-6 && (c.1 - 1.0).abs() < 1e-6, "{c:?}");
        for _ in 0..10 {
            v.add_zoom(1.0);
        }
        assert_eq!(v.zoom, ZOOM_MAX);
        assert!(!v.add_zoom(1.0));
        for _ in 0..10 {
            v.add_zoom(-1.0);
        }
        assert_eq!(v.zoom, ZOOM_MIN);
    }

    #[test]
    fn dragging_clamps_to_the_content() {
        let mut v = view();
        v.drag(1e7, 1e7);
        assert_eq!((v.scroll_x, v.scroll_y), (0.0, 0.0));
        v.drag(-1e7, -1e7);
        let (w, h) = v.content();
        assert_eq!(v.scroll_x, w - 440.0);
        assert_eq!(v.scroll_y, h - 268.0);
        // Dragging right moves the content right: the scroll falls.
        let before = v.scroll_x;
        v.drag(5.0, 0.0);
        assert_eq!(v.scroll_x, before - 5.0);
    }

    #[test]
    fn content_smaller_than_the_panel_stays_centred() {
        let mut v = View::new([0.0, 0.0, 1e6, 1e6], Bounds::of([node(0.0, 0.0)]));
        let before = (v.scroll_x, v.scroll_y);
        v.drag(100.0, 100.0);
        assert_eq!((v.scroll_x, v.scroll_y), before);
    }

    #[test]
    fn shows_answers_for_the_panel() {
        let v = view();
        assert!(v.shows([30.0, 10.0, 5.0, 5.0]));
        assert!(!v.shows([0.0, 10.0, 5.0, 5.0]));
        assert!(!v.shows([500.0, 10.0, 5.0, 5.0]));
    }

    #[test]
    fn a_line_quad_has_its_thickness() {
        let q = line_quad([0.0, 0.0], [10.0, 0.0], 2.0);
        assert_eq!(q[0], [0.0, 2.0]);
        assert_eq!(q[2], [10.0, -2.0]);
    }
}
