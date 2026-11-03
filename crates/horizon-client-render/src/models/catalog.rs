//! Bind prepared registry states to model choices; bake each transformed model
//! once and share its geometry across positions. Resource failure recovery and
//! resource-pack reloads remain separate work.
use crate::{Result, sprites::Sprites};
use horizon_assets::{
    bake::{BlockModelTransform, ModelQuad, bake_block_model},
    blockstate::{
        BlockStateDefinition, Variant, VariantSet,
        bind::{Properties, PropertySchema, PropertyValues, SelectedModels},
        choice::CompiledChoices,
    },
};
use horizon_core::{
    position::BlockPos,
    random::{LegacyRandom, position_seed},
};
use horizon_world::{
    chunk::BlockState,
    registry::{BlockRegistry, BlockStateId},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    path::Path,
    sync::{Arc, OnceLock},
};

pub struct BakedFace {
    pub quad: ModelQuad,
    pub layer: u32,
    /// The texels under the quad's UVs are all opaque (`SpriteContents.computeTransparency`):
    /// drawn in the solid layer without an alpha test.
    pub opaque: bool,
    /// Fractional alpha: drawn in the sorted translucent pass.
    pub translucent: bool,
    /// The Fusion connecting texture rule its sprite joins by ([`crate::fusion`]), plus one:
    /// two bytes, within the padding every face already has.
    pub fusion: Option<std::num::NonZeroU16>,
}
pub struct BakedModel {
    pub ambient_occlusion: bool,
    /// The model's particle slot also exists for models without block faces.
    pub particle: Option<u32>,
    pub faces: Vec<BakedFace>,
    pub cube: Option<[super::cube::CubeStyle; 6]>,
}
impl BakedModel {
    /// No faces: what a model that didn't bake draws.
    fn empty() -> Self {
        Self {
            ambient_occlusion: true,
            particle: None,
            faces: Vec::new(),
            cube: None,
        }
    }
}
/// The block states from which models bake when a chunk first draws them ([`StateModels`]'s
/// lazy load): All the Mods 10's 276,297 baked variants held 6.26 million faces, 1.25 GB.
pub const LAZY_MODELS_FROM: usize = 200_000;
/// What a model not baked at the load bakes from: its resolved library and the laid-out sprites.
struct LazyBake {
    library: Arc<horizon_assets::resolve::ModelLibrary>,
    sprites: crate::sprites::SpriteLookup,
}
impl LazyBake {
    /// `bake_resolved` from the laid-out sprites, without the sprite names.
    fn bake(&self, variant: &Variant) -> Result<BakedModel> {
        let model = self.library.resolve(&variant.model)?;
        let sprites = &self.sprites;
        let particle = model
            .material("particle")?
            .map(|material| sprites.layer(&material.sprite));
        let transform = BlockModelTransform::new(variant.rotations, variant.uv_lock);
        let faces: Vec<BakedFace> = bake_block_model(&model, transform)?
            .into_iter()
            .map(|mut quad| {
                let layer = sprites.layer(&quad.material.sprite);
                quad.material.sprite = String::new();
                BakedFace {
                    translucent: quad.material.force_translucent || sprites.translucent(layer),
                    opaque: sprites.region_opaque(layer, &quad.uv),
                    quad,
                    layer,
                    fusion: None,
                }
            })
            .collect();
        let cube = super::cube::cube_styles(&faces);
        Ok(BakedModel {
            ambient_occlusion: model.ambient_occlusion,
            particle,
            faces,
            cube,
        })
    }
}
/// The sprites `bake_resolved` places for `model`, in its order (the particle, then each face's),
/// without keeping the bake: the layers its faces draw, and the first face's layer on each side.
fn place_sprites(
    root: &Path,
    sprites: &mut Sprites,
    model: &horizon_assets::resolve::ResolvedCuboidModel,
    turns: [u8; 3],
    lock: bool,
) -> Result<(Box<[u32]>, Sides)> {
    if let Some(material) = model.material("particle")? {
        sprites.insert(root, &material.sprite)?;
    }
    let (mut layers, mut sides) = (Vec::new(), [None; 6]);
    for quad in bake_block_model(model, BlockModelTransform::new(turns, lock))? {
        let layer = sprites.insert(root, &quad.material.sprite)?;
        let side = &mut sides[side_index(quad.facing)];
        side.get_or_insert(layer);
        layers.push(layer);
    }
    layers.sort_unstable();
    layers.dedup();
    Ok((layers.into(), sides))
}
/// A model's first face layer on each side, in [`horizon_assets::model::Direction::ALL`]'s order.
type Sides = [Option<u32>; 6];
fn side_index(side: horizon_assets::model::Direction) -> usize {
    horizon_assets::model::Direction::ALL
        .iter()
        .position(|d| *d == side)
        .unwrap_or(0)
}
type VariantKey = (String, [u8; 3], bool);
/// Empty by default: no state has a model (a dedicated server, which draws
/// nothing).
#[derive(Default)]
pub struct StateModels {
    /// Each definition's choices, a variant standing as its index in `baked`.
    choices: Vec<CompiledChoices<u32>>,
    /// By state id: its definition in `choices` and the models it selects.
    states: Vec<Option<(usize, SelectedModels)>>,
    /// Every transformed model once, with the variant it was baked from: at the load, or for a
    /// large pack when first drawn ([`LAZY_MODELS_FROM`], [`Self::finish_loading`]).
    baked: Vec<(Variant, OnceLock<BakedModel>)>,
    /// Standalone models by id: their index in `baked`.
    standalone: BTreeMap<String, u32>,
    /// A lazy load's models, until [`Self::finish_loading`]: they resolve from here.
    library: Option<Arc<horizon_assets::resolve::ModelLibrary>>,
    /// A lazy load's layers each variant's faces draw, until [`Self::finish_loading`].
    variant_layers: Vec<Box<[u32]>>,
    /// A lazy load's first face layer on each side of each variant: what [`Self::side_layer`]
    /// answers for a model not baked yet.
    variant_sides: Vec<Sides>,
    /// What the models not baked yet bake from, once the load is finished.
    lazy: Option<Arc<LazyBake>>,
}
fn key(variant: &Variant) -> VariantKey {
    (variant.model.clone(), variant.rotations, variant.uv_lock)
}
/// Bakes model `name` of `library`, turned and UV-locked as a variant says.
fn bake_variant(
    root: &Path,
    sprites: &mut Sprites,
    library: &horizon_assets::resolve::ModelLibrary,
    name: &str,
    turns: [u8; 3],
    lock: bool,
) -> Result<BakedModel> {
    bake_resolved(root, sprites, &library.resolve(name)?, turns, lock)
}
/// Bakes a resolved `model`, turned and UV-locked as a variant says.
fn bake_resolved(
    root: &Path,
    sprites: &mut Sprites,
    model: &horizon_assets::resolve::ResolvedCuboidModel,
    turns: [u8; 3],
    lock: bool,
) -> Result<BakedModel> {
    let particle = model
        .material("particle")?
        .map(|material| sprites.insert(root, &material.sprite))
        .transpose()?;
    let faces: Vec<BakedFace> = bake_block_model(model, BlockModelTransform::new(turns, lock))?
        .into_iter()
        .map(|quad| {
            let layer = sprites.insert(root, &quad.material.sprite)?;
            Ok(BakedFace {
                translucent: quad.material.force_translucent || sprites.translucent[layer as usize],
                opaque: sprites.region_opaque(layer, &quad.uv),
                quad,
                layer,
                fusion: None,
            })
        })
        .collect::<Result<_>>()?;
    let cube = super::cube::cube_styles(&faces);
    Ok(BakedModel {
        ambient_occlusion: model.ambient_occlusion,
        particle,
        faces,
        cube,
    })
}
/// The definitions a blockstate file holds, bottom first: the layers of a
/// file the pack overlay stacked (`packs::BLOCKSTATE_LAYERS`), else the
/// file itself.
fn definition_layers(text: &str) -> std::result::Result<Vec<String>, String> {
    if !text.contains(crate::packs::BLOCKSTATE_LAYERS) {
        return Ok(vec![text.to_owned()]);
    }
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    match value.get(crate::packs::BLOCKSTATE_LAYERS) {
        Some(Value::Array(layers)) => Ok(layers.iter().map(Value::to_string).collect()),
        _ => Ok(vec![text.to_owned()]),
    }
}
/// Each state's model from definitions bound bottom first: the topmost one
/// that binds the state, as (its layer, its selection). A higher pack's
/// definition replaces only the states it binds (`putAll` in
/// `loadBlockStateDefinitionStack`).
fn topmost(layers: Vec<Vec<Option<SelectedModels>>>) -> Vec<Option<(usize, SelectedModels)>> {
    let mut picked: Vec<Option<(usize, SelectedModels)>> = Vec::new();
    for (layer, bound) in layers.into_iter().enumerate() {
        picked.resize(picked.len().max(bound.len()), None);
        for (slot, selected) in picked.iter_mut().zip(bound) {
            if let Some(selected) = selected {
                *slot = Some((layer, selected));
            }
        }
    }
    picked
}
/// `model-states.json` by block: as a tree, or as text a block at a time.
pub(crate) trait ModelStateFacts {
    fn block(&self, name: &str) -> Result<Option<std::borrow::Cow<'_, Value>>>;
}
impl ModelStateFacts for BTreeMap<String, Value> {
    fn block(&self, name: &str) -> Result<Option<std::borrow::Cow<'_, Value>>> {
        Ok(self.get(name).map(std::borrow::Cow::Borrowed))
    }
}
impl ModelStateFacts for BTreeMap<String, Box<serde_json::value::RawValue>> {
    fn block(&self, name: &str) -> Result<Option<std::borrow::Cow<'_, Value>>> {
        self.get(name)
            .map(|text| Ok(std::borrow::Cow::Owned(serde_json::from_str(text.get())?)))
            .transpose()
    }
}
impl StateModels {
    pub fn load(
        root: &Path,
        registry: &BlockRegistry,
        blocks: &[&str],
        sprites: &mut Sprites,
    ) -> Result<Self> {
        let facts: BTreeMap<String, Value> =
            serde_json::from_slice(&fs::read(root.join("model-states.json"))?)?;
        Self::from_facts(root, registry, blocks, sprites, &facts)
    }
    /// Bind and bake models using an already parsed model-state document.
    pub fn from_facts(
        root: &Path,
        registry: &BlockRegistry,
        blocks: &[&str],
        sprites: &mut Sprites,
        facts: &BTreeMap<String, Value>,
    ) -> Result<Self> {
        Self::from_facts_with_models(
            root,
            registry,
            blocks,
            sprites,
            facts,
            &crate::model_library::ParsedModels::default(),
            false,
        )
    }
    pub(crate) fn from_facts_with_models(
        root: &Path,
        registry: &BlockRegistry,
        blocks: &[&str],
        sprites: &mut Sprites,
        facts: &impl ModelStateFacts,
        models: &crate::model_library::ParsedModels,
        lazy: bool,
    ) -> Result<Self> {
        let mut choices = Vec::new();
        let mut states = vec![None; registry.len()];
        let mut variants = BTreeSet::new();
        let mut by_name: std::collections::HashMap<&str, Vec<BlockStateId>> =
            std::collections::HashMap::new();
        for i in 0..registry.len() {
            let id = BlockStateId(i as u32);
            if let Some(state) = registry.state(id) {
                by_name.entry(state.name.as_str()).or_default().push(id);
            }
        }
        for block in blocks.iter().copied().collect::<BTreeSet<_>>() {
            // Vanilla blocks come bare; content mods' blocks with their namespace.
            let (namespace, path) = block.split_once(':').unwrap_or(("minecraft", block));
            let name = format!("{namespace}:{path}");
            let facts = facts
                .block(&name)?
                .ok_or("missing block model state schema")?;
            let schema: PropertySchema = facts["schema"]
                .as_object()
                .ok_or("invalid model schema")?
                .iter()
                .map(|(key, value)| {
                    let values = if let Some(range) = value.get("integer") {
                        PropertyValues::Integer {
                            minimum: i32::try_from(
                                range[0]
                                    .as_i64()
                                    .ok_or("invalid integer property minimum")?,
                            )?,
                            maximum: i32::try_from(
                                range[1]
                                    .as_i64()
                                    .ok_or("invalid integer property maximum")?,
                            )?,
                        }
                    } else {
                        PropertyValues::Named(
                            value["named"]
                                .as_array()
                                .ok_or("missing named property values")?
                                .iter()
                                .map(|v| {
                                    v.as_str()
                                        .map(str::to_owned)
                                        .ok_or("invalid named property value")
                                })
                                .collect::<std::result::Result<_, _>>()?,
                        )
                    };
                    Ok((key.clone(), values))
                })
                .collect::<Result<_>>()?;
            let possible: Vec<Properties> = serde_json::from_value(facts["states"].clone())?;
            // Vanilla's BlockStateModelLoader: a definition that does not
            // read or bind is logged and left out; with none left the block
            // gets no model, and the other blocks still load. A file several
            // packs provide binds each pack's definition on its own, bottom
            // first (`loadBlockStateDefinitionStack`).
            let layers = match horizon_core::prepared_files::read_to_string(
                &root.join(format!("assets/{namespace}/blockstates/{path}.json")),
            )
            .map_err(|e| e.to_string())
            .and_then(|text| definition_layers(&text))
            {
                Ok(layers) => layers,
                Err(e) => {
                    log::warn!(
                        "Horizon: blockstate definition of {namespace}:{path} not loaded: {e}"
                    );
                    continue;
                }
            };
            let mut loaded = Vec::with_capacity(layers.len());
            for text in &layers {
                let layer = BlockStateDefinition::from_json(text)
                    .map_err(|e| e.to_string())
                    .and_then(|definition| {
                        let bound = definition
                            .bind(&schema, &possible)
                            .map_err(|e| e.to_string())?;
                        Ok((definition, bound))
                    });
                match layer {
                    Ok(layer) => loaded.push(layer),
                    Err(e) => log::warn!(
                        "Horizon: blockstate definition of {namespace}:{path} not loaded: {e}"
                    ),
                }
            }
            if loaded.is_empty() {
                continue;
            }
            let native_order: Vec<_> = by_name
                .get(name.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .map(|&id| (id, &registry.state(id).expect("indexed state").properties))
                .collect();
            if native_order.len() != possible.len()
                || native_order
                    .iter()
                    .zip(&possible)
                    .any(|((_, a), b)| *a != b)
            {
                return Err(
                    "model state definitions disagree with the imported block registry".into(),
                );
            }
            let (definitions, bound): (Vec<_>, Vec<_>) = loaded.into_iter().unzip();
            let first = choices.len();
            for ((id, _), picked) in native_order.into_iter().zip(topmost(bound)) {
                if let Some((layer, selected)) = picked {
                    states[id.0 as usize] = Some((first + layer, selected));
                }
            }
            for definition in definitions {
                for choice in definition
                    .variants
                    .iter()
                    .flatten()
                    .map(|(_, choice)| choice)
                    .chain(
                        definition
                            .multipart
                            .iter()
                            .flatten()
                            .map(|selector| &selector.variant),
                    )
                {
                    match choice {
                        VariantSet::Single(v) => {
                            variants.insert(key(v));
                        }
                        VariantSet::Weighted(v) => variants.extend(v.iter().map(|(v, _)| key(v))),
                    }
                }
                choices.push(CompiledChoices::new(&definition)?);
            }
        }
        let names: Vec<String> = variants.iter().map(|(name, _, _)| name.clone()).collect();
        let library = match models.block_library(root, names.clone()) {
            Ok(library) => library,
            Err(e) => {
                log::warn!("Horizon: block models incomplete ({e}); using the missing model");
                models.block_library_or_missing(root, names)?
            }
        };
        let resolved: Vec<_> = variants
            .iter()
            .map(|(name, ..)| library.resolve(name))
            .collect();
        // Every texture the models name, decoded side by side before the bake adds them one
        // by one in its own order.
        sprites.prefetch(
            root,
            resolved
                .iter()
                .flatten()
                .flat_map(|model| model.textures.values())
                .map(|material| material.sprite.as_str()),
        );
        let mut baked = Vec::with_capacity(variants.len());
        let mut variant_layers = Vec::new();
        let mut variant_sides = Vec::new();
        let mut index = BTreeMap::new();
        for ((name, turns, lock), model) in variants.into_iter().zip(resolved) {
            let model = model?;
            let model = if lazy {
                // Its sprites placed as the bake would, so the layers come out the same.
                let (layers, sides) = place_sprites(root, sprites, &model, turns, lock)?;
                variant_layers.push(layers);
                variant_sides.push(sides);
                OnceLock::new()
            } else {
                OnceLock::from(bake_resolved(root, sprites, &model, turns, lock)?)
            };
            index.insert((name.clone(), turns, lock), baked.len() as u32);
            let variant = Variant {
                model: name,
                rotations: turns,
                uv_lock: lock,
            };
            baked.push((variant, model));
        }
        // Every variant of every definition was baked or placed above.
        let choices = choices.iter().map(|c| c.map(|v| index[&key(v)])).collect();
        Ok(Self {
            choices,
            states,
            baked,
            standalone: BTreeMap::new(),
            library: lazy.then(|| Arc::new(library)),
            variant_layers,
            variant_sides,
            lazy: None,
        })
    }
    /// The layer of the first face on `side` of the models a state draws at `position` (connected
    /// textures' tile rules): from the faces, or a model not baked yet from its placed sprites.
    pub fn side_layer(
        &self,
        state: BlockStateId,
        position: BlockPos,
        side: horizon_assets::model::Direction,
    ) -> Option<u32> {
        let (choice, selected) = self.state(state)?;
        let mut random = LegacyRandom::new(position_seed(position));
        let mut found = None;
        self.choices[*choice]
            .for_each(selected, &mut random, |&model| {
                if found.is_some() {
                    return;
                }
                let model = model as usize;
                found = match self.baked[model].1.get() {
                    Some(baked) => baked
                        .faces
                        .iter()
                        .find(|f| f.quad.facing == side)
                        .map(|f| f.layer),
                    None => self
                        .variant_sides
                        .get(model)
                        .and_then(|sides| sides[side_index(side)]),
                };
            })
            .ok()?;
        found
    }
    /// The model at `index`, baked now if it waited ([`LAZY_MODELS_FROM`]).
    fn model(&self, index: usize) -> (&Variant, &BakedModel) {
        let (variant, baked) = &self.baked[index];
        // Asked for during the load (the gallery wants only the variants): nothing yet, and
        // nothing kept, so it bakes once the sprites are laid out.
        let Some(lazy) = self.lazy.as_ref().filter(|_| baked.get().is_none()) else {
            static NONE: OnceLock<BakedModel> = OnceLock::new();
            return (variant, baked.get().unwrap_or_else(|| NONE.get_or_init(BakedModel::empty)));
        };
        let model = baked.get_or_init(|| {
            lazy.bake(variant).unwrap_or_else(|e| {
                log::warn!("Horizon: block model {} not baked: {e}", variant.model);
                BakedModel::empty()
            })
        });
        (variant, model)
    }
    /// A lazy load bakes now, with their sprite names, the models whose faces draw one of
    /// `layers`: Fusion matches their faces by sprite name ([`crate::fusion`]). The others have
    /// none of its rules, and bake when drawn.
    pub(crate) fn bake_showing(&mut self, root: &Path, sprites: &mut Sprites, layers: &HashSet<u32>) {
        let Some(library) = self.library.clone() else {
            return;
        };
        let wanted: Vec<usize> = self
            .variant_layers
            .iter()
            .enumerate()
            .filter(|(i, drawn)| {
                self.baked[*i].1.get().is_none() && drawn.iter().any(|l| layers.contains(l))
            })
            .map(|(i, _)| i)
            .collect();
        for i in wanted {
            let (variant, slot) = &self.baked[i];
            let baked = library
                .resolve(&variant.model)
                .map_err(Into::into)
                .and_then(|model| {
                    bake_resolved(root, sprites, &model, variant.rotations, variant.uv_lock)
                });
            match baked {
                Ok(model) => {
                    let _ = slot.set(model);
                }
                Err(e) => log::warn!("Horizon: block model {} not baked: {e}", variant.model),
            }
        }
    }
    /// The sprites are placed: the faces baked so far let go of their names, and a lazy load's
    /// other models bake from `sprites`' layout when first drawn.
    pub(crate) fn finish_loading(&mut self, sprites: &Sprites) {
        self.forget_sprite_names();
        self.variant_layers = Vec::new();
        if let Some(library) = self.library.take() {
            self.lazy = Some(Arc::new(LazyBake {
                library,
                sprites: sprites.lookup(),
            }));
        }
    }
    /// Bakes `names`, models no blockstate needs to name (vanilla's
    /// standalone models, NeoForge's `ModelEvent.RegisterStandalone`), as
    /// they are, unturned, for [`Self::standalone`]. One a block state
    /// already bakes unturned is shared. A model that doesn't read (missing,
    /// or a parent missing) is logged and left out, as a missing blockstate
    /// is; the others still bake.
    pub(crate) fn bake_standalone(
        &mut self,
        root: &Path,
        sprites: &mut Sprites,
        models: &crate::model_library::ParsedModels,
        names: &[String],
    ) -> Result<()> {
        for name in names {
            if self.standalone.contains_key(name) {
                continue;
            }
            if let Some(i) = self
                .baked
                .iter()
                .position(|(v, _)| v.model == *name && v.rotations == [0; 3] && !v.uv_lock)
            {
                self.standalone.insert(name.clone(), i as u32);
                continue;
            }
            let model = models
                .block_library(root, vec![name.clone()])
                .and_then(|library| bake_variant(root, sprites, &library, name, [0; 3], false));
            match model {
                Ok(model) => {
                    self.standalone
                        .insert(name.clone(), self.baked.len() as u32);
                    let variant = Variant {
                        model: name.clone(),
                        rotations: [0; 3],
                        uv_lock: false,
                    };
                    self.baked.push((variant, OnceLock::from(model)));
                }
                Err(e) => log::warn!("Horizon: standalone model {name} not loaded: {e}"),
            }
        }
        Ok(())
    }
    /// Baked models, their faces, the states with a model and how many multipart
    /// selections those states hold: what the catalog's memory goes by.
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let baked = self.baked.iter().filter_map(|(_, model)| model.get());
        let faces = baked.clone().map(|model| model.faces.len()).sum();
        let states = self.states.iter().flatten();
        let parts = states
            .clone()
            .map(|(_, selected)| match selected {
                SelectedModels::Multipart(parts) => parts.len(),
                SelectedModels::Variant(_) => 0,
            })
            .sum();
        (baked.count(), faces, states.count(), parts)
    }
    /// Lets go of each face's sprite name, once whatever matches faces by name has run: `layer` says
    /// which sprite a face draws. A large pack has millions of faces, each with its own string.
    pub(crate) fn forget_sprite_names(&mut self) {
        for model in self.baked.iter_mut().filter_map(|(_, model)| model.get_mut()) {
            for face in &mut model.faces {
                face.quad.material.sprite = String::new();
            }
        }
    }
    /// Every model baked so far with the variant it was baked from, to change its faces.
    pub fn models_mut(&mut self) -> impl Iterator<Item = (&Variant, &mut BakedModel)> {
        self.baked
            .iter_mut()
            .filter_map(|(variant, model)| Some((&*variant, model.get_mut()?)))
    }
    /// A model baked by [`Self::bake_standalone`], by id (`mymod:block/shaft_half`).
    pub fn standalone(&self, name: &str) -> Option<&BakedModel> {
        let &i = self.standalone.get(name)?;
        Some(self.model(i as usize).1)
    }
    pub fn collect(
        &self,
        state: BlockStateId,
        position: BlockPos,
    ) -> Result<Vec<(&Variant, &BakedModel)>> {
        // Blocks use their position seed; contained/displayed blocks can supply one.
        self.collect_seeded(state, position_seed(position))
    }
    /// `BlockStateModel.particleMaterial`: the particle sprite layer of the
    /// state's first variant (see `CompiledChoices::particle_variant`).
    pub fn particle(&self, state: BlockStateId) -> Option<u32> {
        let (choice, selected) = self.state(state)?;
        let model = self.choices[*choice].particle_variant(selected)?;
        self.model(*model as usize).1.particle
    }
    pub fn collect_seeded(
        &self,
        state: BlockStateId,
        seed: i64,
    ) -> Result<Vec<(&Variant, &BakedModel)>> {
        let mut models = Vec::new();
        self.each_seeded(state, seed, |variant, model| models.push((variant, model)))?;
        Ok(models)
    }
    /// The models a block draws at `position`, into `out` (cleared first):
    /// `collect` for the mesher, which reuses one list for every block.
    pub fn models_at<'a>(
        &'a self,
        state: BlockStateId,
        position: BlockPos,
        out: &mut Vec<&'a BakedModel>,
    ) -> Result<()> {
        out.clear();
        self.each_seeded(state, position_seed(position), |_, model| out.push(model))
    }
    fn each_seeded<'a>(
        &'a self,
        state: BlockStateId,
        seed: i64,
        mut f: impl FnMut(&'a Variant, &'a BakedModel),
    ) -> Result<()> {
        let (choice, selected) = self.state(state).ok_or("unbound gallery block state")?;
        let mut random = LegacyRandom::new(seed);
        self.choices[*choice].for_each(selected, &mut random, |&model| {
            let (variant, model) = self.model(model as usize);
            f(variant, model);
        })?;
        Ok(())
    }
    fn state(&self, state: BlockStateId) -> Option<&(usize, SelectedModels)> {
        self.states.get(state.0 as usize)?.as_ref()
    }
}

