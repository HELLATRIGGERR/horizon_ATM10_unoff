# Source and content notices

This independent project builds on [Horizon](https://github.com/ludovic111/horizon)
and [Applied Energistics 2](https://github.com/AppliedEnergistics/Applied-Energistics-2).
Horizon copyright and third-party notices are preserved in
[LICENSES/Horizon-MIT.txt](LICENSES/Horizon-MIT.txt) and
[LICENSES/Horizon-THIRD-PARTY-NOTICES.md](LICENSES/Horizon-THIRD-PARTY-NOTICES.md);
the original license files for vendored libraries, fonts, and icons remain in
the engine tree. AE2's upstream notices are retained in
[LICENSES/AE2-NOTICE.txt](LICENSES/AE2-NOTICE.txt).

The generator uses AE2 commit `db17504a86128fdf3dae31f5fb7a112a646e0b93`.
AE2 textures and models are credited to © 2020 Ridanisaurus Rid and
© 2013–2020 AlgorithmX2 and contributors. Its upstream README identifies them as
[CC BY-NC-SA 3.0](LICENSES/CC-BY-NC-SA-3.0.txt), and text and translations as
[CC0 1.0](LICENSES/CC0-1.0.txt). It also credits the Guidebook click sound to
[EminYILDIRIM](https://freesound.org/people/EminYILDIRIM/sounds/536108/)
under [CC BY 4.0](LICENSES/CC-BY-4.0.txt). These upstream assets are generated
locally from the checkout, not included in the source export; the upstream
notices and asset terms still apply to the generated pack. Screenshots in this
repository depict rendered AE2 and Minecraft content; the depicted assets
retain their respective copyright and license terms.

Conventional tags are generated locally from the official
[NeoForge 21.1.259 universal JAR](https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.259/),
whose project uses [LGPL 2.1](LICENSES/NeoForge-LGPL-2.1.txt). The Guide compiler
uses the separately installed `marked` and `yaml` packages; their package
licenses accompany the local dependency installation. Minecraft resources are
prepared from the player's own installation and are not distributed here.

AE2 addons from All the Mods 10 are listed in [addons/](addons/). Code derived
from [MEGA Cells](https://github.com/62832/MEGACells) 4.11.0 (tag `4.11.0`,
commit `f46ab8f9a2c14f0de3e09241241b49fb2d06a81a`) is under its
[LGPL 3.0](LICENSES/AE2-LGPL-3.0.txt); the files are listed in
[addons/megacells/SOURCE_MANIFEST.json](addons/megacells/SOURCE_MANIFEST.json).
MEGA Cells' assets, by its README under
[CC BY-NC-SA 3.0](LICENSES/CC-BY-NC-SA-3.0.txt), are generated locally from the
player's own jar by `scripts/prepare-addon.ts` and are not distributed here.
Code derived from [ME Requester](https://github.com/AlmostReliable/merequester)
1.21.1-1.5.0 by AlmostReliable (tag `v1.21.1-neoforge-1.5.0`, commit
`57554ba7e126b38e4a38f19c37cf9916dfef48bc`) is under its
[LGPL 3.0](LICENSES/AE2-LGPL-3.0.txt); the files are listed in
[addons/merequester/SOURCE_MANIFEST.json](addons/merequester/SOURCE_MANIFEST.json).
Its assets, whose licence the mod does not state, are likewise generated
locally from the player's jar and not distributed here.
Code derived from [ExtendedAE](https://github.com/GlodBlock/ExtendedAE) 1.21-2.2.42
by GlodBlock (commit `1f86f927552f02451236465c0c96a12a3de8f66c` on branch
`1.21.1-neoforge`, which sets that version; it has no tag) is under its
[LGPL 3.0](LICENSES/AE2-LGPL-3.0.txt); the files are listed in
[addons/extendedae/SOURCE_MANIFEST.json](addons/extendedae/SOURCE_MANIFEST.json).
ExtendedAE's assets (models and textures by Sea_Kerman and Ridanisaurus, licence not
stated) are generated locally from the player's own jar by `scripts/prepare-addon.ts`
and are not distributed here.
Code derived from [AE2WTLib](https://github.com/Mari023/AE2WirelessTerminalLibrary)
19.5.1 by Mari023 (tag `19.5.1`, commit `49f70f0627b40762cfbd33732834c7271d5ec17d`)
is under its [MIT licence](LICENSES/AE2WTLib-MIT.txt), combined in the same files
with AE2's LGPL 3.0 code; the files are listed in
[addons/ae2wtlib/SOURCE_MANIFEST.json](addons/ae2wtlib/SOURCE_MANIFEST.json).
AE2WTLib's assets, by its README under
[CC BY-NC-SA 3.0](LICENSES/CC-BY-NC-SA-3.0.txt), are generated locally from the
player's own jar by `scripts/prepare-addon.ts` and are not distributed here.
Code derived from [AEInfinityBooster](https://github.com/Hexeption/AEInfinityBooster)
2.1.0 by Hexeption (tag `v2.1.0`, commit `c46ff3fc124586ae6f91a75f8bccd05d0476547b`)
is under its [LGPL 3.0](LICENSES/AE2-LGPL-3.0.txt); the files are listed in
[addons/aeinfinitybooster/SOURCE_MANIFEST.json](addons/aeinfinitybooster/SOURCE_MANIFEST.json).
Its assets, whose licence the mod does not state, are likewise generated
locally from the player's jar and not distributed here.
Code derived from [AE2 Import Export Card](https://github.com/starforcraft/AE2-Import-Export-Card)
1.21.1-1.9.1 by Ultramega (branch `1.21.1`, commit `9187cf64095271d46ccc8a238be34223f19b250e`)
is under its [MIT licence](LICENSES/AE2ImportExportCard-MIT.txt), combined in the same files
with AE2's LGPL 3.0 code; the files are listed in
[addons/ae2importexportcard/SOURCE_MANIFEST.json](addons/ae2importexportcard/SOURCE_MANIFEST.json).
Its assets, whose licence the mod does not state, are generated locally from the
player's jar and not distributed here.
Code derived from [Applied Flux](https://github.com/GlodBlock/ExtendedAE/tree/appflux/1.21.1-neoforge)
1.21-2.1.7 by GlodBlock (commit `523c3e5aa1579f0cba54a18af1382e36083699c9` on branch
`appflux/1.21.1-neoforge` of ExtendedAE's repository, which sets that version; it has no tag
of its own) is under its [LGPL 3.0](LICENSES/AppliedFlux-LGPL-3.0.txt), the repository's
`LICENSE.txt`, with the GPL 3.0 it extends in [AE2-LGPL-3.0.txt](LICENSES/AE2-LGPL-3.0.txt);
the files are listed in [addons/appflux/SOURCE_MANIFEST.json](addons/appflux/SOURCE_MANIFEST.json).
Its assets, whose licence the mod does not state, are generated locally from the
player's jar and not distributed here.
