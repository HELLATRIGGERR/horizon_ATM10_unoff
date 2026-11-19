# Flux Networks (native mod of this fork)

A port of [Flux Networks](https://github.com/SonarSonic/Flux-Networks) 8.0.0 by **Sonar Sonic** and **BloCamLimb**:
cable-free power between plugs and points on a network, as All the Mods 10 runs it. Written for this fork; the mod
itself still comes from the player: its art, models and data are read from the player's own jar at build time and
never committed.

- `flux-core/`: the rules with no game in them (a device's buffer, limit, priority and surge, the transfer cycle,
  the saved networks, the settings, the chest menu's layout).
- `runtime/flux_world.rs` and `hooks/`: the glue on Horizon's named hook points (`docs/native-mods.md`).
- `content/`: block declarations over the carry of the jar. `pack/scripts/fluxnetworks.ts content` builds the content
  mod, loaded with `--dev-mod`.
- `tests/`: world tests. `pack/scripts/fluxnetworks.ts units` runs the crate's tests, `stage build world` the rest.
- `NOTES.md`: state and next steps.

Right-clicking a plug or a point opens a chest menu in place of the mod's GUI, on purpose: it lists your networks and
offers create and disconnect, and looks nothing like the original GUI.

## On upstream's hooks (merge of nightly-source/2026-11-18)

- The networks and settings are read at `server_started` and dropped at `world_lifecycle`'s close phases;
  `level_tick_pre` hands a `LevelRef` (its `dimension_id()` is the level the devices tick in).
- `block_entity` answers the compound only; the hooks hand the API's `mod_api::ModBlockEntity`, the levels' own maps
  the engine's (`EngineEntity` in `flux_world`), both with `id()` and their fields through `Deref`.
- `fe_receive` passes one `FeReceiveCall`: a `MODIFY_ARG` injection on the call into `fire_fe_receive`.
- `right_click_block` on its `BlockClick`: a device's click is cancelled and answered `Success`. `place_block` ->
  `on_block_placed_by` (HEAD, reading the call). `perform_command` -> `run_mod_command` (`/flux [line:text]`).
- Engine needs (HANDOFF "S3"): `right_click_block` and `on_block_placed_by` fire only while a trait mod listens
  (N5), so the device menu does not open from a click and a placed device does not take its item's settings yet.
  `pack/scripts/fluxnetworks.ts stage` stages every bundle under `pack/mods`, so it passes only once Mekanism is
  on upstream's hooks too.

## Licences

Flux Networks' code is MIT; the port's rules follow its source, so `flux-core`, `runtime/` and `hooks/` are MIT too:

```
MIT License

Copyright (c) 2017-2021 SonarSonic, BloCamLimb

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

The GUI's designs and resources are BloCamLimb's under CC BY-NC-SA 4.0. They aren't covered by the notice above and
aren't in this repository. The content mod holds a copy of the art for local use only, and nothing here reproduces the
GUI: it stays local, as does the block and item art, for which no separate licence is stated.