pub fn descriptor(name: &str, overrides: &[(&str, &str)]) -> BlockState {
    BlockState {
        name: format!("minecraft:{name}"),
        properties: overrides
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_library::ParsedModels;

    /// #7013: a pack's blockstate file that defines only some states (here
    /// a fence gate's `powered=true` ones) keeps vanilla's model for the
    /// rest, as vanilla binds each pack's definition and lets a higher one
    /// replace only what it binds.
    #[test]
    fn a_partial_pack_blockstate_keeps_vanilla_for_the_other_states() {
        let schema: PropertySchema = BTreeMap::from([(
            "powered".to_owned(),
            PropertyValues::Named(BTreeSet::from(["false".to_owned(), "true".to_owned()])),
        )]);
        let states: Vec<Properties> = ["false", "true"]
            .iter()
            .map(|p| BTreeMap::from([("powered".to_owned(), (*p).to_owned())]))
            .collect();
        let stacked = serde_json::json!({ crate::packs::BLOCKSTATE_LAYERS: [
            {"variants":{"":{"model":"minecraft:block/oak_fence_gate"}}},
            {"variants":{"powered=true":{"model":"minecraft:block/stone"}}}
        ]})
        .to_string();
        let layers = definition_layers(&stacked).unwrap();
        assert_eq!(layers.len(), 2);
        let bound = layers
            .iter()
            .map(|text| {
                BlockStateDefinition::from_json(text)
                    .unwrap()
                    .bind(&schema, &states)
                    .unwrap()
            })
            .collect();
        let picked = topmost(bound);
        assert_eq!(picked[0], Some((0, SelectedModels::Variant(0))));
        assert_eq!(picked[1], Some((1, SelectedModels::Variant(0))));
        // A file that isn't stacked is one definition.
        let plain = r#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#;
        assert_eq!(definition_layers(plain).unwrap(), vec![plain.to_owned()]);
    }

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn write(&self, name: &str, json: &str) {
            let (namespace, path) = name.split_once(':').unwrap();
            let path = self
                .0
                .join(format!("assets/{namespace}/models/{path}.json"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, json).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn standalone_models_bake_once_and_skip_what_does_not_read() -> Result<()> {
        let fixture = Fixture(
            std::env::temp_dir().join(format!("horizon-standalone-{}", std::process::id())),
        );
        // A half shaft: one element, two faces; a child with its parent's elements.
        fixture.write(
            "lanef:block/shaft_half",
            r##"{"textures":{"side":"lanef:block/shaft"},"elements":[{"from":[6,6,0],"to":[10,10,8],"faces":{"north":{"texture":"#side"},"up":{"texture":"#side"}}}]}"##,
        );
        fixture.write(
            "lanef:block/cog",
            r#"{"parent":"lanef:block/shaft_half","textures":{"side":"lanef:block/cog"}}"#,
        );
        fixture.write("lanef:block/orphan", r#"{"parent":"lanef:block/absent"}"#);
        // Real textures: every missing one shares a layer.
        for (name, shade) in [("shaft", 40u8), ("cog", 200u8)] {
            let path = fixture
                .0
                .join(format!("assets/lanef/textures/block/{name}.png"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), 16, 16);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[shade; 16 * 16 * 4])
                .unwrap();
        }
        let mut models = StateModels::default();
        let mut sprites = Sprites::default();
        let names: Vec<String> = [
            "lanef:block/shaft_half",
            "lanef:block/cog",
            "lanef:block/orphan",
            "lanef:block/absent",
            "lanef:block/shaft_half",
        ]
        .map(str::to_owned)
        .into();
        let parsed = ParsedModels::load(&fixture.0);
        models.bake_standalone(&fixture.0, &mut sprites, &parsed, &names)?;
        let shaft = models.standalone("lanef:block/shaft_half").expect("baked");
        assert_eq!(shaft.faces.len(), 2);
        let cog = models.standalone("lanef:block/cog").expect("baked");
        assert_eq!(cog.faces.len(), 2);
        assert_ne!(cog.faces[0].layer, shaft.faces[0].layer, "its own texture");
        assert!(models.standalone("lanef:block/orphan").is_none());
        assert!(models.standalone("lanef:block/absent").is_none());
        assert!(models.standalone("lanef:block/unnamed").is_none());
        // Named twice, baked once.
        assert_eq!(models.baked.len(), 2);
        // The names go; the layers stay.
        let layer = cog.faces[0].layer;
        models.forget_sprite_names();
        let cog = models.standalone("lanef:block/cog").expect("baked");
        assert_eq!(cog.faces[0].layer, layer);
        assert!(cog.faces[0].quad.material.sprite.is_empty());
        // Without a parsed library, the files are read directly.
        let mut direct = StateModels::default();
        direct.bake_standalone(&fixture.0, &mut sprites, &ParsedModels::default(), &names)?;
        assert_eq!(
            direct.standalone("lanef:block/cog").map(|m| m.faces.len()),
            Some(2)
        );
        Ok(())
    }

    #[test]
    fn a_standalone_model_a_state_bakes_unturned_is_shared() -> Result<()> {
        let mut models = StateModels::default();
        let variant = |rotations| Variant {
            model: "lanef:block/shared".into(),
            rotations,
            uv_lock: false,
        };
        let empty = || BakedModel {
            ambient_occlusion: true,
            particle: None,
            faces: Vec::new(),
            cube: None,
        };
        models.baked.push((variant([0, 90, 0]), OnceLock::from(empty())));
        models.baked.push((variant([0; 3]), OnceLock::from(empty())));
        let names = vec!["lanef:block/shared".to_owned()];
        let root = std::env::temp_dir().join("horizon-standalone-none");
        models.bake_standalone(
            &root,
            &mut Sprites::default(),
            &ParsedModels::default(),
            &names,
        )?;
        assert_eq!(models.baked.len(), 2);
        assert_eq!(models.standalone.get("lanef:block/shared"), Some(&1));
        Ok(())
    }
}

#[cfg(test)]
mod lazy_tests {
    use super::*;
    use crate::model_library::ParsedModels;

    /// Over vanilla's prepared resources: the same sprite layers, and every state's models the
    /// same, face by face, baked at the load or when first asked for.
    #[test]
    fn a_lazy_load_bakes_what_an_eager_one_does() -> Result<()> {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client")
            .join(horizon_core::VANILLA_VERSION);
        if !root.join("model-states.json").exists() {
            return Ok(());
        }
        let registry = horizon_world::level::data::load_registry(&root)?;
        let parsed = ParsedModels::load(&root);
        let load = |lazy| -> Result<(Sprites, StateModels)> {
            let mut sprites = Sprites::default();
            let mut catalog =
                crate::models::prepare_with_models(&root, &registry, &mut sprites, &parsed, lazy)?
                    .catalog;
            catalog.finish_loading(&sprites);
            Ok((sprites, catalog))
        };
        let (eager_sprites, eager) = load(false)?;
        let (lazy_sprites, lazy) = load(true)?;
        // A second lazy load, asked for sides before anything bakes.
        let (_, lazy_sides) = load(true)?;
        assert_eq!(eager_sprites.layers, lazy_sprites.layers);
        assert!(lazy.counts().0 < eager.counts().0, "nothing waited");
        let face = |f: &BakedFace| {
            format!(
                "{} {} {} {:?} {:?} {:?} {:?} {:?} {} {:?} {} {}",
                f.layer,
                f.opaque,
                f.translucent,
                f.fusion,
                f.quad.positions,
                f.quad.uv,
                f.quad.facing,
                f.quad.cull,
                f.quad.tint_index,
                f.quad.shade_direction_override,
                f.quad.light_emission,
                f.quad.material.force_translucent,
            )
        };
        let model = |m: &BakedModel| {
            (
                m.ambient_occlusion,
                m.particle,
                m.faces.iter().map(face).collect::<Vec<_>>(),
                format!("{:?}", m.cube),
            )
        };
        let mut compared = 0;
        for id in 0..registry.len() as u32 {
            let state = BlockStateId(id);
            let at = BlockPos::new(id as i32 % 7, 64, id as i32 % 13);
            let (Ok(a), Ok(b)) = (eager.collect(state, at), lazy.collect(state, at)) else {
                assert_eq!(eager.collect(state, at).is_ok(), lazy.collect(state, at).is_ok());
                continue;
            };
            assert_eq!(a.len(), b.len(), "state {id}");
            for ((va, ma), (vb, mb)) in a.iter().zip(&b) {
                assert_eq!(va, vb, "state {id}");
                assert_eq!(model(ma), model(mb), "state {id}, {}", va.model);
                compared += 1;
            }
            assert_eq!(eager.particle(state), lazy.particle(state), "state {id}");
            for side in horizon_assets::model::Direction::ALL {
                assert_eq!(
                    eager.side_layer(state, at, side),
                    lazy_sides.side_layer(state, at, side),
                    "state {id} {side:?}"
                );
            }
        }
        assert!(compared > 10_000, "{compared}");
        Ok(())
    }
}
