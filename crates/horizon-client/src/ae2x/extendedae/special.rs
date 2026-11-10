//! ExtendedAE's special buses, emitters and planes (slice 5) and its Oversize
//! and Vacuum Interfaces: their ids and the rules that need no world. Derived
//! from ExtendedAE 2.2.42 (`PartTagStorageBus`, `PartTagExportBus`,
//! `TagPriorityList`, `TagExpParserV2`, `PartModStorageBus`, `PartModExportBus`,
//! `ModPriorityList`, `PartPreciseStorageBus`, `PartPreciseExportBus`,
//! `PartThresholdExportBus`, `PartThresholdLevelEmitter`,
//! `PartActiveFormationPlane`, `PartSmartAnnihilationPlane`,
//! `OversizeConfigInv`, `TileVacuumInterface`, `EAEConfig`), commit `1f86f927`,
//! LGPL-3.0; see `ae2/addons/extendedae/`. The game's side is in
//! `game/ae2x/extendedae/special.rs` and `interfaces.rs`.

pub const TAG_STORAGE_BUS: &str = "extendedae:tag_storage_bus";
pub const TAG_EXPORT_BUS: &str = "extendedae:tag_export_bus";
pub const MOD_STORAGE_BUS: &str = "extendedae:mod_storage_bus";
pub const MOD_EXPORT_BUS: &str = "extendedae:mod_export_bus";
pub const PRECISE_STORAGE_BUS: &str = "extendedae:precise_storage_bus";
pub const PRECISE_EXPORT_BUS: &str = "extendedae:precise_export_bus";
pub const THRESHOLD_EXPORT_BUS: &str = "extendedae:threshold_export_bus";
pub const THRESHOLD_LEVEL_EMITTER: &str = "extendedae:threshold_level_emitter";
pub const ACTIVE_FORMATION_PLANE: &str = "extendedae:active_formation_plane";
pub const SMART_ANNIHILATION_PLANE: &str = "extendedae:smart_annihilation_plane";
pub const OVERSIZE_INTERFACE: &str = "extendedae:oversize_interface";
pub const OVERSIZE_INTERFACE_PART: &str = "extendedae:oversize_interface_part";
pub const VACUUM_INTERFACE: &str = "extendedae:vacuum_interface";

/// The part's saved fields, as the mod names them: the tag buses' white and
/// black expressions, the mod buses' list, the precise and threshold export
/// buses' amounts (`config2`), the threshold export bus's mode, the threshold
/// emitter's bounds and state.
pub const TAG_WHITE: &str = "oreExp";
pub const TAG_BLACK: &str = "oreExp2";
pub const MOD_IDS: &str = "modid";
pub const AMOUNTS: &str = "config2";
pub const THRESHOLD_MODE: &str = "cmod";
pub const UPPER: &str = "upperValue";
pub const LOWER: &str = "lowerValue";
pub const CURRENT_STATE: &str = "currentState";

/// The operations a tick of a bus with `speed_cards` (`IOBusPart.getOperationsPerTick`),
/// which is also the Active Formation Plane's drop multiplier (`getDropMultiplier`).
pub fn speed_operations(speed_cards: u8) -> u64 {
    match speed_cards {
        1 => 8,
        2 => 32,
        3 => 64,
        4 => 96,
        _ => 1,
    }
}

/// Config slots a capacity-card device shows: 18 and nine a card, of 63.
pub fn capacity_slots(capacity_cards: u8) -> usize {
    (18 + usize::from(capacity_cards) * 9).min(63)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Tag(String),
    Not,
    And,
    Or,
    Xor,
    Open,
    Close,
    End,
}
impl Token {
    /// `Op.priority`; NOT is the right-associative one.
    fn priority(&self) -> Option<u8> {
        Some(match self {
            Token::Not => 3,
            Token::And => 2,
            Token::Xor => 1,
            Token::Or => 0,
            _ => return None,
        })
    }
    fn kind(&self) -> u8 {
        match self {
            Token::Not => 0,
            Token::And | Token::Or | Token::Xor => 1,
            Token::Tag(_) => 2,
            Token::Open => 3,
            Token::Close => 4,
            Token::End => 6,
        }
    }
}
/// `TagExpParserV2.isValidToken`: what may follow a token (START is 5).
fn may_follow(current: u8, next: u8) -> bool {
    match current {
        0 | 1 | 3 => matches!(next, 0 | 3 | 2),
        2 | 4 => matches!(next, 1 | 4 | 6),
        5 => matches!(next, 0 | 2 | 3 | 6),
        _ => false,
    }
}
/// `ResourceLocation.validPathChar`, `:` and `*`.
fn tag_char(c: char) -> bool {
    matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.' | '/' | ':' | '*')
}

