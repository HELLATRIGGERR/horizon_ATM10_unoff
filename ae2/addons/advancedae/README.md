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
| `advanced_ae:adv_processing_pattern` (`PatternDetailsHelper.encodedPatternItemBuilder(AdvProcessingPattern::new)`, which stacks to 1), its component `advanced_ae:encoded_adv_processing_pattern`: AE2's `sparseInputs` and `sparseOutputs`, and `directionMap`, a `NullableDirection` per input | a processing pattern item of its own component in the definitions registry (`Defs::add_processing_pattern`), so every processing pattern decoder, the provider and Pattern Access slots, the Memory Card's blanks and the crafting plans read it as AE2's; the component in the command codec (`ae2_components`) and on the wire |
| `AdvProcessingPattern.getDirectionMap`: one direction per input key, a key's last | `game/ae2x/advancedae`: `direction_map`; an entry that is no direction reads as none (the codec's fault tolerance) |
| `advanced_ae:small_adv_pattern_provider` (`SmallAdvPatternProviderBlock`, `AEBaseBlock.metalProps`, `push_direction`, `connection_state`), `AdvPatternProviderLogic(node, host, 9)` | a machine variant of AE2's Pattern Provider (nine patterns, nine return slots, no cards): blocking mode, the crafting lock, the Pattern Access Terminal toggle, priority, `push_direction` and the wrench as AE2's |
| `AdvPatternProviderLogic.pushPattern`: an Advanced pattern is pushed with `pushInputsDirectionally` first, each input into the face of the target its direction names (none: the face toward the provider), and when one input finds no room there, as AE2's, all to one face | the provider push (`ae2_autocrafting`): the target is chosen and the inputs sent by the faces of `directional_faces`; AE2's Pattern Provider pushes the same pattern as any processing pattern |
| `sendStacksOut` with the push's `directionMap`: what a face did not take waits and goes to its own face | the send list keeps the faces (`advanced_ae_send_faces`) beside AE2's send face until it is empty |
| `SmallAdvPatternProviderScreen` with `screens/small_adv_pattern_provider.json` | `game/ae2x/advancedae/screens.rs`: AE2's provider layout over the mod's style |
| `AdvPatternEncoderMenu`: an AE2 processing pattern in the input slot gives an Advanced one with no direction (`decodeProcessingPattern`), an Advanced one is copied, `update` sets a key's direction | `encoder_output`, `with_direction`; `/ae2 advae show|encode|direction <slot> ...` applies them to a pattern in the player's inventory, for tests |
| `advanced_ae:adv_pattern_encoder` (`AdvPatternEncoderItem`, stacks to 1) | the item, its model and recipe `advpartenc` |
| recipes `smalladvpatpro` and `advpartenc`, loot table, pickaxe tag | from the jar; `smalladvpatpro2` and `smalladvpatpropart` name the provider part, a later slice's |

## What works (slice 1b)

| Advanced AE | Port |
|---|---|
| `AdvPatternEncoderItem.use`: the encoder's menu (`AdvPatternEncoderHost`, `AdvPatternEncoderMenu`): an input slot of encoded patterns and an output slot over the player's inventory, the encoder's own slot locked; a pattern put in decodes into the output (`decodeInputPattern`, also as the menu opens), taking the output takes the input (`onChangeInventory`); the two slots stay in the item | `game/ae2x/advancedae/encoder.rs`: a 9 x 3 window of the two slots; the slots in the item as AE2's `AppEngInternalInventory` list `inOutInventory`, in its `minecraft:custom_data` (the mod keeps them in AE2AddonLib's NBT component, which the port does not read) |
| `AdvPatternEncoderScreen` with `screens/adv_pattern_encoder.json`: three rows of the inputs from the scroll, the row's slot and input, seven `DirectionInputButton`s (any, north, east, south, west, up, down; the row's face highlighted; a tooltip each), the small scrollbar (the wheel and a press past the handle move two rows); `AdvPatternEncoderChangeDirectionPacket` | `encoder_screen.rs` over the window, the mod's background and button textures; a button is a menu button (`BUTTONS + 7 * row + column`); the list is read from the output slot, as the menu's `AdvPatternEncoderPacket` sends it |
| `PATTERN_ENCODER_HOTKEY` (`key.advanced_ae.pattern_encoder_action`, unbound, category `key.advanced_ae.category`): the first encoder in the inventory opens; on the open screen the hotkey closes it (`isCloseHotkey`) | the key mapping in Controls (`horizon-client-input`, feature `ae2-advancedae`); the addons' hotkey id `advanced_ae:pattern_encoder_action`, answered on the server; the close hotkey goes no further |
| `AAESettings.FILTERED_IMPORT` and its `AAESettingToggleButton` (icons `FILTERED_IMPORT_ON`/`OFF` of the mod's `states.png`); `AdvPatternProviderReturnInventory`: while on, the return slots take only what the provider's patterns make (`getOutputCache`, `getTrackedCrafts`) | the provider's fourth toolbar button (menu entry 21, config `filtered_import`, `NO`/`YES`); the filter on the return inventory's inserts (hoppers, machines) and the menu's slots (a click, a swap, a shift-click); AE2's icon sheet takes an addon's (`icon::sheet`) |
| `MixinEncodedPatternItem`: an Advanced pattern's tooltip, AE2's lines (`Produces:`, `with:`, `and`) with each input's direction letter, `(U)` | `tooltip` in `game/ae2x/advancedae`, the item's hover lines; `/ae2 advae show` gives it too |
| `MixinPatternEncodingTermMenu`: encoding a processing pattern while the encoded slot holds an Advanced one keeps it Advanced, each input key it had keeping its direction | `reencoded`, in AE2's Pattern Encoding Terminal and AE2WTLib's wireless one |

## Not yet

- The encoder's Curios slot (the port has no Curios).
- With Filtered Import on, the mod's `outputCache` is filled only while the provider is on a grid; the port's filter
  reads the patterns whatever the grid. A pattern that is no processing pattern, put in the encoder's input after a
  processing one, leaves the old output in the mod (and taking it takes the new input); the port empties the output.
- A target with an ME storage capability (`AdvPatternProviderTargetCache.find`: an interface) takes every input
  whatever its face; the port asks the face.
- The Extended (36) provider, the part forms and the upgrade items (slice 4), the Reaction Chamber (2), the
  Quantum Computer (3), the buses and Throughput Monitor (5), the Quantum Crafter (6), the Quantum Armor (7).

## Tests

`horizon-client-lib` (the variant, the registry row, `NullableDirection` names), `horizon-command` (the component's
codec), `horizon-client` (`game/ae2x/advancedae`: the encoder's edits, the saved direction list, which provider pushes
directionally, the tooltip, re-encoding, Filtered Import; the encoder menu's slots and buttons and the item's
inventory; the screen's buttons; the provider's layout on the window's entries), and the world tests
`advancedae-provider` (the encoder's edits; the provider's screen; raw iron through a furnace's top and coal through
its side; AE2's provider pushing the same pattern to one face; a restart) and `advancedae-encoder` (the encoder's screen
used and by its hotkey, its rows and buttons; the tooltip; Filtered Import with a hopper and a shift-click; re-encoding
in AE2's terminal; a restart):

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts advancedae-provider advancedae-encoder
```
