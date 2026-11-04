//! Baking of item models: a cuboid model's quads, or the layer extrusion of
//! `builtin/generated` (`ItemModelGenerator`), with the display transforms
//! and render properties a `CuboidItemModelWrapper` keeps.
use crate::{
    bake::{ModelQuad, bake_block_model, bake_flat_face, default_uv},
    item_model::DisplayContext,
    json::Result,
    math::{Mat4, quaternion_xyz},
    model::{Direction, DisplayTransform, FluidContainer, GuiLight, ItemTransform, Material},
    resolve::{ModelLibrary, ResolvedCuboidModel},
};
use std::{collections::BTreeSet, f32::consts::PI};

pub const MISSING_SPRITE: &str = "minecraft:missingno";

/// The alpha of every unique animation frame of a sprite.
#[derive(Clone, Debug, PartialEq)]
pub struct SpriteMask {
    pub width: usize,
    pub height: usize,
    /// `frames[f][y * width + x]` is true for alpha 0.
    pub frames: Vec<Vec<bool>>,
}
impl SpriteMask {
    pub fn opaque(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            frames: vec![vec![false; width * height]],
        }
    }
    fn transparent(&self, frame: usize, x: i64, y: i64) -> bool {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return true;
        }
        self.frames[frame][y as usize * self.width + x as usize]
    }
}
/// The sprites of the item and block atlases.
pub trait SpriteLookup {
    /// `None` when the sprite is in neither atlas (the missing sprite stands in).
    fn mask(&self, sprite: &str) -> Option<SpriteMask>;
    /// How a fluid container model draws `fluid` (a source or flowing fluid's
    /// id); `None` for a fluid the game lacks, which `BuiltInRegistries.FLUID`
    /// reads as empty: the container draws no fluid.
    fn fluid(&self, fluid: &str) -> Option<FluidLook> {
        let _ = fluid;
        None
    }
}
/// A fluid as `DynamicFluidContainerModel` draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct FluidLook {
    /// `IClientFluidTypeExtensions.getStillTexture()`: a block atlas sprite.
    pub still: String,
    /// `FluidType.getLightLevel`.
    pub light_level: u8,
    /// `FluidType.isLighterThanAir` (a density of 0 or less).
    pub lighter_than_air: bool,
}

#[derive(Clone, Debug)]
pub struct BakedItemModel {
    /// `QuadCollection.getAll`: unculled quads, then north, south, east,
    /// west, up and down.
    pub quads: Vec<ModelQuad>,
    /// `gui_light: side`.
    pub uses_block_light: bool,
    pub particle: Material,
    /// By [`crate::model::DISPLAY_CONTEXTS`]; `None` is `NO_TRANSFORM`.
    pub transforms: [Option<ItemTransform>; 9],
}

fn context_index(context: DisplayContext) -> Option<usize> {
    Some(match context {
        DisplayContext::None => return None,
        DisplayContext::ThirdPersonLeftHand => 0,
        DisplayContext::ThirdPersonRightHand => 1,
        DisplayContext::FirstPersonLeftHand => 2,
        DisplayContext::FirstPersonRightHand => 3,
        DisplayContext::Head => 4,
        DisplayContext::Gui => 5,
        DisplayContext::Ground => 6,
        DisplayContext::Fixed => 7,
        DisplayContext::OnShelf => 8,
    })
}
impl BakedItemModel {
    /// `ItemTransforms.getTransform`.
    pub fn transform(&self, context: DisplayContext) -> Option<&ItemTransform> {
        context_index(context).and_then(|i| self.transforms[i].as_ref())
    }
    /// `LayerRenderState.applyTransform`: the display transform followed by
    /// the item model's own local transformation.
    pub fn layer_matrix(&self, context: DisplayContext, local: &Mat4) -> Mat4 {
        item_transform_matrix(self.transform(context), context.left_hand()).mul(local)
    }
}
/// `ItemTransform.apply` on an identity pose.
pub fn item_transform_matrix(transform: Option<&ItemTransform>, left_hand: bool) -> Mat4 {
    let Some(t) = transform else {
        return Mat4::IDENTITY.translate([-0.5, -0.5, -0.5]);
    };
    let (tx, ry, rz) = if left_hand {
        (-t.translation[0], -t.rotation[1], -t.rotation[2])
    } else {
        (t.translation[0], t.rotation[1], t.rotation[2])
    };
    let radians = PI / 180.0;
    Mat4::IDENTITY
        .translate([tx, t.translation[1], t.translation[2]])
        .rotate(quaternion_xyz(
            t.rotation[0] * radians,
            ry * radians,
            rz * radians,
        ))
        .scale(t.scale)
        .translate([-0.5, -0.5, -0.5])
}