/// A compiled tag expression (`TagExpParserV2.compile`): tags joined by `&`,
/// `|`, `^`, `!` and parentheses, each tag a pattern where `*` is any run of
/// characters and `.` any one (the mod matches it as a regular expression).
/// One that does not parse matches nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagExpression {
    rpn: Vec<Token>,
}
impl TagExpression {
    pub fn compile(text: &str) -> Self {
        let text = text.trim().replace("&&", "&").replace("||", "|");
        Self { rpn: to_rpn(&tokenize(&text)).unwrap_or_default() }
    }
    /// The tag patterns it asks about.
    pub fn patterns(&self) -> impl Iterator<Item = &str> {
        self.rpn.iter().filter_map(|t| match t {
            Token::Tag(s) => Some(s.as_str()),
            _ => None,
        })
    }
    /// `eval`: 1 when it holds, -1 when it does not, 0 when it is broken.
    fn eval(&self, has: &dyn Fn(&str) -> bool) -> i8 {
        if self.rpn.is_empty() {
            return 0;
        }
        let mut stack: Vec<bool> = Vec::new();
        for token in &self.rpn {
            match token {
                Token::Tag(pattern) => stack.push(has(pattern)),
                Token::Not => match stack.pop() {
                    Some(v) => stack.push(!v),
                    None => return 0,
                },
                Token::And | Token::Or | Token::Xor => {
                    let (Some(a), Some(b)) = (stack.pop(), stack.pop()) else { return 0 };
                    stack.push(match token {
                        Token::And => a && b,
                        Token::Or => a || b,
                        _ => a ^ b,
                    });
                }
                _ => return 0,
            }
        }
        match stack.as_slice() {
            [v] => if *v { 1 } else { -1 },
            _ => 0,
        }
    }
    /// A whitelist's predicate: it holds.
    pub fn white(&self, has: &dyn Fn(&str) -> bool) -> bool {
        self.eval(has) == 1
    }
    /// A blacklist's predicate: it parses and does not hold.
    pub fn black(&self, has: &dyn Fn(&str) -> bool) -> bool {
        self.eval(has) == -1
    }
}
/// `tokenize`: a character that is neither a tag's, an operator nor blank
/// ends the tag before it and is dropped.
fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut tag = String::new();
    for c in text.chars().chain([' ']) {
        if tag_char(c) {
            tag.push(c);
            continue;
        }
        if !tag.is_empty() {
            tokens.push(Token::Tag(std::mem::take(&mut tag)));
        }
        let op = match c {
            '&' => Token::And,
            '|' => Token::Or,
            '^' => Token::Xor,
            '!' => Token::Not,
            '(' => Token::Open,
            ')' => Token::Close,
            _ => continue,
        };
        tokens.push(op);
    }
    tokens.push(Token::End);
    tokens
}
/// `convertToRPN`: the shunting yard, refusing a token that may not follow
/// the one before and unmatched parentheses.
fn to_rpn(tokens: &[Token]) -> Option<Vec<Token>> {
    let mut previous = 5u8;
    let mut rpn = Vec::new();
    let mut ops: Vec<Token> = Vec::new();
    for token in tokens {
        if !may_follow(previous, token.kind()) {
            return None;
        }
        previous = token.kind();
        match token {
            Token::Tag(_) => rpn.push(token.clone()),
            Token::Not | Token::And | Token::Or | Token::Xor => {
                let mine = token.priority()?;
                let right = *token == Token::Not;
                while let Some(top) = ops.last() {
                    match top.priority() {
                        Some(p) if !(p < mine || (right && p == mine)) => rpn.push(ops.pop()?),
                        _ => break,
                    }
                }
                ops.push(token.clone());
            }
            Token::Open => ops.push(Token::Open),
            Token::Close => loop {
                match ops.pop()? {
                    Token::Open => break,
                    op => rpn.push(op),
                }
            },
            Token::End => {
                while let Some(op) = ops.pop() {
                    if op == Token::Open {
                        return None;
                    }
                    rpn.push(op);
                }
            }
        }
    }
    Some(rpn)
}
/// `checkTag` for one name: `*` alone or the name itself, else the pattern
/// with `*` as `.*`, matched whole as a regular expression (`.` any one).
pub fn tag_pattern_matches(pattern: &str, name: &str) -> bool {
    fn go(p: &[u8], s: &[u8]) -> bool {
        match p.split_first() {
            None => s.is_empty(),
            Some((b'*', rest)) => (0..=s.len()).any(|i| go(rest, &s[i..])),
            Some((b'.', rest)) => !s.is_empty() && go(rest, &s[1..]),
            Some((c, rest)) => s.first() == Some(c) && go(rest, &s[1..]),
        }
    }
    pattern == "*" || pattern == name || go(pattern.as_bytes(), name.as_bytes())
}

