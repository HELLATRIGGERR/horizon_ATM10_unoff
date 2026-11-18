# Occultism on this fork

A port of **Occultism 1.224.6** (`occultism-1.21.1-neoforge-1.224.6.jar`, the version All the Mods 10 ships) to this
fork of Horizon. It is the native bundle in this folder (`horizon-mixin.toml`, `hooks/`, `runtime/`) and a crate of
pure rules (`occultism-core`). `pack/scripts/occultism.ts` builds and tests them; `docs/atm10-port.md` ("Occultism")
says where the work stands.

## Credits and licence

Occultism is by **klikli-dev** and its contributors: <https://github.com/klikli-dev/occultism>. Its code is MIT
(`LICENSE` in the repository at the tag `release/v1.21.1-1.224.6`, commit `9c5508c35f`, and `mod_license=MIT` in its
`gradle.properties`). `LICENSE-occultism` in this folder is that licence with its copyright notice.

The code here is translated from Occultism's own source at that tag: `SpiritFireBlock` and `SpiritFireRecipe`,
`PlayerEventHandler.spiritFire`, `SacrificialBowlBlock`, `GoldenSacrificialBowlBlockEntity`, `Ritual`, `CraftRitual`
and `RitualRecipe`. It is MIT like the original, under the notice in `LICENSE-occultism`.

Pentacles are Modonomicon multiblocks (Modonomicon is `MIT AND CC-BY-4.0`). Their format is read from Occultism's data
files and matched by our own code; no Modonomicon code is translated. Occultism's book, which needs Modonomicon, is not
ported. Its mobs use GeckoLib (MIT) for models and SmartBrainLib (MPL-2.0) for their brains; neither is translated
here.

No file of Occultism is committed: no code, textures, models, sounds or lang files. Its blocks, items, recipes,
pentacles, loot tables and assets come from the carry of the player's own jar (`horizon-port-pack carry`), loaded with
`--dev-mod`. This bundle adds their behaviour.