/// Bake the top geometry of `name` the way `CuboidItemModelWrapper` does.
pub fn bake_item_model(
    library: &ModelLibrary,
    sprites: &dyn SpriteLookup,
    name: &str,
) -> Result<BakedItemModel> {
    let model = library
        .resolve(name)
        .or_else(|_| library.resolve(crate::resolve::MISSING_MODEL))?;
    bake_resolved(&model, sprites)
}

/// Bake an already resolved model, e.g. one from
/// [`ModelLibrary::resolve_with_textures`].
pub fn bake_resolved(
    model: &ResolvedCuboidModel,
    sprites: &dyn SpriteLookup,
) -> Result<BakedItemModel> {
    let known = |sprite: &str| sprite == MISSING_SPRITE || sprites.mask(sprite).is_some();
    let material = |m: &Material| {
        if known(&m.sprite) {
            m.clone()
        } else {
            Material {
                sprite: MISSING_SPRITE.to_owned(),
                force_translucent: m.force_translucent,
            }
        }
    };
    if let Some(container) = &model.fluid_container {
        return bake_fluid_container(model, container, sprites);
    }
    let quads = if model.uses_item_generator() {
        let mut quads = Vec::new();
        for (layer, slot) in ["layer0", "layer1", "layer2", "layer3", "layer4"]
            .into_iter()
            .enumerate()
        {
            let Some(m) = model.material(slot)? else {
                break;
            };
            let m = material(m);
            let mask = if m.sprite == MISSING_SPRITE {
                SpriteMask::opaque(16, 16)
            } else {
                sprites
                    .mask(&m.sprite)
                    .unwrap_or_else(|| SpriteMask::opaque(16, 16))
            };
            quads.extend(extrude_layer(&mask, &m, layer as i32)?);
        }
        quads
    } else {
        let mut quads =
            bake_block_model(model, crate::bake::BlockModelTransform::new([0; 3], false))?;
        for q in &mut quads {
            q.material = material(&q.material);
        }
        // `QuadCollection.Builder.build`: unculled, then each culled direction.
        let mut ordered: Vec<ModelQuad> =
            quads.iter().filter(|q| q.cull.is_none()).cloned().collect();
        for side in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
            Direction::Up,
            Direction::Down,
        ] {
            ordered.extend(quads.iter().filter(|q| q.cull == Some(side)).cloned());
        }
        ordered
    };
    // `resolveParticleMaterial`: a missing `particle` slot is the missing sprite.
    let particle = model.material("particle")?.map_or_else(
        || Material {
            sprite: MISSING_SPRITE.to_owned(),
            force_translucent: false,
        },
        material,
    );
    let transforms = std::array::from_fn(|i| match &model.display.contexts[i] {
        DisplayTransform::Explicit(Some(t)) => Some(t.clone()),
        _ => None,
    });
    Ok(BakedItemModel {
        quads,
        uses_block_light: model.gui_light == GuiLight::Side,
        particle,
        transforms,
    })
}
/// `DynamicFluidContainerModel.bake`: the `base` texture extruded as an
/// item layer (tint index 0), the `fluid` mask's shape filled with the
/// fluid's still texture (tint index 1, the item colour its fluid's tint
/// gives through `DynamicFluidContainerModel.Colors`; full bright for a fluid
/// that gives light), and the `cover` mask's shape over it (tint index 2)
/// with the base texture or, when `cover_is_mask` is false, its own. Every
/// layer draws translucent (NeoForge's `ITEM_UNSORTED_TRANSLUCENT`) and lit
/// from the front (`withUseBlockLight(false)`); a gas turns upside down
/// with `flip_gas`. The fluid and cover layers stand a little out of the
/// base (`FLUID_TRANSFORM`, `COVER_TRANSFORM`: 1.002 and 1.004 along z).
fn bake_fluid_container(
    model: &ResolvedCuboidModel,
    container: &FluidContainer,
    sprites: &dyn SpriteLookup,
) -> Result<BakedItemModel> {
    // The sprite getter: an unknown texture is the missing sprite; every layer is translucent.
    let sprite = |m: &Material| Material {
        sprite: if m.sprite == MISSING_SPRITE || sprites.mask(&m.sprite).is_some() {
            m.sprite.clone()
        } else {
            MISSING_SPRITE.to_owned()
        },
        force_translucent: true,
    };
    let mask_of = |m: &Material| {
        sprites
            .mask(&m.sprite)
            .unwrap_or_else(|| SpriteMask::opaque(16, 16))
    };
    let base = model.material("base")?.map(sprite);
    let fluid_mask = model.material("fluid")?.map(sprite);
    let look = sprites.fluid(&container.fluid);
    let fluid = look.as_ref().map(|l| {
        sprite(&Material {
            sprite: l.still.clone(),
            force_translucent: true,
        })
    });
    let cover = model
        .material("cover")?
        .filter(|_| !container.cover_is_mask || base.is_some())
        .map(sprite);
    let particle = model
        .material("particle")?
        .map(sprite)
        .or_else(|| fluid.clone())
        .or_else(|| base.clone())
        .or_else(|| cover.clone().filter(|_| !container.cover_is_mask))
        .unwrap_or(Material {
            sprite: MISSING_SPRITE.to_owned(),
            force_translucent: false,
        });
    let mut quads = Vec::new();
    if let Some(base) = &base {
        quads.extend(extrude_layer(&mask_of(base), base, 0)?);
    }
    if let (Some(template), Some(fluid), Some(look)) = (&fluid_mask, &fluid, &look) {
        let mut layer = mask_elements(&mask_of(template), fluid, 1)?;
        stand_out(&mut layer, 1.002);
        if container.apply_fluid_luminosity && look.light_level > 0 {
            // QuadTransformers.settingMaxEmissivity.
            for quad in &mut layer {
                quad.light_emission = 15;
            }
        }
        quads.extend(layer);
    }
    if let Some(cover) = &cover {
        let texture = if container.cover_is_mask {
            base.as_ref()
        } else {
            Some(cover)
        };
        if let Some(texture) = texture {
            let mut layer = mask_elements(&mask_of(cover), texture, 2)?;
            stand_out(&mut layer, 1.004);
            quads.extend(layer);
        }
    }
    if container.flip_gas && look.is_some_and(|l| l.lighter_than_air) {
        // `new Transformation(null, new Quaternionf(0, 0, 1, 0), null, null)`: half a turn about z.
        for quad in &mut quads {
            for p in &mut quad.positions {
                (p[0], p[1]) = (1.0 - p[0], 1.0 - p[1]);
            }
            quad.facing = match quad.facing {
                Direction::Up => Direction::Down,
                Direction::Down => Direction::Up,
                Direction::East => Direction::West,
                Direction::West => Direction::East,
                facing => facing,
            };
        }
    }
    let transforms = std::array::from_fn(|i| match &model.display.contexts[i] {
        DisplayTransform::Explicit(Some(t)) => Some(t.clone()),
        _ => None,
    });
    Ok(BakedItemModel {
        quads,
        uses_block_light: false,
        particle,
        transforms,
    })
}

