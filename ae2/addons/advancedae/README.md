# Advanced AE 1.6.12-1.21.1

[Advanced AE](https://github.com/pedroksl/AdvancedAE) by pedroksl, as All the Mods 10 ships it. Code
LGPL-3.0 (`LICENSE.md`, here [ae2/LICENSES/AdvancedAE-LGPL-3.0.txt](../../LICENSES/AdvancedAE-LGPL-3.0.txt)),
so the port's derived files carry the same terms; the port is written from the mod's behaviour and
source at commit `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a` (branch `forge/1.21.1`, which sets
`mod_version=1.6.12-1.21.1`). The tag `1.6.12-1.21.1-neoforge` points at `779c9c1`, a later port to
Minecraft 26.1.2 (`mod_version=26.1.6`), so the port does not follow the tag. Models by Jm³ and
Sea_Kerman; the assets (licence not stated) are generated locally from the player's jar and never
committed. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in
[content.json](content.json).

The jar bundles AE2AddonLib 1.0.3, whose `mods.toml` says LGPL-3.0 and whose repository says GPL-3.0.
Nothing is derived from it: of it the port knows only the saved form of `NullableDirection` (the
direction's `Direction.name()`, or `null`), which Advanced AE's own `EncodedAdvProcessingPattern` writes.
The bundled ae2wtlib_api (MIT) is not needed by this slice.

Feature `ae2-advancedae`; content `content/advanced_ae` from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts advancedae`; `[mods.advanced_ae]` in
`pack/pack.toml`. ATM10's `config/advanced_ae-common.toml` holds only the Quantum Computer's settings,
a later slice's; its KubeJS changes no recipe of this slice.

## What works (slice 1)

| Advanced AE | Port |
|---|---|
| `advanced_ae:adv_processing_pattern` (`PatternDetailsHelper.encodedPatternItemBuilder(AdvProcessingPattern::new)`), its component `advanced_ae:encoded_adv_processing_pattern`: AE2's `sparseInputs` and `sparseOutputs`, and `directionMap`, a `NullableDirection` per input | a processing pattern item of its own component in the definitions registry (`Defs::add_processing_pattern`), so every processing pattern decoder, the provider and Pattern Access slots, the Memory Card's blanks and the crafting plans read it as AE2's; the component in the command codec (`ae2_components`) and on the wire |
| `AdvProcessingPattern.getDirectionMap`: one direction per input key, a key's last | `game/ae2x/advancedae`: `direction_map`; an entry that is no direction reads as none (the codec's fault tolerance) |
| `advanced_ae:small_adv_pattern_provider` (`SmallAdvPatternProviderBlock`, `AEBaseBlock.metalProps`, `push_direction`, `connection_state`), `AdvPatternProviderLogic(node, host, 9)` | a machine variant of AE2's Pattern Provider (nine patterns, nine return slots, no cards): blocking mode, the crafting lock, the Pattern Access Terminal toggle, priority, `push_direction` and the wrench as AE2's |
| `AdvPatternProviderLogic.pushPattern`: an Advanced pattern is pushed with `pushInputsDirectionally` first, each input into the face of the target its direction names (none: the face toward the provider), and when one input finds no room there, as AE2's, all to one face | the provider push (`ae2_autocrafting`): the target is chosen and the inputs sent by the faces of `directional_faces`; AE2's Pattern Provider pushes the same pattern as any processing pattern |
| `sendStacksOut` with the push's `directionMap`: what a face did not take waits and goes to its own face | the send list keeps the faces (`advanced_ae_send_faces`) beside AE2's send face until it is empty |
| `SmallAdvPatternProviderScreen` with `screens/small_adv_pattern_provider.json` | `game/ae2x/advancedae/screens.rs`: AE2's provider layout over the mod's style |
| `AdvPatternEncoderMenu`: an AE2 processing pattern in the input slot gives an Advanced one with no direction (`decodeProcessingPattern`), an Advanced one is copied, `update` sets a key's direction | `encoder_output`, `with_direction`; `/ae2 advae show|encode|direction <slot> ...` applies them to a pattern in the player's inventory |
| `advanced_ae:adv_pattern_encoder` (`AdvPatternEncoderItem`, stacks to 1) | the item, its model and recipe `advpartenc` |
| recipes `smalladvpatpro` and `advpartenc`, loot table, pickaxe tag | from the jar; `smalladvpatpro2` and `smalladvpatpropart` name the provider part, a later slice's |

## Not yet

- The Advanced Pattern Encoder's screen (`AdvPatternEncoderScreen`: the input and output slots, a scrolling list
  of the inputs with a button per direction, its hotkey and Curios slot): until then the item does nothing when
  used, and `/ae2 advae` makes the edits.
- `FILTERED_IMPORT` (`AdvPatternProviderReturnInventory`: the return slots take only what the patterns make) and
  its toolbar button.
- The tooltip's direction letters (`MixinEncodedPatternItem`), and the encoding terminal's re-encoding of an
  Advanced pattern with its directions (`MixinPatternEncodingTermMenu`): re-encoded, it becomes AE2's pattern.
- A target with an ME storage capability (`AdvPatternProviderTargetCache.find`: an interface) takes every input
  whatever its face; the port asks the face.
- The Extended (36) provider, the part forms and the upgrade items (slice 4), the Reaction Chamber (2), the
  Quantum Computer (3), the buses and Throughput Monitor (5), the Quantum Crafter (6), the Quantum Armor (7).

## Tests

`horizon-client-lib` (the variant, the registry row, `NullableDirection` names), `horizon-command` (the component's
codec), `horizon-client` (`game/ae2x/advancedae`: the encoder's edits, the saved direction list, which provider pushes
directionally; the screen layout on the window's entries), and the world test `advancedae-provider` (the encoder's
edits; the provider's screen; raw iron through a furnace's top and coal through its side; AE2's provider pushing the
same pattern to one face; a restart):

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts advancedae-provider
```
