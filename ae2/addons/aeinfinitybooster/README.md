# AEInfinityBooster 2.1.0

[AEInfinityBooster](https://github.com/Hexeption/AEInfinityBooster) by Hexeption, as All the Mods 10 8.4 ships it
(`aeinfinitybooster-neoforge-2.1.0+mc1.21.1.jar`; 8.3 had 1.0.0.58). Code LGPL-3.0 (tag `v2.1.0`, `LICENSE`), so the
port's derived files carry the same terms. The licence of the assets is not stated; they are generated locally from
the player's jar and never committed. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content
in [content.json](content.json).

Feature `ae2-aeinfinitybooster`; content `content/aeinfinitybooster` from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts aeinfinitybooster`; settings from `config/aeinfinitybooster-common.toml`
(`[mods.aeinfinitybooster]` in `pack/pack.toml`). All the Mods 10 ships no such file, so the mod's defaults hold:
`infinityCardDrain` 50, `dimensionCardDrain` 100, `chunkLoadingEnabled` true.

## Slice 1: the Infinity Range Booster, same dimension

| AEInfinityBooster 2.1.0 | Port |
|---|---|
| `infinity_card`: an `Item` with `durability(0)`, so it stacks to one | a declared item, stack size 1 |
| `MixinWirelessBlockEntity` (`AccessPointCardFilter`), `MixinRestrictedInputSlot`: the access point's booster slot takes AE2's Wireless Booster and the cards | an access point card row in the definitions registry (`ae2x/aeinfinitybooster.rs`); the slot asks `Defs::access_point_accepts` |
| `MixinWirelessTerminalMenuHost.getAccessPointSignal`: an active access point holding the card answers a terminal in its dimension with a squared distance of 256 however far it is; AE2 still connects through the nearest answer | AE2's wireless reach (`access_point_signal`) takes the card's signal from the row: the terminal reaches the grid from anywhere in the dimension and its battery pays AE2's drain for 16 blocks (16 AE/t at ATM10's `wirelessTerminalDrainMultiplier` 1) |
| `MixinWirelessMenu`, `MixinWirelessAccessPointScreen`: the access point's screen shows the range as ∞ and the drain as `infinityCardDrain` (50 AE/t) | the screen data and text of the access point |
| `WirelessAccessPointBlockEntity.updatePower` is untouched: `getBoosters` counts whatever is in the slot, so the grid pays AE2's drain for one booster, `wirelessBaseCost` + `wirelessCostMultiplier` = 9 AE/t in ATM10 | the grid's idle draw counts the card as one booster; `/ae2 power` shows 9 AE/t, not 50 |
| `MixinWirelessBlockEntity`/`MixinWirelessBlockEntityRemove` with `chunkLoadingEnabled`: `onReady` forces the access point's chunk and claims it, `saveChanges` forces it when a card goes in (its own only if nothing else had forced it) and releases its own when the card comes out, `setRemoved` releases its own | `game/ae2x/aeinfinitybooster/`: the same states on the access points of the loaded world, once a second, in the vanilla forced-chunk set `/forceload` uses |
| `infinity_card` recipe | left out: All the Mods 10's KubeJS removes it and adds its own (`allthemods:aeinfinitybooster/infinity_card`), which comes with the pack's final data, not from this jar |

Differences, known:

- The chunk is checked once a second instead of on each inventory change, block entity load and removal.
- The 50 AE/t on the screen is what the mod shows; neither the mod nor the port takes it from the grid.

Tests: `horizon-client-lib` (the config, the card row) and `horizon-client` (the chunk states), and the world test
`aeinfinitybooster-range`: a wireless terminal 10,000 blocks from its access point opens with the card and is refused
without it, the screen's ∞ and 50 AE/t, the grid's 9 AE/t in `/ae2 power`, the chunk kept and let go.

## Slice 2: the Dimension Card

| AEInfinityBooster 2.1.0 | Port |
|---|---|
| `dimension_card`: an `Item` with `durability(0)`, so it stacks to one | a declared item, stack size 1; its recipe from the jar (four Infinity cards, four nether stars, an eye of ender), which All the Mods 10 keeps |
| `AccessPointCardFilter`, `MixinRestrictedInputSlot`: the booster slot takes it | the second access point card row |
| `MixinWirelessTerminalMenuHost.getAccessPointSignal`: an active access point holding the card answers a terminal in any dimension with a squared distance of 1024; AE2's `updateConnectedAccessPoint` asks every access point of the linked grid, whatever its level, and `getLinkedGrid` resolves the link in its own dimension | AE2's wireless reach asks the access points of the linked grid in every loaded level: one in another level answers only with a card that is not for its dimension alone (`same_dimension`); the terminal's battery pays AE2's drain for 32 blocks (32 AE/t at ATM10's `wirelessTerminalDrainMultiplier` 1); the menu's anchor in the other level is the access point's address |
| `MixinWirelessMenu`: the screen shows ∞ and `dimensionCardDrain` (100 AE/t) | the row's shown drain |
| `updatePower` untouched: the grid pays AE2's drain for one booster, 9 AE/t in ATM10 | the same |
| chunk loading as with the Infinity card | the same (the row's `chunk_loading`); the forced chunk keeps the access point's level ticking while the player is in another one |

Differences, known:

- The card has no enchantment glint (`isFoil`) and no tooltip (`dimension_card.tooltip.chunk_loading`).
- The port checks the card's chunk once a second, in a level that ticks: a card put in while `/forceload` holds the
  chunk does not claim it (as in the mod), but if that `/forceload` is then removed while a player keeps the level
  ticking, the next check forces the chunk as the card's own (the mod leaves it unforced until the next inventory
  change).

Tests: `horizon-client-lib` (the card's row) and `horizon-client` (an access point of another level answers only with
the Dimension Card, with its signal however near), and the world test `ae2-crossdim`: the card in the Overworld access
point shows ∞ and 100 AE/t, the grid's idle draw rises by one booster's 1 AE/t, the chunk is forced by the card; from
the Nether a terminal without AE2WTLib's bridge card opens through it, lists the Overworld network and drains 32 AE/t;
taken out, the card lets the chunk go.

## Later (`.build/ae2-addons-plan.md` §5.6)

- None planned: both cards are in. The Infinity card's recipe comes with the pack's final data (All the Mods 10's
  KubeJS).