/// `TagPriorityList`: the white and black expressions, blanks removed. With
/// both, a key must pass both; with neither, every key is listed.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TagFilter {
    white: Option<TagExpression>,
    black: Option<TagExpression>,
}
impl TagFilter {
    pub fn new(white: &str, black: &str) -> Self {
        let clean = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let (white, black) = (clean(white), clean(black));
        Self {
            white: (!white.is_empty()).then(|| TagExpression::compile(&white)),
            black: (!black.is_empty()).then(|| TagExpression::compile(&black)),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.white.is_none() && self.black.is_none()
    }
    /// Every pattern either expression asks about.
    pub fn patterns(&self) -> Vec<&str> {
        self.white.iter().chain(&self.black).flat_map(TagExpression::patterns).collect()
    }
    /// `isListed` of a key whose names (its tags, its block's, its id) match
    /// the pattern when `has` says so; `keyed` is false for a key that is no
    /// item or fluid, which no expression matches.
    pub fn listed(&self, keyed: bool, has: &dyn Fn(&str) -> bool) -> bool {
        if self.is_empty() {
            return true;
        }
        if !keyed {
            return false;
        }
        self.white.as_ref().is_none_or(|w| w.white(has)) && self.black.as_ref().is_none_or(|b| b.black(has))
    }
}

/// `ModPriorityList`: the comma-separated mod ids or names, as Java's
/// `String.split(",")` and `trim` leave them: an empty text is one empty
/// entry (so it lists nothing), trailing empty pieces go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModFilter {
    ids: Vec<String>,
}
impl ModFilter {
    pub fn new(text: &str) -> Self {
        let mut pieces: Vec<&str> = text.split(',').collect();
        if !text.is_empty() {
            while pieces.last() == Some(&"") {
                pieces.pop();
            }
        }
        Self { ids: pieces.into_iter().map(|p| p.trim().to_owned()).collect() }
    }
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
    /// `isListed`: the key's namespace or that mod's display name is listed.
    pub fn listed(&self, namespace: &str, name: Option<&str>) -> bool {
        self.ids.iter().any(|id| id == namespace || Some(id.as_str()) == name)
    }
}

/// `PartThresholdLevelEmitter`: on from the upper bound, off again under the
/// lower one; the state is saved (`currentState`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Threshold {
    pub upper: i64,
    pub lower: i64,
    pub state: bool,
}
impl Threshold {
    /// `checkSanity`.
    pub fn sane(&self) -> bool {
        self.lower <= self.upper
    }
    /// `isLevelEmitterOn` for the reported `value`: `low` is the emitter's
    /// `LOW_SIGNAL` setting, `active` its node. An inactive or insane one
    /// emits nothing and keeps its state.
    pub fn update(&mut self, value: i64, low: bool, active: bool) -> bool {
        if !active || !self.sane() {
            return false;
        }
        let current = low != self.state;
        let on = if current { value >= self.lower } else { value >= self.upper };
        self.state = low != on;
        self.state
    }
}

