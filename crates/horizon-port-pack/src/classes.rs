//! What a mod's Java code does, read from its class files without running
//! or decompiling them: each class's superclass, interfaces, declared
//! methods and the classes and annotations its constant pool names. The
//! mod's own classes are followed up to the first game class they extend,
//! and that class says what the code is: a block entity, a menu, an entity,
//! a block or item with behaviour of its own, a worldgen feature...
//!
//! Since 26.1 the game ships unobfuscated, and NeoForge has run on Mojang's
//! names since 1.20.5, so game classes and methods are read by name. Fabric
//! jars before 26.1 name them in intermediary (`net/minecraft/class_2248`)
//! and Forge's methods are SRG (`m_60503_`): a class extending such a game
//! class is counted as code of its own, whatever it overrides.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// One class file, as much as this module reads.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Class {
    pub name: String,
    pub superclass: String,
    pub interfaces: Vec<String>,
    /// Declared methods, by name (constructors and lambdas left out).
    pub methods: Vec<String>,
    /// Every class the constant pool names (`net/minecraft/...`).
    pub refs: BTreeSet<String>,
    /// Annotation and descriptor strings (`Lorg/spongepowered/asm/mixin/Mixin;`).
    pub mixin: bool,
    /// The NeoForge events it listens to: the parameter of an `@SubscribeEvent`
    /// method, or of a method reference or lambda that takes one event
    /// (`NeoForge.EVENT_BUS.addListener(this::onTick)`), as binary names
    /// (`net/neoforged/neoforge/event/tick/ServerTickEvent$Post`).
    pub listens: BTreeSet<String>,
    /// The NeoForge capabilities whose fields it reads
    /// (`Capabilities.ItemHandler.BLOCK`).
    pub capabilities: BTreeSet<String>,
}

/// The annotation on a NeoForge event listener method.
const SUBSCRIBE_EVENT: &str = "Lnet/neoforged/bus/api/SubscribeEvent;";

/// The event a `(Lnet/neoforged/…Event;)V` descriptor takes as its only
/// parameter: a class under `net/neoforged/` whose outermost name ends in
/// `Event`.
fn one_event(descriptor: &str) -> Option<&str> {
    let class = descriptor.strip_prefix("(L")?.strip_suffix(";)V")?;
    let outer = class.rsplit('/').next()?.split('$').next()?;
    (class.starts_with("net/neoforged/") && !class.contains(';') && outer.ends_with("Event"))
        .then_some(class)
}

/// Whether a `RuntimeVisibleAnnotations` attribute names `annotation`
/// (JVMS 4.7.16).
fn annotated(body: &[u8], utf8: &HashMap<usize, String>, annotation: &str) -> Option<bool> {
    let mut r = Reader { bytes: body, at: 0 };
    let mut found = false;
    for _ in 0..r.u2()? {
        found |= utf8
            .get(&(r.u2()? as usize))
            .is_some_and(|t| t == annotation);
        skip_pairs(&mut r, 0)?;
    }
    Some(found)
}

/// Skips an annotation's element-value pairs.
fn skip_pairs(r: &mut Reader<'_>, depth: u8) -> Option<()> {
    for _ in 0..r.u2()? {
        r.skip(2)?;
        skip_value(r, depth)?;
    }
    Some(())
}

/// Skips one element value; nesting deeper than 32 is refused.
fn skip_value(r: &mut Reader<'_>, depth: u8) -> Option<()> {
    if depth > 32 {
        return None;
    }
    match r.u1()? {
        b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' | b'c' => r.skip(2),
        b'e' => r.skip(4),
        b'@' => {
            r.skip(2)?;
            skip_pairs(r, depth + 1)
        }
        b'[' => {
            for _ in 0..r.u2()? {
                skip_value(r, depth + 1)?;
            }
            Some(())
        }
        _ => None,
    }
}

