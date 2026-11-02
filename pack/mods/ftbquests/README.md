# FTB Quests' quest book, as a native mod

The quest book of FTB Quests for this fork of Horizon: the chapter list, each chapter's quest graph with its lines,
scrolling and zoom, and a quest's panel with its description, tasks and rewards. Quests come from the player's own
instance (`config/ftbquests/quests`), the same files FTB Quests reads on Java.

## Credits

[FTB Quests](https://github.com/FTBTeam/FTB-Quests) and [FTB Library](https://github.com/FTBTeam/FTB-Library) are by the
FTB Team. Their jars say All Rights Reserved, so nothing of them is copied here: this is our own code, written from how
the mod behaves, its public source as a reference, and its file formats. Its textures are read from the player's own
jars and stay on the player's computer. Quest files, pictures and translations belong to the pack that ships them
(All the Mods 10's are All Rights Reserved) and stay in the player's instance; nothing derived from them is in this
repository, and no test holds any of it.

## What it does

- The book item (`ftbquests:book`) opens the screen, as `/ftbquests open_book` does. The mod's own key (unbound until
  the player binds it, as the mod ships it) is in Options > Mods.
- Item tasks, looking at the inventory or handing the items in; smart filters (`ftbfiltersystem`) of items, tags and
  namespaces; checkmark tasks. Other task types show with their real text, locked.
- Item rewards with a claim button and the Collect button. Others show with their real text, locked.
- Dependencies, in all of FTB Quests' requirements and both progression modes, unlock quests.
- Progress per player, saved with the world in `<world>/ftbquests/<uuid>.snbt`, in FTB Quests' own fields.

See `NOTES.md` for the state and what is left, and `docs/native-mods.md` (API 64) for the screen hooks it uses.

## Layout

| Path | What |
|---|---|
| `horizon-mixin.toml`, `hooks/` | The bundle: one small helper per hook |
| `runtime/ftbq_world.rs` | The server half: progress, detection, rewards |
| `runtime/ftbq_ui.rs` | The screen |
| `ftbquests-core` | The pure crate: SNBT, the book, translations, rules, geometry, messages |
| `ftbquests-content` | Tool: the local content mod of GUI textures, from the player's jars |
| `tests/` | World tests |

`bun run pack/scripts/ftbquests.ts content units stage build world` builds and tests all of it (`ATM10_DIR` names the
instance; `FTBQ_TESTS` picks single tests).