/// `ThresholdMode`, saved as its ordinal in `cmod`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThresholdMode {
    #[default]
    Greater,
    Lower,
}
impl ThresholdMode {
    pub fn from_ordinal(n: i8) -> Self {
        if n == 1 { Self::Lower } else { Self::Greater }
    }
    pub fn ordinal(self) -> i8 {
        self as i8
    }
    /// `checkAmount`: above the slot's amount, or at and under it.
    pub fn passes(self, stored: u64, threshold: u64) -> bool {
        match self {
            Self::Greater => stored > threshold,
            Self::Lower => stored <= threshold,
        }
    }
    /// `getMaxOutput`: what is above the amount, or no bound.
    pub fn max_output(self, stored: u64, threshold: u64) -> u64 {
        match self {
            Self::Greater => stored.saturating_sub(threshold),
            Self::Lower => u64::MAX,
        }
    }
}

/// `PartPreciseExportBus.doBusWork` for one slot of `amount`: none when the
/// operations left cannot carry one batch (the loop stops), else the
/// multiple of `amount` to send, from what the grid gives (`extractable`, a
/// simulated extraction up to the operations' worth or, on a redstone pulse,
/// one batch) and what the target holds (`holds`, a simulated push).
pub fn precise_batch(amount: u64, ops: u64, factor: u64, pulse: bool, extractable: &mut dyn FnMut(u64) -> u64, holds: &mut dyn FnMut(u64) -> u64) -> Option<u64> {
    if amount == 0 || ops < (amount / factor.max(1)).max(1) {
        return None;
    }
    let ceil = if pulse { extractable(amount) } else { extractable(ops.saturating_mul(factor)) / amount * amount };
    Some(holds(ceil) / amount * amount)
}

/// `PreciseInventory.insert`: how much of `amount` may go into a target that
/// already shows `present` of the key, `configured` its slot's amount (none
/// when not listed) on a bus whose list is `empty`.
pub fn precise_insert(configured: Option<u64>, empty: bool, present: u64, amount: u64) -> u64 {
    let wanted = if empty { 1 } else { configured.unwrap_or(0) };
    wanted.saturating_sub(present).min(amount)
}

/// `OversizeConfigInv.useRegisteredCapacities`: a key type's slot capacity
/// times its multiplier, `Long.MAX_VALUE` past it.
pub fn oversize_capacity(base: u64, multiplier: u64) -> u64 {
    base.checked_mul(multiplier).map_or(i64::MAX as u64, |n| n.min(i64::MAX as u64))
}
/// `OversizeConfigInv.getMaxAmount` of an item: its stack size times the
/// items' multiplier, at most the items' capacity (AE2's 99 times it).
pub fn oversize_item_max(stack_max: u32, multiplier: u64) -> u64 {
    (u64::from(stack_max) * multiplier).min(oversize_capacity(99, multiplier))
}