/// Reads a class file's header, constant pool and declared methods.
pub fn parse(bytes: &[u8]) -> Option<Class> {
    let mut r = Reader { bytes, at: 0 };
    if r.u4()? != 0xCAFE_BABE {
        return None;
    }
    r.skip(4)?; // minor, major
    let count = r.u2()? as usize;
    let mut utf8: HashMap<usize, String> = HashMap::new();
    let mut class_of: HashMap<usize, usize> = HashMap::new();
    // Field references (class, name and type), the names of names and
    // types, and method types' descriptors: for `listens` and `capabilities`.
    let mut fields: Vec<(usize, usize)> = Vec::new();
    let mut name_of: HashMap<usize, usize> = HashMap::new();
    let mut method_types: Vec<usize> = Vec::new();
    let mut i = 1;
    while i < count {
        let tag = r.u1()?;
        match tag {
            1 => {
                let len = r.u2()? as usize;
                let s = r.take(len)?;
                utf8.insert(i, String::from_utf8_lossy(s).into_owned());
            }
            7 => {
                class_of.insert(i, r.u2()? as usize);
            }
            9 => fields.push((r.u2()? as usize, r.u2()? as usize)),
            12 => {
                name_of.insert(i, r.u2()? as usize);
                r.skip(2)?;
            }
            16 => method_types.push(r.u2()? as usize),
            8 | 19 | 20 => r.skip(2)?,
            3 | 4 | 10 | 11 | 17 | 18 => r.skip(4)?,
            15 => r.skip(3)?,
            5 | 6 => {
                r.skip(8)?;
                i += 1;
            }
            _ => return None,
        }
        i += 1;
    }
    let class_name = |idx: usize| -> String {
        class_of
            .get(&idx)
            .and_then(|u| utf8.get(u))
            .cloned()
            .unwrap_or_default()
    };
    r.skip(2)?; // access flags
    let name = class_name(r.u2()? as usize);
    let superclass = class_name(r.u2()? as usize);
    let n = r.u2()? as usize;
    let mut interfaces = Vec::with_capacity(n);
    for _ in 0..n {
        interfaces.push(class_name(r.u2()? as usize));
    }
    // Fields, then methods: flags, name, descriptor, attributes.
    let mut methods = Vec::new();
    let mut listens = BTreeSet::new();
    for member in 0..2 {
        let n = r.u2()?;
        for _ in 0..n {
            r.skip(2)?;
            let name = r.u2()? as usize;
            let descriptor = r.u2()? as usize;
            let attributes = r.u2()?;
            let mut subscribes = false;
            for _ in 0..attributes {
                let attribute = r.u2()? as usize;
                let len = r.u4()? as usize;
                let body = r.take(len)?;
                if member == 1
                    && utf8
                        .get(&attribute)
                        .is_some_and(|a| a == "RuntimeVisibleAnnotations")
                {
                    subscribes |= annotated(body, &utf8, SUBSCRIBE_EVENT).unwrap_or(false);
                }
            }
            if subscribes && let Some(event) = utf8.get(&descriptor).and_then(|d| one_event(d)) {
                listens.insert(event.to_owned());
            }
            if member == 1
                && let Some(m) = utf8.get(&name)
                && !m.starts_with('<')
                && !m.starts_with("lambda$")
            {
                methods.push(m.clone());
            }
        }
    }
    let refs = class_of
        .values()
        .filter_map(|u| utf8.get(u))
        .filter(|s| !s.starts_with('['))
        .cloned()
        .collect();
    let mixin = utf8
        .values()
        .any(|s| s == "Lorg/spongepowered/asm/mixin/Mixin;");
    listens.extend(
        method_types
            .iter()
            .filter_map(|d| utf8.get(d).and_then(|d| one_event(d)))
            .map(str::to_owned),
    );
    let capabilities = fields
        .iter()
        .filter_map(|&(class, name_and_type)| {
            let owner = class_name(class);
            let kind = owner.strip_prefix("net/neoforged/neoforge/capabilities/Capabilities$")?;
            let field = utf8.get(name_of.get(&name_and_type)?)?;
            Some(format!("Capabilities.{}.{field}", kind.replace('$', ".")))
        })
        .collect();
    Some(Class {
        name,
        superclass,
        interfaces,
        methods,
        refs,
        mixin,
        listens,
        capabilities,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.bytes.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(drop)
    }
    fn u1(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u2(&mut self) -> Option<u16> {
        let b = self.take(2)?;
        Some(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u4(&mut self) -> Option<u32> {
        let b = self.take(4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// What a class does, as the port sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    /// A block with behaviour of its own (use, ticks, collisions...).
    BlockBehaviour,
    /// A block with a shape of its own: it ports with its template's.
    BlockShape,
    /// An item with behaviour of its own (use, on hit...).
    ItemBehaviour,
    BlockEntity,
    Menu,
    Entity,
    Screen,
    Renderer,
    /// A worldgen feature, placement or structure type written in Java.
    Worldgen,
    /// A recipe type (its recipes don't craft) or ingredient type.
    Recipe,
    Effect,
    Enchantment,
    Particle,
    Command,
    Network,
    /// Reacts to game events (NeoForge's bus, Fabric's callbacks).
    Events,
    /// A Mixin: a patch to the game's own code.
    Mixin,
    /// Extends a game class under obfuscated names (Fabric before 26.1,
    /// Forge's methods): what it does can't be read.
    Unread,
}

impl Feature {
    /// How the coverage line names `n` of them.
    pub fn describe(self, n: usize) -> String {
        let (one, many) = match self {
            Feature::BlockBehaviour => (
                "block with behaviour of its own (use, ticks, collisions)",
                "blocks with behaviour of their own (use, ticks, collisions)",
            ),
            Feature::BlockShape => (
                "block with a shape of its own: it ports with its template's",
                "blocks with shapes of their own: they port with their templates'",
            ),
            Feature::ItemBehaviour => (
                "item with behaviour of its own (use, on hit)",
                "items with behaviour of their own (use, on hit)",
            ),
            Feature::BlockEntity => (
                "block entity (machine, container or animated block)",
                "block entities (machines, containers, animated blocks)",
            ),
            Feature::Menu => ("menu (a container screen)", "menus (container screens)"),
            Feature::Entity => ("mob or entity", "mobs or entities"),
            Feature::Screen => ("screen of its own", "screens of its own"),
            Feature::Renderer => ("renderer or model in code", "renderers or models in code"),
            Feature::Worldgen => (
                "worldgen feature or structure type in code",
                "worldgen features or structure types in code",
            ),
            Feature::Recipe => (
                "recipe type of its own (its recipes don't craft)",
                "recipe types of its own (their recipes don't craft)",
            ),
            Feature::Effect => ("status effect", "status effects"),
            Feature::Enchantment => ("enchantment effect in code", "enchantment effects in code"),
            Feature::Particle => ("particle type", "particle types"),
            Feature::Command => ("command", "commands"),
            Feature::Network => ("network message", "network messages"),
            Feature::Events => ("game event it reacts to", "game events it reacts to"),
            Feature::Mixin => (
                "Mixin (a patch to the game's code)",
                "Mixins (patches to the game's code)",
            ),
            Feature::Unread => (
                "class extending the game under obfuscated names",
                "classes extending the game under obfuscated names",
            ),
        };
        format!("{n} {}", if n == 1 { one } else { many })
    }
}

/// Block methods that only say how it looks or is placed: the template's
/// do the same for the states a blockstate file lists.
const BLOCK_LOOKS: [&str; 18] = [
    "codec",
    "createBlockStateDefinition",
    "getStateForPlacement",
    "rotate",
    "mirror",
    "getRenderShape",
    "skipRendering",
    "getShadeBrightness",
    "propagatesSkylightDown",
    "getCloneItemStack",
    "isPathfindable",
    "getFluidState",
    "updateShape",
    "canBeReplaced",
    "getLightBlock",
    "useShapeForLightOcclusion",
    "getDescriptionId",
    "animateTick",
];
/// Block methods that only change its outline or collision.
const BLOCK_SHAPES: [&str; 6] = [
    "getShape",
    "getCollisionShape",
    "getVisualShape",
    "getOcclusionShape",
    "getInteractionShape",
    "getBlockSupportShape",
];
/// Item methods that only say how it looks or is named.
const ITEM_LOOKS: [&str; 6] = [
    "appendHoverText",
    "getName",
    "isFoil",
    "getDescriptionId",
    "getBarColor",
    "getBarWidth",
];

/// The game's own, loader and library classes: the first of these a mod's
/// class chain reaches is what it extends.
fn external(name: &str, own: &BTreeMap<String, Class>) -> bool {
    !own.contains_key(name)
}

fn obfuscated_method(m: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    m.strip_prefix("method_").is_some_and(digits)
        || m.strip_prefix("m_")
            .and_then(|s| s.strip_suffix('_'))
            .is_some_and(digits)
}

/// What a game class (Mojang's names) is, by its package and name.
fn kind_of_game_class(name: &str) -> Option<Feature> {
    let rest = name.strip_prefix("net/minecraft/")?;
    if rest.starts_with("class_") {
        return Some(Feature::Unread);
    }
    let (package, simple) = rest.rsplit_once('/').unwrap_or(("", rest));
    Some(match package {
        "world/level/block/entity" => Feature::BlockEntity,
        p if p.starts_with("world/level/block") => Feature::BlockBehaviour,
        "world/inventory" if simple.ends_with("Menu") => Feature::Menu,
        p if p.starts_with("world/entity") => Feature::Entity,
        p if p.starts_with("world/item/crafting") => Feature::Recipe,
        p if p.starts_with("world/item/enchantment") => Feature::Enchantment,
        p if p.starts_with("world/item") => Feature::ItemBehaviour,
        p if p.starts_with("world/level/levelgen") => Feature::Worldgen,
        "world/effect" => Feature::Effect,
        p if p.starts_with("client/gui/screens") || p.starts_with("client/gui/components") => {
            Feature::Screen
        }
        p if p.starts_with("client/renderer") || p.starts_with("client/model") => Feature::Renderer,
        p if p.starts_with("client/particle") || p.starts_with("core/particles") => {
            Feature::Particle
        }
        _ => return None,
    })
}

/// Events a content mod uses to register what it has: not behaviour.
fn registration_event(name: &str) -> bool {
    [
        "BuildCreativeModeTabContentsEvent",
        "RegisterEvent",
        "Register",
        "Lifecycle",
        "ModConfigEvent",
        "AddPackFindersEvent",
        "ItemGroupEvents",
        "ClientLifecycleEvents",
        "ServerLifecycleEvents",
    ]
    .iter()
    .any(|k| name.rsplit('/').next().unwrap_or(name).contains(k))
        || name.contains("/lifecycle/")
        || name.contains("/registries/")
        || name.contains("/registry/")
}

/// The code census: how many of each feature, and examples.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Code {
    pub classes: usize,
    pub features: BTreeMap<Feature, usize>,
    /// The game events it subscribes to, by simple name.
    pub events: BTreeSet<String>,
    /// Blocks and items with behaviour, and the methods they override.
    pub examples: BTreeMap<String, Vec<String>>,
}

impl Code {
    /// Whether it does anything beyond registering blocks and items.
    pub fn behaviour(&self) -> bool {
        self.features
            .keys()
            .any(|f| *f != Feature::BlockShape && *f != Feature::Particle)
    }
}

/// Reads every class of a jar. `classes` are the class files' bytes.
pub fn census<'a>(classes: impl IntoIterator<Item = &'a [u8]>) -> Code {
    let own: BTreeMap<String, Class> = classes
        .into_iter()
        .filter_map(parse)
        .map(|c| (c.name.clone(), c))
        .collect();
    let mut code = Code {
        classes: own.len(),
        ..Code::default()
    };
    let mut count = |f: Feature| *code.features.entry(f).or_default() += 1;
    for c in own.values() {
        if c.mixin {
            count(Feature::Mixin);
            continue;
        }
        // The first class outside the jar the chain reaches, and the
        // methods declared along the way.
        let mut methods: BTreeSet<&str> = c.methods.iter().map(String::as_str).collect();
        let mut at = c;
        let mut seen = 0;
        while !external(&at.superclass, &own) && seen < 64 {
            at = &own[&at.superclass];
            methods.extend(at.methods.iter().map(String::as_str));
            seen += 1;
        }
        let base = at.superclass.as_str();
        let kind = kind_of_game_class(base).or_else(|| {
            c.interfaces.iter().find_map(|i| match i.as_str() {
                "net/minecraft/network/protocol/common/custom/CustomPacketPayload" => {
                    Some(Feature::Network)
                }
                "net/minecraft/world/MenuProvider" => Some(Feature::Menu),
                _ => None,
            })
        });
        let Some(kind) = kind else {
            continue;
        };
        let feature = match kind {
            Feature::BlockBehaviour => {
                let behaviour: Vec<&str> = methods
                    .iter()
                    .copied()
                    .filter(|m| !BLOCK_LOOKS.contains(m) && !BLOCK_SHAPES.contains(m))
                    .collect();
                if !behaviour.is_empty() {
                    code.examples.insert(
                        c.name.rsplit('/').next().unwrap_or("").to_owned(),
                        behaviour.iter().map(|m| m.to_string()).collect(),
                    );
                    if behaviour.iter().any(|m| obfuscated_method(m)) {
                        Some(Feature::Unread)
                    } else {
                        Some(Feature::BlockBehaviour)
                    }
                } else if methods.iter().any(|m| BLOCK_SHAPES.contains(m)) {
                    Some(Feature::BlockShape)
                } else {
                    None
                }
            }
            Feature::ItemBehaviour => {
                let behaviour: Vec<&str> = methods
                    .iter()
                    .copied()
                    .filter(|m| !ITEM_LOOKS.contains(m))
                    .collect();
                if behaviour.is_empty() {
                    None
                } else {
                    code.examples.insert(
                        c.name.rsplit('/').next().unwrap_or("").to_owned(),
                        behaviour.iter().map(|m| m.to_string()).collect(),
                    );
                    Some(Feature::ItemBehaviour)
                }
            }
            // A class extending an obfuscated game class without declaring
            // anything adds nothing to read.
            Feature::Unread if methods.is_empty() => None,
            k => Some(k),
        };
        if let Some(f) = feature {
            count(f);
        }
    }
    for c in own.values() {
        for r in &c.refs {
            let event = (r.starts_with("net/neoforged/neoforge/event/")
                || r.starts_with("net/minecraftforge/event/")
                || r.starts_with("net/fabricmc/fabric/api/event/")
                || r.starts_with("net/fabricmc/fabric/api/entity/event/"))
                && !registration_event(r);
            if event {
                let simple = r.rsplit('/').next().unwrap_or(r);
                code.events
                    .insert(simple.split('$').next().unwrap_or(simple).to_owned());
            }
            if r.starts_with("com/mojang/brigadier/")
                && !code.features.contains_key(&Feature::Command)
            {
                code.features.insert(Feature::Command, 1);
            }
        }
    }
    if !code.events.is_empty() {
        code.features.insert(Feature::Events, code.events.len());
    }
    code
}

/// The game's own classes a block's click is read from.
const FURNACE_BLOCK: &str = "net/minecraft/world/level/block/AbstractFurnaceBlock";
const FURNACE_ENTITY: &str = "net/minecraft/world/level/block/entity/AbstractFurnaceBlockEntity";
/// What a furnace's block entity holds to cook: its recipe and the items counted for it.
const COOKING_HOLDER: &str = "net/minecraft/world/inventory/RecipeCraftingHolder";
const MENU_PROVIDER: &str = "net/minecraft/world/MenuProvider";
/// Implements `MenuProvider`: the game's chests, furnaces and hoppers are its subclasses.
const MENU_CONTAINER: &str = "net/minecraft/world/level/block/entity/BaseContainerBlockEntity";
const BLOCK_ENTITY_PACKAGE: &str = "net/minecraft/world/level/block/entity/";

/// What the classes behind a block say of how a click on it goes in Java.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Behaviour {
    /// Cooks like a furnace: the block extends `AbstractFurnaceBlock`, or its block entity extends
    /// `AbstractFurnaceBlockEntity` or implements `RecipeCraftingHolder`. A furnace's own template brings
    /// the menu, `facing` and `lit` such a block has.
    pub furnace: bool,
    /// Opens a menu: the block or its block entity implements `MenuProvider`.
    pub menu: bool,
}

/// What one class is, from what is above it: its superclasses and interfaces.
#[derive(Debug, Default, Clone, Copy)]
struct Kind {
    block_entity: bool,
    furnace: bool,
    menu: bool,
}

/// A mod jar's classes by binary name (`a/b/C`): their superclasses, interfaces and the classes their constant
/// pools name, never what a method does. A block names its block entity (`newBlockEntity`, the casts that read
/// it), and the block entity's class says whether it is a `MenuProvider` or cooks.
#[derive(Debug, Default)]
pub struct Hierarchy {
    classes: HashMap<String, Class>,
    kinds: HashMap<String, Kind>,
}

impl Hierarchy {
    /// The jar's classes, as [`parse`] read them.
    pub fn from_classes(files: impl IntoIterator<Item = Class>) -> Self {
        let classes: HashMap<String, Class> =
            files.into_iter().map(|c| (c.name.clone(), c)).collect();
        let mut h = Self {
            classes,
            kinds: HashMap::new(),
        };
        let kinds = h.classes.keys().map(|n| (n.clone(), h.kind_of(n))).collect();
        h.kinds = kinds;
        h
    }

    /// The class files of a jar, each under the per-class cap and all under the archive's ceiling.
    pub fn read<R: std::io::Read + std::io::Seek>(
        zip: &mut zip::ZipArchive<R>,
        names: &[String],
    ) -> Self {
        let mut budget = crate::entry::Budget::archive();
        let mut files = Vec::new();
        for n in names.iter().filter(|n| n.ends_with(".class")) {
            let Ok(e) = zip.by_name(n) else { continue };
            let declared = e.size();
            match budget.read(e, declared, (1 << 24) - 1) {
                Ok(bytes) => files.extend(parse(&bytes)),
                Err(e) if crate::entry::is_too_big(&e) && declared < 1 << 24 => break,
                Err(_) => {}
            }
        }
        Self::from_classes(files)
    }

    /// `name` and every class and interface above it: the mod's own followed through the jar, the game's named.
    fn above(&self, name: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut todo = vec![name.to_owned()];
        while let Some(n) = todo.pop() {
            if n.is_empty() || !seen.insert(n.clone()) {
                continue;
            }
            if let Some(c) = self.classes.get(&n) {
                todo.push(c.superclass.clone());
                todo.extend(c.interfaces.iter().cloned());
            }
        }
        seen
    }

    fn kind_of(&self, name: &str) -> Kind {
        let above = self.above(name);
        Kind {
            block_entity: above
                .iter()
                .any(|a| a.starts_with(BLOCK_ENTITY_PACKAGE) && a.ends_with("BlockEntity")),
            furnace: above.contains(FURNACE_ENTITY) || above.contains(COOKING_HOLDER),
            menu: above.contains(MENU_PROVIDER) || above.contains(MENU_CONTAINER),
        }
    }

    /// How a click on the block of class `block` (`a.b.C` or `a/b/C`) goes: from the block's own classes, and
    /// from the mod's block entity classes they name.
    pub fn behaviour(&self, block: &str) -> Behaviour {
        let own = self.above(&block.replace('.', "/"));
        let mut out = Behaviour {
            furnace: own.contains(FURNACE_BLOCK),
            menu: own.contains(MENU_PROVIDER),
        };
        for class in own.iter().filter_map(|n| self.classes.get(n)) {
            for kind in class
                .refs
                .iter()
                .filter_map(|r| self.kinds.get(r))
                .filter(|k| k.block_entity)
            {
                out.furnace |= kind.furnace;
                out.menu |= kind.menu;
            }
        }
        out
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    /// A class file with `name` extending `superclass`, declaring `methods`
    /// and naming `refs` in its constant pool.
    pub fn class_with(
        name: &str,
        superclass: &str,
        methods: &[&str],
        refs: &[&str],
        mixin: bool,
    ) -> Vec<u8> {
        class_implementing(name, superclass, &[], methods, refs, mixin)
    }

    /// [`class_with`] implementing `interfaces`.
    pub fn class_implementing(
        name: &str,
        superclass: &str,
        interfaces: &[&str],
        methods: &[&str],
        refs: &[&str],
        mixin: bool,
    ) -> Vec<u8> {
        let mut pool: Vec<Vec<u8>> = Vec::new();
        let utf = |pool: &mut Vec<Vec<u8>>, s: &str| -> u16 {
            let mut e = vec![1];
            e.extend((s.len() as u16).to_be_bytes());
            e.extend(s.as_bytes());
            pool.push(e);
            pool.len() as u16
        };
        let class = |pool: &mut Vec<Vec<u8>>, s: &str| -> u16 {
            let u = utf(pool, s);
            let mut e = vec![7];
            e.extend(u.to_be_bytes());
            pool.push(e);
            pool.len() as u16
        };
        let this = class(&mut pool, name);
        let sup = class(&mut pool, superclass);
        let implemented: Vec<u16> = interfaces.iter().map(|i| class(&mut pool, i)).collect();
        for r in refs {
            class(&mut pool, r);
        }
        // A long takes two slots.
        pool.push(vec![5, 0, 0, 0, 0, 0, 0, 0, 1]);
        pool.push(Vec::new());
        if mixin {
            utf(&mut pool, "Lorg/spongepowered/asm/mixin/Mixin;");
        }
        let desc = utf(&mut pool, "()V");
        let names: Vec<u16> = methods.iter().map(|m| utf(&mut pool, m)).collect();
        let mut out = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65];
        out.extend(((pool.len() + 1) as u16).to_be_bytes());
        for e in &pool {
            out.extend(e);
        }
        out.extend([0, 0x21]);
        out.extend(this.to_be_bytes());
        out.extend(sup.to_be_bytes());
        out.extend((implemented.len() as u16).to_be_bytes());
        for i in implemented {
            out.extend(i.to_be_bytes());
        }
        out.extend([0, 0]); // fields
        out.extend((names.len() as u16).to_be_bytes());
        for n in names {
            out.extend([0, 1]);
            out.extend(n.to_be_bytes());
            out.extend(desc.to_be_bytes());
            // One attribute, to be skipped.
            out.extend([0, 1, 0, 1, 0, 0, 0, 2, 0, 0]);
        }
        out.extend([0, 0]); // class attributes
        out
    }

    /// A class extending `superclass` and declaring nothing.
    pub fn class_file(name: &str, superclass: &str) -> Vec<u8> {
        class_with(name, superclass, &[], &[], false)
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::class_with as class_file;
    use super::*;

    #[test]
    fn reads_the_header_methods_and_pool() {
        let bytes = class_file(
            "a/Lamp",
            "net/minecraft/world/level/block/Block",
            &["<init>", "useWithoutItem", "lambda$x$0"],
            &["net/minecraft/world/item/Item"],
            false,
        );
        let c = parse(&bytes).unwrap();
        assert_eq!(c.name, "a/Lamp");
        assert_eq!(c.superclass, "net/minecraft/world/level/block/Block");
        assert_eq!(c.methods, ["useWithoutItem"]);
        assert!(c.refs.contains("net/minecraft/world/item/Item"));
        assert!(!c.mixin);
        assert!(parse(&bytes[..bytes.len() - 3]).is_none());
        assert!(parse(b"not a class").is_none());
    }

    #[test]
    fn follows_own_classes_to_the_game() {
        let block = "net/minecraft/world/level/block/Block";
        let files = [
            // Looks only: a template covers it.
            class_file("m/Plain", block, &["getStateForPlacement"], &[], false),
            class_file("m/Round", block, &["getShape"], &[], false),
            // Behaviour two levels down.
            class_file("m/Base", block, &["randomTick"], &[], false),
            class_file("m/Lamp", "m/Base", &[], &[], false),
            class_file(
                "m/Tile",
                "net/minecraft/world/level/block/entity/BlockEntity",
                &[],
                &[],
                false,
            ),
            class_file("m/Mix", "java/lang/Object", &[], &[], true),
            class_file(
                "m/Old",
                "net/minecraft/class_2248",
                &["method_9534"],
                &[],
                false,
            ),
            class_file(
                "m/Events",
                "java/lang/Object",
                &[],
                &[
                    "net/neoforged/neoforge/event/entity/player/PlayerInteractEvent$RightClickBlock",
                    "net/neoforged/neoforge/event/BuildCreativeModeTabContentsEvent",
                    "net/neoforged/neoforge/registries/DeferredRegister",
                ],
                false,
            ),
            class_file(
                "m/Gem",
                "net/minecraft/world/item/Item",
                &["appendHoverText"],
                &[],
                false,
            ),
        ];
        let code = census(files.iter().map(Vec::as_slice));
        assert_eq!(code.classes, 9);
        let n = |f| code.features.get(&f).copied().unwrap_or(0);
        assert_eq!(n(Feature::BlockBehaviour), 2, "{code:?}");
        assert_eq!(n(Feature::BlockShape), 1);
        assert_eq!(n(Feature::BlockEntity), 1);
        assert_eq!(n(Feature::Mixin), 1);
        assert_eq!(n(Feature::Unread), 1);
        assert_eq!(n(Feature::ItemBehaviour), 0);
        assert_eq!(
            code.events.iter().collect::<Vec<_>>(),
            ["PlayerInteractEvent"]
        );
        assert!(code.behaviour());
    }

    #[test]
    fn registration_only_is_not_behaviour() {
        let files = [
            class_file(
                "m/Mod",
                "java/lang/Object",
                &["register"],
                &[
                    "net/neoforged/neoforge/registries/DeferredRegister",
                    "net/minecraft/world/level/block/Block",
                ],
                false,
            ),
            class_file(
                "m/Pillar",
                "net/minecraft/world/level/block/RotatedPillarBlock",
                &[],
                &[],
                false,
            ),
        ];
        let code = census(files.iter().map(Vec::as_slice));
        assert!(!code.behaviour(), "{code:?}");
    }

    #[test]
    fn a_listener_takes_one_neoforge_event() {
        let event = "net/neoforged/neoforge/event/tick/ServerTickEvent$Pre";
        assert_eq!(one_event(&format!("(L{event};)V")), Some(event));
        assert_eq!(one_event(&format!("(L{event};I)V")), None);
        assert_eq!(one_event(&format!("(L{event};)Z")), None);
        assert_eq!(
            one_event("(Lnet/neoforged/neoforge/common/NeoForge;)V"),
            None
        );
        assert_eq!(one_event("(Lcom/example/MyEvent;)V"), None);
    }

    const BLOCK: &str = "net/minecraft/world/level/block/Block";
    const ENTITY_BLOCK: &str = "net/minecraft/world/level/block/EntityBlock";
    const BLOCK_ENTITY: &str = "net/minecraft/world/level/block/entity/BlockEntity";

    fn hierarchy(files: &[Vec<u8>]) -> Hierarchy {
        Hierarchy::from_classes(files.iter().filter_map(|f| parse(f)))
    }

    #[test]
    fn a_block_whose_block_entity_cooks_and_opens_a_menu() {
        // Iron Furnaces: a plain `Block` that names its block entity, whose base class and interfaces say the rest.
        let files = [
            implementing("m/Furnace", BLOCK, &[ENTITY_BLOCK], &["m/FurnaceTile"]),
            implementing("m/Iron", "m/Furnace", &[], &[]),
            implementing("m/FurnaceTile", "m/Tile", &[COOKING], &[]),
            implementing("m/Tile", BLOCK_ENTITY, &[MENU], &[]),
        ];
        let h = hierarchy(&files);
        let both = Behaviour { furnace: true, menu: true };
        assert_eq!(h.behaviour("m.Iron"), both);
        assert_eq!(h.behaviour("m/Furnace"), both);
    }

    #[test]
    fn a_machine_that_opens_a_menu_does_not_cook() {
        let files = [
            implementing("m/Machine", BLOCK, &[ENTITY_BLOCK], &["m/MachineTile"]),
            implementing("m/MachineTile", "m/Tile", &[], &[]),
            implementing("m/Tile", BLOCK_ENTITY, &[MENU], &[]),
            // The game's containers are menu providers, through `BaseContainerBlockEntity`.
            implementing("m/Chest", BLOCK, &[], &["m/ChestTile"]),
            implementing(
                "m/ChestTile",
                "net/minecraft/world/level/block/entity/BaseContainerBlockEntity",
                &[],
                &[],
            ),
        ];
        let h = hierarchy(&files);
        assert_eq!(h.behaviour("m.Machine"), Behaviour { furnace: false, menu: true });
        assert_eq!(h.behaviour("m.Chest"), Behaviour { furnace: false, menu: true });
    }

    #[test]
    fn a_block_extending_the_games_furnace_block_cooks() {
        let files = [implementing(
            "m/Kiln",
            "net/minecraft/world/level/block/AbstractFurnaceBlock",
            &[],
            &[],
        )];
        assert!(hierarchy(&files).behaviour("m.Kiln").furnace);
    }

    #[test]
    fn a_block_entity_extending_the_games_furnace_cooks() {
        let files = [
            implementing("m/Smelter", BLOCK, &[ENTITY_BLOCK], &["m/SmelterTile"]),
            implementing(
                "m/SmelterTile",
                "net/minecraft/world/level/block/entity/AbstractFurnaceBlockEntity",
                &[],
                &[],
            ),
        ];
        assert!(hierarchy(&files).behaviour("m.Smelter").furnace);
    }

    #[test]
    fn game_classes_a_block_only_names_do_not_make_it_a_cooker() {
        // A block that reads the game's furnace fuel table names its block entity class, but is not one.
        let files = [
            implementing(
                "m/Burner",
                BLOCK,
                &[],
                &[
                    "net/minecraft/world/level/block/entity/AbstractFurnaceBlockEntity",
                    "net/minecraft/world/MenuProvider",
                ],
            ),
            // Nor is a class that holds a recipe and is no block entity: a menu.
            implementing("m/Panel", BLOCK, &[], &["m/PanelMenu"]),
            implementing("m/PanelMenu", "java/lang/Object", &[COOKING, MENU], &[]),
        ];
        let h = hierarchy(&files);
        assert_eq!(h.behaviour("m.Burner"), Behaviour::default());
        assert_eq!(h.behaviour("m.Panel"), Behaviour::default());
        assert_eq!(h.behaviour("m.NotInTheJar"), Behaviour::default());
    }

    #[test]
    fn a_block_implementing_menu_provider_opens_a_menu() {
        let files = [implementing("m/Altar", BLOCK, &[MENU], &[])];
        assert_eq!(
            hierarchy(&files).behaviour("m.Altar"),
            Behaviour { furnace: false, menu: true }
        );
    }

    const COOKING: &str = "net/minecraft/world/inventory/RecipeCraftingHolder";
    const MENU: &str = "net/minecraft/world/MenuProvider";

    fn implementing(name: &str, superclass: &str, interfaces: &[&str], refs: &[&str]) -> Vec<u8> {
        super::tests_support::class_implementing(name, superclass, interfaces, &[], refs, false)
    }
}