/// `FaceBakery`'s model rotation for a z scale about the block's centre.
fn stand_out(quads: &mut [ModelQuad], scale: f32) {
    for quad in quads {
        for p in &mut quad.positions {
            p[2] = 0.5 + (p[2] - 0.5) * scale;
        }
    }
}

/// `UnbakedGeometryHelper.createUnbakedItemMaskElements` baked with
/// `material` (`bakeElements` with a getter that gives it for every
/// texture): the mask's side faces as an item layer has them (its front and
/// back left out), and over its opaque pixels (in any frame) rectangles
/// 7.5 to 8.5 deep, each with all six faces at their default UVs. A run of
/// opaque pixels grows down while the row below is opaque from its start to
/// the first pixel past it; a run that reaches the right edge makes none.
fn mask_elements(mask: &SpriteMask, material: &Material, layer: i32) -> Result<Vec<ModelQuad>> {
    let mut quads = extrude_layer(mask, material, layer)?;
    quads.drain(..2);
    let (width, height) = (mask.width, mask.height);
    let mut bits = vec![false; width * height];
    for frame in &mask.frames {
        for (bit, transparent) in bits.iter_mut().zip(frame) {
            *bit |= !transparent;
        }
    }
    for y in 0..height {
        let mut start: Option<usize> = None;
        for x in 0..width {
            let opaque = bits[x + y * width];
            if opaque != start.is_none() {
                continue;
            }
            let Some(x_start) = start else {
                start = Some(x);
                continue;
            };
            let mut y_end = y + 1;
            'expand: while y_end < height {
                for x2 in x_start..=x {
                    if !bits[x2 + y_end * width] {
                        break 'expand;
                    }
                }
                y_end += 1;
            }
            for i in x_start..x {
                for j in y..y_end {
                    bits[i + j * width] = false;
                }
            }
            let (w, h) = (width as f32, height as f32);
            let from = [
                (16 * x_start) as f32 / w,
                16.0 - (16 * y_end) as f32 / h,
                7.5,
            ];
            let to = [(16 * x) as f32 / w, 16.0 - (16 * y) as f32 / h, 8.5];
            for direction in Direction::ALL {
                quads.push(bake_flat_face(
                    from,
                    to,
                    default_uv(from, to, direction),
                    direction,
                    material.clone(),
                    layer,
                )?);
            }
            start = None;
        }
    }
    Ok(quads)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Side {
    Up,
    Down,
    Left,
    Right,
}
impl Side {
    fn step(self) -> (i64, i64) {
        match self {
            Self::Up => (0, 1),
            Self::Down => (0, -1),
            Self::Left => (1, 0),
            Self::Right => (-1, 0),
        }
    }
    fn direction(self) -> Direction {
        match self {
            Self::Up => Direction::Up,
            Self::Down => Direction::Down,
            Self::Left => Direction::East,
            Self::Right => Direction::West,
        }
    }
}