/// `TileVacuumInterface.MAX_SIZE` and `MAX_OFFSET`.
pub const VACUUM_MAX_SIZE: i32 = 10;
pub const VACUUM_MAX_OFFSET: i32 = 16;
/// The Vacuum Interface's saved work area, `size` (default 5, 5, 5) and `offset`.
pub const VACUUM_SIZE: &str = "size";
pub const VACUUM_OFFSET: &str = "offset";
/// `setSize` and `setOffset`: each axis clamped.
pub fn vacuum_clamp(size: [i32; 3], offset: [i32; 3]) -> ([i32; 3], [i32; 3]) {
    (size.map(|n| n.clamp(1, VACUUM_MAX_SIZE)), offset.map(|n| n.clamp(-VACUUM_MAX_OFFSET, VACUUM_MAX_OFFSET)))
}
/// `createWorkArea`: from the block's corner plus the offset, half the size
/// back (rounded down) and the rest forward.
pub fn vacuum_area(pos: [i32; 3], size: [i32; 3], offset: [i32; 3]) -> ([f64; 3], [f64; 3]) {
    let mut min = [0.0; 3];
    let mut max = [0.0; 3];
    for i in 0..3 {
        let center = f64::from(pos[i] + offset[i]);
        let half = size[i] / 2;
        min[i] = center - f64::from(half);
        max[i] = center + f64::from(size[i] - half);
    }
    (min, max)
}
/// `BlockPos.asLong` and `BlockPos.of`, the form `size` and `offset` are saved in.
pub fn block_pos_long(p: [i32; 3]) -> i64 {
    ((i64::from(p[0]) & 0x3FF_FFFF) << 38) | ((i64::from(p[2]) & 0x3FF_FFFF) << 12) | (i64::from(p[1]) & 0xFFF)
}
pub fn block_pos_of(n: i64) -> [i32; 3] {
    [(n >> 38) as i32, ((n << 52) >> 52) as i32, ((n << 26) >> 38) as i32]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names<'a>(list: &'a [&'a str]) -> impl Fn(&str) -> bool + 'a {
        move |pattern| list.iter().any(|name| tag_pattern_matches(pattern, name))
    }

    #[test]
    fn tag_expressions_follow_the_parser() {
        let ore = ["c:ores", "c:ores/iron", "minecraft:iron_ore"];
        let has = names(&ore);
        assert!(TagExpression::compile("c:ores").white(&has));
        assert!(TagExpression::compile("c:ores/*").white(&has));
        assert!(TagExpression::compile("c:ores/* & !c:ores/gold").white(&has));
        assert!(!TagExpression::compile("c:ingots | c:gems").white(&has));
        assert!(TagExpression::compile("(c:ingots | c:ores) && minecraft:*").white(&has));
        // NOT binds tightest and to the right; AND before XOR before OR.
        assert!(TagExpression::compile("!!c:ores").white(&has));
        assert!(TagExpression::compile("c:gems | c:ores & c:ores/iron").white(&has));
        assert!(!TagExpression::compile("c:ores ^ c:ores/iron").white(&has));
        // `.` is any character, as in the mod's regular expression.
        assert!(TagExpression::compile("c:ores.iron").white(&has));
        // Broken: neither list holds it.
        for broken in ["c:ores &", "(c:ores", "c:ores)", "& c:ores", "c:ores c:gems", ""] {
            let e = TagExpression::compile(broken);
            assert!(!e.white(&has) && !e.black(&has), "{broken}");
        }
        // A blacklist holds where its expression parses and fails.
        assert!(TagExpression::compile("c:gems").black(&has));
        assert!(!TagExpression::compile("c:ores").black(&has));
        // Characters no tag has split and drop: `C` here.
        assert!(TagExpression::compile("Cc:ores").white(&has));
    }

    #[test]
    fn tag_filters_combine_white_and_black() {
        let iron = ["c:ores", "c:ores/iron"];
        let gold = ["c:ores", "c:ores/gold"];
        let f = TagFilter::new("c:ores", "c:ores/gold");
        assert!(f.listed(true, &names(&iron)));
        assert!(!f.listed(true, &names(&gold)));
        assert!(!f.listed(false, &names(&iron)), "no item or fluid");
        assert!(TagFilter::new(" ", "").is_empty());
        assert!(TagFilter::new("", "").listed(false, &names(&[])));
        assert!(TagFilter::new("", "c:ores/gold").listed(true, &names(&iron)));
        assert_eq!(TagFilter::new("c : ores", "").patterns(), ["c:ores"]);
    }

    #[test]
    fn mod_filters_split_as_java_does() {
        assert!(!ModFilter::new("").is_empty(), "one empty entry");
        assert!(!ModFilter::new("").listed("minecraft", None));
        assert!(ModFilter::new(",").is_empty());
        let f = ModFilter::new(" minecraft , ae2,");
        assert!(f.listed("minecraft", None) && f.listed("ae2", None) && !f.listed("mekanism", None));
        assert!(ModFilter::new("Mekanism").listed("mekanism", Some("Mekanism")));
    }

    #[test]
    fn threshold_emitter_keeps_its_band() {
        let mut t = Threshold { upper: 100, lower: 20, state: false };
        assert!(!t.update(50, false, true));
        assert!(t.update(100, false, true));
        assert!(t.update(20, false, true), "on until under the lower bound");
        assert!(!t.update(19, false, true));
        assert!(!t.update(99, false, true));
        // LOW_SIGNAL: the band's state inverted, from the saved output (`checkState(value, invert)`).
        let mut t = Threshold { upper: 100, lower: 20, state: false };
        assert!(!t.update(50, true, true), "off: the band counts as on");
        assert!(t.update(10, true, true), "on under the lower bound");
        assert!(t.update(50, true, true), "still on under the upper");
        assert!(!t.update(100, true, true));
        assert!(!t.update(30, true, true));
        // Off when insane or inactive, the state kept.
        let mut t = Threshold { upper: 10, lower: 20, state: true };
        assert!(!t.update(50, false, true));
        assert!(t.state);
        let mut t = Threshold { upper: 10, lower: 0, state: true };
        assert!(!t.update(50, false, false));
        assert!(t.state);
    }

    #[test]
    fn threshold_export_modes() {
        assert!(ThresholdMode::Greater.passes(11, 10) && !ThresholdMode::Greater.passes(10, 10));
        assert!(ThresholdMode::Lower.passes(10, 10) && !ThresholdMode::Lower.passes(11, 10));
        assert_eq!(ThresholdMode::Greater.max_output(100, 64), 36);
        assert_eq!(ThresholdMode::from_ordinal(1), ThresholdMode::Lower);
        assert_eq!(ThresholdMode::from_ordinal(0).ordinal(), 0);
    }

    #[test]
    fn precise_batches_are_whole() {
        // 1 operation x 8 (the bus speed) and a batch of 3: 6 of 8.
        let got = precise_batch(3, 8, 1, false, &mut |n| n.min(100), &mut |n| n);
        assert_eq!(got, Some(6));
        // The target holds 4: one batch.
        assert_eq!(precise_batch(3, 8, 1, false, &mut |n| n, &mut |n| n.min(4)), Some(3));
        // The grid has 2: nothing.
        assert_eq!(precise_batch(3, 8, 1, false, &mut |n| n.min(2), &mut |n| n), Some(0));
        // Too few operations for one batch: the loop stops.
        assert_eq!(precise_batch(16, 8, 1, false, &mut |n| n, &mut |n| n), None);
        // A pulse sends one batch.
        assert_eq!(precise_batch(3, 96, 1, true, &mut |n| n, &mut |n| n), Some(3));
        // A bucket a batch, 1,000 a fluid's operation.
        assert_eq!(precise_batch(1000, 1, 1000, false, &mut |n| n, &mut |n| n), Some(1000));
        assert_eq!(precise_insert(Some(10), false, 4, 64), 6);
        assert_eq!(precise_insert(None, false, 0, 64), 0);
        assert_eq!(precise_insert(None, true, 0, 64), 1);
        assert_eq!(precise_insert(Some(10), false, 12, 64), 0);
    }

    #[test]
    fn oversize_and_vacuum_sizes() {
        assert_eq!(oversize_item_max(64, 16), 1024);
        assert_eq!(oversize_item_max(16, 16), 256);
        assert_eq!(oversize_item_max(99, 16), 1584);
        assert_eq!(oversize_capacity(4000, 16), 64000);
        assert_eq!(oversize_capacity(u64::MAX / 2, 4), i64::MAX as u64);
        assert_eq!(vacuum_area([10, 64, -3], [5, 5, 5], [0, 0, 0]), ([8.0, 62.0, -5.0], [13.0, 67.0, 0.0]));
        assert_eq!(vacuum_area([0, 0, 0], [4, 1, 3], [0, 2, 0]), ([-2.0, 2.0, -1.0], [2.0, 3.0, 2.0]));
        assert_eq!(vacuum_clamp([0, 11, 5], [-17, 3, 16]), ([1, 10, 5], [-16, 3, 16]));
        for p in [[5, 5, 5], [-3, 2, -16], [0, -1, 1]] {
            assert_eq!(block_pos_of(block_pos_long(p)), p);
        }
        assert_eq!(speed_operations(3), 64);
        assert_eq!(capacity_slots(5), 63);
    }
}