/// `ItemModelGenerator.bakeExtrudedSprite` for one layer: the front and back
/// faces and one thin face per opaque/transparent pixel edge. The native
/// client collects the sides in a hash set, so their order is unspecified;
/// they are emitted sorted by pixel here.
fn extrude_layer(mask: &SpriteMask, material: &Material, layer: i32) -> Result<Vec<ModelQuad>> {
    let from = [0.0, 0.0, 7.5];
    let to = [16.0, 16.0, 8.5];
    let mut quads = vec![
        bake_flat_face(
            from,
            to,
            [0.0, 0.0, 16.0, 16.0],
            Direction::South,
            material.clone(),
            layer,
        )?,
        bake_flat_face(
            from,
            to,
            [16.0, 0.0, 0.0, 16.0],
            Direction::North,
            material.clone(),
            layer,
        )?,
    ];
    let (width, height) = (mask.width as i64, mask.height as i64);
    let mut faces = BTreeSet::new();
    for frame in 0..mask.frames.len() {
        for y in 0..height {
            for x in 0..width {
                if mask.transparent(frame, x, y) {
                    continue;
                }
                for side in [Side::Up, Side::Down, Side::Left, Side::Right] {
                    let (sx, sy) = side.step();
                    if mask.transparent(frame, x - sx, y - sy) {
                        faces.insert((y, x, side));
                    }
                }
            }
        }
    }
    let x_scale = 16.0 / mask.width as f32;
    let y_scale = 16.0 / mask.height as f32;
    for (y, x, side) in faces {
        let (x, y) = (x as f32, y as f32);
        let u0 = x + 0.1;
        let u1 = x + 1.0 - 0.1;
        let (v0, v1) = if matches!(side, Side::Up | Side::Down) {
            (y + 0.1, y + 1.0 - 0.1)
        } else {
            (y + 1.0 - 0.1, y + 0.1)
        };
        let (mut start_x, mut start_y, mut end_x, mut end_y) = (x, y, x, y);
        match side {
            Side::Up => end_x += 1.0,
            Side::Down => {
                end_x += 1.0;
                start_y += 1.0;
                end_y += 1.0;
            }
            Side::Left => end_y += 1.0,
            Side::Right => {
                start_x += 1.0;
                end_x += 1.0;
                end_y += 1.0;
            }
        }
        start_x *= x_scale;
        end_x *= x_scale;
        start_y *= y_scale;
        end_y *= y_scale;
        start_y = 16.0 - start_y;
        end_y = 16.0 - end_y;
        let (from, to) = match side {
            Side::Up => ([start_x, start_y, 7.5], [end_x, start_y, 8.5]),
            Side::Down => ([start_x, end_y, 7.5], [end_x, end_y, 8.5]),
            Side::Left => ([start_x, start_y, 7.5], [start_x, end_y, 8.5]),
            Side::Right => ([end_x, start_y, 7.5], [end_x, end_y, 8.5]),
        };
        quads.push(bake_flat_face(
            from,
            to,
            [u0 * x_scale, v0 * y_scale, u1 * x_scale, v1 * y_scale],
            side.direction(),
            material.clone(),
            layer,
        )?);
    }
    Ok(quads)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CuboidModel, TextureSlot};
    use std::collections::BTreeMap;

    /// The stick: 2x2 with one transparent pixel. `item/cit`: one opaque pixel, so a bake
    /// shows whose mask it extruded.
    struct Sprites;
    impl SpriteLookup for Sprites {
        fn mask(&self, sprite: &str) -> Option<SpriteMask> {
            match sprite {
                "minecraft:item/stick" => Some(SpriteMask {
                    width: 2,
                    height: 2,
                    frames: vec![vec![true, false, false, false]],
                }),
                "horizon_cit:item/cit" => Some(SpriteMask {
                    width: 1,
                    height: 1,
                    frames: vec![vec![false]],
                }),
                _ => None,
            }
        }
    }
    fn library() -> ModelLibrary {
        ModelLibrary::with_item_generator(BTreeMap::from(
            [
                ("item/generated", r#"{"parent":"builtin/generated","display":{"gui":{"scale":[2,2,2]}}}"#),
                ("item/stick", r#"{"parent":"item/generated","textures":{"layer0":"item/stick"}}"#),
                ("horizon:loop", r#"{"parent":"horizon:loop2"}"#),
                ("horizon:loop2", r#"{"parent":"horizon:loop"}"#),
                ("block/slab", r##"{"textures":{"particle":"#side","side":"block/absent"},"elements":[{"from":[0,0,0],"to":[16,8,16],"faces":{"up":{"texture":"#side"},"down":{"texture":"#side","cullface":"down"}}}]}"##),
            ]
            .map(|(name, json)| (name.to_owned(), CuboidModel::from_json(json).unwrap())),
        ))
        .unwrap()
    }
    #[test]
    fn baking_by_name_falls_back_to_the_missing_model() {
        let library = library();
        // Front, back, and the 8 unit edges around the L of 3 opaque pixels.
        let stick = bake_item_model(&library, &Sprites, "item/stick").unwrap();
        assert_eq!(stick.quads.len(), 10);
        // An unknown sprite bakes as the missing sprite.
        let slab = bake_item_model(&library, &Sprites, "block/slab").unwrap();
        assert_eq!(slab.quads.len(), 2);
        assert_eq!(slab.particle.sprite, MISSING_SPRITE);
        // A cyclic chain, an error to `resolve`, bakes as the missing cube.
        let cyclic = bake_item_model(&library, &Sprites, "horizon:loop").unwrap();
        assert_eq!(cyclic.quads.len(), 6);
        assert!(
            cyclic
                .quads
                .iter()
                .all(|q| q.material.sprite == MISSING_SPRITE)
        );
    }
    #[test]
    fn replaced_layer_bakes_with_its_own_sprite() {
        let library = library();
        let child = BTreeMap::from([(
            "layer0".to_owned(),
            TextureSlot::Material(Material {
                sprite: "horizon_cit:item/cit".to_owned(),
                force_translucent: false,
            }),
        )]);
        let model = library.resolve_with_textures("item/stick", &child).unwrap();
        let baked = bake_resolved(&model, &Sprites).unwrap();
        // The CIT sprite's own mask, one opaque pixel: front, back and 4 edges.
        assert_eq!(baked.quads.len(), 6);
        assert!(
            baked
                .quads
                .iter()
                .all(|q| q.material.sprite == "horizon_cit:item/cit")
        );
        assert_eq!(baked.particle.sprite, "horizon_cit:item/cit");
        assert_eq!(baked.transforms[5].as_ref().unwrap().scale, [2.0; 3]);
    }

    /// A NeoForge bucket: `neoforge:item/bucket`'s base and fluid mask, the
    /// leaf naming its fluid. The base is a 4x4 opaque square; the mask's
    /// fluid is the 2x2 in its middle, and a pixel on its right edge.
    struct Buckets;
    impl SpriteLookup for Buckets {
        fn mask(&self, sprite: &str) -> Option<SpriteMask> {
            let (width, height) = (4, 4);
            let frame = match sprite {
                "minecraft:item/bucket" | "test:block/brine_still" => vec![false; 16],
                #[rustfmt::skip]
                "neoforge:item/mask/bucket_fluid" => vec![
                    true, true,  true,  true,
                    true, false, false, true,
                    true, false, false, false,
                    true, true,  true,  true,
                ],
                _ => return None,
            };
            Some(SpriteMask {
                width,
                height,
                frames: vec![frame],
            })
        }
        fn fluid(&self, fluid: &str) -> Option<FluidLook> {
            (fluid == "test:brine").then(|| FluidLook {
                still: "test:block/brine_still".into(),
                light_level: 0,
                lighter_than_air: false,
            })
        }
    }
    fn buckets() -> ModelLibrary {
        ModelLibrary::with_item_generator(BTreeMap::from(
            [
                ("neoforge:item/default", r#"{"gui_light":"front","display":{"ground":{"scale":[0.5,0.5,0.5]}}}"#),
                ("neoforge:item/bucket", r#"{"parent":"neoforge:item/default","textures":{"base":"item/bucket","fluid":"neoforge:item/mask/bucket_fluid"}}"#),
                ("test:item/brine_bucket", r#"{"parent":"neoforge:item/bucket","loader":"neoforge:fluid_container","fluid":"test:brine"}"#),
                ("test:item/gone_bucket", r#"{"parent":"neoforge:item/bucket","loader":"neoforge:fluid_container","fluid":"test:gone"}"#),
            ]
            .map(|(name, json)| (name.to_owned(), CuboidModel::from_json(json).unwrap())),
        ))
        .unwrap()
    }
    #[test]
    fn a_fluid_container_masks_its_fluid_into_the_bucket() {
        let library = buckets();
        let bucket = bake_item_model(&library, &Buckets, "test:item/brine_bucket").unwrap();
        let layer = |tint: i32| -> Vec<&ModelQuad> {
            bucket
                .quads
                .iter()
                .filter(|q| q.tint_index == tint)
                .collect()
        };
        // The base as an item layer: front, back and the 16 edges of the square.
        let base = layer(0);
        assert_eq!(base.len(), 18);
        assert!(
            base.iter()
                .all(|q| q.material.sprite == "minecraft:item/bucket")
        );
        // The fluid layer: the mask's 8 edges and 6 faces of the 2x2 run (from 4,4 to 12,12),
        // none for the pixel on the right edge; the fluid's still texture, z scaled by 1.002.
        let fluid = layer(1);
        assert_eq!(
            fluid.len(),
            10 + 6,
            "the edges of the 2x2 and its edge pixel, and one box"
        );
        assert!(
            fluid
                .iter()
                .all(|q| q.material.sprite == "test:block/brine_still")
        );
        assert!(fluid.iter().all(|q| q.material.force_translucent));
        let front = fluid.iter().find(|q| q.facing == Direction::South).unwrap();
        let (xs, ys): (Vec<f32>, Vec<f32>) = front.positions.iter().map(|p| (p[0], p[1])).unzip();
        assert_eq!(xs.iter().cloned().fold(f32::MAX, f32::min), 0.25);
        assert_eq!(xs.iter().cloned().fold(f32::MIN, f32::max), 0.75);
        assert_eq!(ys.iter().cloned().fold(f32::MAX, f32::min), 0.25);
        assert_eq!(ys.iter().cloned().fold(f32::MIN, f32::max), 0.75);
        let z = 0.5 + (8.5 / 16.0 - 0.5) * 1.002;
        assert!(
            front.positions.iter().all(|p| (p[2] - z).abs() < 1e-6),
            "{:?}",
            front.positions
        );
        // Front lit, the fluid's texture as the particle, the parent's display.
        assert!(!bucket.uses_block_light);
        assert_eq!(bucket.particle.sprite, "test:block/brine_still");
        assert_eq!(bucket.transforms[6].as_ref().unwrap().scale, [0.5; 3]);
        // A fluid the game lacks: the bucket alone.
        let gone = bake_item_model(&library, &Buckets, "test:item/gone_bucket").unwrap();
        assert_eq!(gone.quads.len(), 18);
        assert_eq!(gone.particle.sprite, "minecraft:item/bucket");
    }
}
