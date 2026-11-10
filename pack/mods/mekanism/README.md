# Mekanism on this fork

Mekanism's energy blocks, its processing machines and factories, its chemicals, their tanks and tubes, its Mechanical Pipes, Fluid Tanks and Logistical Transporters, its Configurator and its multiblocks,
and Mekanism Generators' Solar, Heat and Gas-Burning Generators for All the Mods 10, as a native mod of this fork of Horizon. It is the native bundle in this folder (`horizon-mixin.toml`, `hooks/`, `runtime/`), a crate of
Mekanism's energy blocks, its processing machines and factories, its chemicals, their tanks and tubes, its Mechanical Pipes, Fluid Tanks and Logistical Transporters and its Configurator,
its Electric Pump and Fluidic Plenisher, and Mekanism Generators' Solar, Heat, Gas-Burning, Bio, Wind and Advanced Solar
Generators for All the Mods 10, as a native mod of this fork of Horizon. It is the native bundle in this folder (`horizon-mixin.toml`, `hooks/`, `runtime/`), a crate of
pure rules (`mekanism-energy`) and a content tool (`mekanism-content`). `pack/scripts/mekanism.ts` builds and tests
them; `NOTES.md` says where the work stands.

## Credits and licence

Mekanism and Mekanism Generators are MIT licensed, Copyright (c) 2017-2025 Aidan C. Brady
(<https://github.com/mekanism/Mekanism>). The rules in `mekanism-energy` are ported from Mekanism 10.7.19.

No file of Mekanism is committed. Models, textures, recipes and loot tables are generated locally from the player's own
jars by `mekanism-content`, with our block declarations in `content/` over them: two content mods, `mekanism` and
`mekanismgenerators`. Built from the full pack's carries (`horizon-port-pack final-data`), they hold ATM10's final
recipes, after KubeJS and Almost Unified, and the pack's tags of their own blocks and items; that is local too. The heat generator's gauge draws the game's own lava texture, copied from the player's client
assets when `mekanism-content` is given them.

## What there is

- Basic, Advanced, Elite, Ultimate and Creative Energy Cubes: Joules in the block entity, side configuration and
  ejection, the screen, placement facing, an item that keeps its energy.
- Settings from ATM10's `config/Mekanism/` (`general.toml`, `tiers.toml`, `common.toml`, `generators.toml`,
  `generator-storage.toml`), read again when a file changes.
- Forge Energy at a cube's edge (hook points API 154): each side as an `IEnergyStorage`, and the ejector feeding any block
  with Forge Energy, such as AE2's Energy Acceptor, at `feConversionRate` Joules to the FE.
- Basic, Advanced, Elite and Ultimate Universal Cables (block entity lifecycle hook points, API 156): networks with one
  buffer as big as their cables together, kept in the bundle per dimension; they connect to cubes and to any block with
  Forge Energy, emit to all of them each tick, pull through a pull side, and keep their shares across saves, restarts
  and chunks unloading. Their OBJ model becomes a multipart blockstate in `mekanism-content`.
- The Solar Generator: Joules from the sun by its biome (the helpers `native_biome` and `native_dimension_type` of hook
  points API 157), the time of day and the weather, Minecraft 1.21.1's maths for the sun as Mekanism reads it on ATM10,
  out of its bottom, at ATM10's `solarGeneration = 150` (`generators.toml`) and 96 000 J (`generator-storage.toml`).
- The Heat Generator: a lava tank (24 000 mB) filled from lava buckets and furnace fuel in its fuel slot (`burnTime / 20`
  mB; a hopper puts only fuel there), heat from burning 10 mB a tick (800), from lava on each face (30) and in an
  ultra-warm dimension (100), a heat capacitor that loses heat to the biome's air and turns it into Joules at the Carnot
  efficiency, out of its front; `facing` and `active` (light 13) in its block, `active` going off after
  `blockDeactivationDelay`. Its tank is a fluid handler (hook points API 160) on every side but the front: any block
  (AE2's export and storage buses) fills it with lava, none drains it; its fuel slot fills it through the item's fluid
  handler (a whole bucket, or what a container gives).
- The Gas-Burning Generator: an 18 000 mB tank (`[gas_generator] tankCapacity`) of any chemical with a fuel attribute
  (hydrogen: a tick of 200 J a mB), filled by any block on its sides but the front and by a chemical tank item in its
  fuel slot, burning up to 256 of its ticks a tick by how full the tank is and as far as its 200 000 J store (1000
  times hydrogen's density) has room, out of its front at twice what it made last tick.
- The Bio-Generator (`mekanism_energy::bio`, runtime `mek_gen2`): a bioethanol tank (`[bio_generator] tankCapacity`,
  24 000 mB) that any block fills on every side but the front with what is in `c:bioethanol` and none drains, a fuel
  slot that empties a bioethanol container into it or turns an item of `c:fuels/bio` into 200 mB and of
  `c:fuels/block/bio` into 1800 when that fits, and a burn of one mB a tick for `bioGeneration` Joules (350 on ATM10)
  while all of them fit its 160 000 J store, out of its front at twice that; `facing` and `active` (light 13).
- The Wind Generator (`mekanism_energy::wind`): Joules by the height of its top, four blocks up, in a line from
  `generationMin` at `minY` to `generationMax` at the world's logical top (150 to 900 J on ATM10: 353 at y 100, 607
  at 200), nothing while its top holds a fluid or can't see the sky; it looks every 20 ticks, the first on its first
  tick, and puts out of its front and bottom at twice `generationMax`; `facing` and `active` (light 8). Mekanism
  10.7's wind doesn't follow the weather.
- The Advanced Solar Generator (`mekanism_energy::solar`'s `AdvancedSolar`): nine sun checks over the three by three
  panel two blocks up, each with the biome there and its own look every 10 to 30 ticks (`AdvancedSolarCheck`);
  while more than four see the sun it makes `advancedSolarGeneration` (1200 on ATM10) times the sun's brightness
  times the checks' mean multiplier (1056 J in plains at a clear noon), into a 2 500 000 J store, out of its front
  and bottom at twice the mean peak's production (2112), twice 1200 before its first tick.
- Bounding blocks (`AttributeHasBounding`): a player placing a wind generator gets four `mekanism:bounding_block`s
  over it, an advanced solar generator one and nine over that; the item isn't placed (the use is claimed in
  `native_use_item_on`) where one of them wouldn't fit (`#minecraft:replaceable` or air, inside the world), and a
  generator taken out of the world takes its bounding blocks with it at the end of the tick.
- The generators: Forge Energy on their energy faces, cables connecting there, a screen like Mekanism's
  (`GuiSolarGenerator`, `GuiHeatGenerator`, `GuiGasGenerator`, `GuiBioGenerator`, `GuiWindGenerator`, the advanced
  solar generator's as the solar's), and `/mek probe` answers for tests.
- The Electric Pump (`mekanism_energy::pump`, runtime `mek_pump`): a source block every 19 ticks of its Joules
  (`machine-usage.toml` `electricPump`, 100 J a tick, from its first source on) into a 10 000 mB tank: the block under
  it first, then in a shuffled order the sources it took (`recurring_nodes`) and those beside them within `[pump]
  range` (80) blocks, one fluid at a time; water stays in the world while `waterSourceConversion` makes it endless,
  lava goes; a filter upgrade turns water into `heavyWaterAmount` mB of heavy water (50 on ATM10); its tank goes up
  into the fluid handler above it (a mechanical pipe, a tank) at `256 * (1 + speed)` mB a tick, and is drained
  through its top. Speed and energy upgrades as the machines', its slot filling a container from the tank.
- The Fluidic Plenisher: a 10 000 mB tank filled through its top or from a container in its slot (a fluid with a
  block), a bucket out into the world every 20 ticks: it walks the space under it (down, north, south, west, east,
  never up) one block an operation up to `maxPlenisherNodes` (4000), taking 100 J every tick while it walks; then it
  keeps the block under it full, 100 J a placement. Water evaporates in an ultra-warm dimension; a waterloggable
  block takes water in.
- Both take Joules through their back from cables, cubes and Forge Energy, save as Mekanism does (`progress`,
  `fluid`, `recurring_nodes`, `finished`, `active_nodes`, `used_nodes`, `fluid_tanks`, `component_upgrade`), and
  have a screen like `GuiElectricPump` with their upgrade slots beside it.
- The Enrichment Chamber, the Crusher and the Energized Smelter (`TileEntityElectricMachine`) and the Metallurgic
  Infuser, on ATM10's `machine-usage.toml` (50 J a tick), `machine-storage.toml` (20 000 J) and `general.toml`
  (`maxUpgradeMultiplier = 16`): ATM10's final `mekanism:enriching`, `mekanism:crushing` and
  `mekanism:metallurgic_infusing` recipes (`native_recipes_of_type`, from the carries the content mods come from) and
  the world's smelting recipes (`native_cooking_recipes`, hook points API 159), item, tag, compound and intersection
  ingredients; 200 ticks of energy a tick a recipe (`CachedRecipe.process`: short of a tick's energy it waits, an input
  that no longer fits starts it over, a full output waits), the progress saved.
- Their upgrades: speed and energy (and muffling) from the upgrade slot, in on the 21st tick
  (`TileComponentUpgrade`); ticks, Joules a tick and the store as `MekanismUtils.getTicks`, `getEnergyPerTick` and
  `getMaxEnergy` work them out (8 speed: 12 ticks of 12 800 J; 8 energy: 4 J a tick and 320 000 J; both: 800 J a
  tick); the upgrade window uninstalls them into the upgrade output slot.
- Their energy slot turns redstone into Joules (`mekanism:energy_conversion`) when it all fits; the infuser's infusion
  slot turns items into its infuse type (`mekanism:chemical_conversion`: carbon, redstone, diamond, refined obsidian,
  bio, fungi, gold, tin), one item a tick, into a 1000 mB tank of one type, used when an operation ends.
- Mekanism 10.7's chemicals (`docs/mekanism-chemicals.md`): the `mekanism:chemical` registry, each chemical as the
  NeoForge oracle saw it (icon, tint, kind, fuel, radioactivity, coolants, tags), declared by `mekanism-content` in
  the content mods (`data/<ns>/mekanism/chemical/`) and read with hook points API 162 (`native_data_files`);
  chemical stacks and tanks in Mekanism's saved form; the chemical handler of the bundle's blocks by side
  (`IChemicalHandler`): the infuser's tank takes an infuse type a recipe takes on its input sides and gives it out of
  its right side.
- The Chemical Oxidizer (an item into a chemical, `mekanism:oxidizing`, 100 ticks, out of its right) and the Chemical
  Infuser (two chemicals into one, `mekanism:chemical_infusing`, `2^speed` operations a tick, out of its front), with
  their chemical item slots, side configuration, ejectors and screens.
- The Osmium Compressor, Purification Chamber and Chemical Injection Chamber: an item and a chemical (a 210 mB tank
  filled from chemical items and conversions in their chemical slot) into an item in 200 ticks, the chemical taken
  every tick: the compressor's 200 mB an operation spread over its ticks, the chambers' drawn at random
  (`randomizedConsumption`), more with speed upgrades and less with chemical upgrades.
- The ore chain: the Chemical Dissolution Chamber (an ore and sulfuric acid drawn at random every tick into 1000 mB
  of a dirty slurry in 100 ticks of 400 J), the Chemical Washer (`2^speed` operations a tick of water and a dirty
  slurry into the clean one, its water from fluid handlers and buckets) and the Chemical Crystallizer (200 mB of a
  clean slurry into a crystal in 200 ticks), on ATM10's recipes: All the Ores' slurries, clumps and crystals, its
  slurries declared in a content mod of its carry. Each ejects its output tank out of its right.
- The Rotary Condensentrator: a chemical into its fluid (Mekanism's `mekanism:rotary` recipes) or back by its mode,
  `2^speed` operations a tick of 50 J, its tanks taking and giving by the mode, its chemical ejected while
  decondensentrating and its fluid while condensentrating (`[auto_eject] fluid`, 1024 mB a tick, split evenly among
  the fluid handlers on its fluid output sides), its chemical and fluid slots filling and emptying items, its mode
  button.
- Basic, Advanced, Elite, Ultimate and Creative Chemical Tanks (`tiers.toml` `[chemical_tanks]`): a chemical in the
  block entity, ejected at the tier's output a tick out of the output sides, dump modes, side configuration (chemicals
  and items), the drain and fill slots that move chemicals to and from tank items, the screen, placement facing, and
  an item that keeps its contents, with its tooltip and bar. Tank items fill and empty in machines' chemical slots too
  (the separator's outputs, the infuser's infusion slot).
- Basic, Advanced, Elite and Ultimate Pressurized Tubes (`[transmitters.chemical]`): networks of one chemical as the
  cables', connecting to any block with a chemical handler on the touching side, emitting to all of them each tick,
  pulling through a pull side at the tier's pull amount, keeping their shares across saves, with the cables' OBJ
  model as a multipart blockstate.
- The Electrolytic Separator: water (any fluid a `mekanism:separating` recipe takes) in a 24 000 mB tank, filled by
  fluid handlers on its fluid input sides and by a bucket in its slot, into hydrogen (left) and oxygen (right) tanks
  of 2400 mB, `2^speed` operations a tick of 400 J (twice hydrogen's fuel density), dump modes (idle, dumping excess,
  dumping), its outputs ejected to the chemical handlers on its left and right, its screen with gauges and dump
  buttons.
- The Combiner (`TileEntityCombiner`, ATM10's 50 J a tick and 40 000 J): ATM10's `mekanism:combining` recipes, a main
  and an extra input (its bottom the extra slot, `EXTRA_MACHINE`) into one output, 200 ticks; an input taken out
  starts it over. The Precision Sawmill (50 J a tick, 20 000 J): `mekanism:sawing`, a main output and a secondary one
  that comes by chance, rolled each operation (`BasicChanceOutput`: the oracle's 200 operations at 25 % gave 43
  sawdust); it waits while its secondary slot can't take the most the recipe gives.
- The Basic, Advanced, Elite and Ultimate factories of smelting, enriching, crushing, combining and sawing
  (`TileEntityFactory`), and of compressing, purifying, injecting and infusing: 3, 5, 7 and 9 processes in Mekanism's
  slot order (input, output and the sawing factories' secondary by process, the combining factory's shared extra
  slot or the chemical factories' chemical slot, the energy slot), each with its own recipe and
  progress (the saved `progress` int array) on one store of `max(storage / 2, usage) * processes` Joules (30 000,
  50 000, 70 000 and 90 000 J of a smelting factory on ATM10; 60 000 J of a basic combining factory), each process
  taking the machine's Joules a tick in turn, so the last ones wait when the store runs short; a factory's recipe for
  a process is one whose output stacks with what its output slots hold. Their upgrades and side configuration as the
  machines', their screens like `GuiFactory` (a down arrow of progress for each process, the ultimate's 34 pixels
  wider panel, the sawing factories' 21 pixels taller one) with the sorting tab (`GuiSortingTab`): sorting on, the
  inputs are spread evenly over the processes whenever their slots changed (`sortInventory`), in multiples of what an
  operation takes, empty processes joining the kinds that can fill more of them. The chemical factories share one
  tank of 210 mB a process (the infusing one's 1000), filled by their chemical slot and on their machines' chemical
  sides, its chemical taken a tick by each process as their machines take it (the compressing one's 200 mB an
  operation spread over its ticks, the purifying and injecting ones' drawn at random) or, infusing, with the item; a
  chemical bar and dump button on their 11 pixels taller screens.
- Tier installers (hook points API 165: `native_use_item_on` with `native_used_stack`, `native_instabuild`,
  `native_default_state`): a sneaking player's basic installer turns an Enrichment Chamber, Crusher, Energized
  Smelter, Combiner, Precision Sawmill, Osmium Compressor, Purification Chamber, Chemical Injection Chamber or
  Metallurgic Infuser into the basic factory of its type (with its chemical tank, chemical slot and use so far), an
  advanced, elite or ultimate one a
  factory into the next tier, with what it held (`MachineUpgradeData`): each process's input, outputs and progress,
  the energy and extra slots, the upgrades and the upgrade slots, the side configuration and ejecting, the sorting,
  the facing, and its Joules up to the new block's own store (`setEnergy` before the upgrades are read). The
  installer is used up but in creative. `/mek install <pos> <installer>` does the same for tests.
- An energy cube's item in a machine's or a factory's energy slot charges it (`fillContainerFromItem`) at the cube
  tier's output a tick (4 000 J of a basic cube on ATM10), its custom data keeping what is left; an item that
  converts to energy is used only when no cube item gave any.
- Their side configuration (energy, items, the infuser's and the separator's chemicals), as the cubes': energy in on the faces that take
  it, from cables, cubes and Forge Energy; the item ejector puts the output into the blocks on the faces whose type
  outputs, every 11 ticks (`TileComponentEjector`); hoppers fill the input with what a recipe takes.
- `facing` from placement and `active` (light 8) in their blocks, `active` going off after `blockDeactivationDelay`;
  their screens like Mekanism's (`GuiElectricMachine`, `GuiMetallurgicInfuser`) with the side configuration and upgrade
  windows.
- Basic, Advanced, Elite and Ultimate Mechanical Pipes (`[transmitters.fluid]`, runtime `mek_pipes`): networks of one
  fluid as the tubes' (`network` with a fluid as its content: its id, or the id and a number for one with components),
  connecting to any block with a fluid handler on the touching side (hook points API 160: the bundle's tanks and
  machines, cauldrons, AE2), emitting to all of them each tick with the even split, pulling through a pull side at
  the tier's pull amount (250, 1000, 8000, 32 000 mB on ATM10) when all of it fits, each pipe's share saved as
  `fluid`, the network a fluid handler on each side for the blocks around it (normal and pull sides take in, normal
  and push give out); the large OBJ transmitter model as a multipart blockstate.
- Basic to Creative Fluid Tanks (`[fluid_tanks]`, `mekanism_energy::fluid_tank`, runtime `mek_fluid_tanks`), as the
  pipes' sources and sinks: one tank on every side that other blocks fill and drain without a rate limit, giving
  down at its output a tick while active (`active_state`; at most what a fluid tank below still takes), a creative
  tank keeping what it gives and voiding what it gets; saved as Mekanism saves it (`fluid_tanks`).
- Basic, Advanced, Elite and Ultimate Logistical Transporters (`[transmitters.items]`, `mekanism_energy::transporter`,
  runtime `mek_transporters`, hook points API 166): stacks travel through touching transporters as moving items,
  the tier's speed in hundredths of a block a tick (5, 10, 20, 50), each on the cheapest path (A* over
  `ULTIMATE.speed / speed` a transporter) to an inventory of the network that takes it, the stacks already on their
  way there counting against its room (`TransporterManager`); a pull side takes up to the tier's pull amount (1,
  16, 32, 64) of one kind, the last slot first, every 11 ticks while that works, waiting longer each time it
  doesn't; a stack whose target stops taking it looks again past half way and at the end of each transporter,
  goes home (`HOME`) or wanders the network (idle) until one takes it. A transporter of a colour joins no
  transporter of another and takes only stacks of its colour; a pulled stack takes its puller's. The containers
  around them (chests, furnaces, the bundle's machines) are their inventories through the container helpers and
  `native_insert_slots`; a machine's item ejector puts its output into one on a path. Their stacks and colour are
  saved as Mekanism saves them (`items`, `color`), a broken one's dropped (`native_pop_resource`), and clients draw
  each stack where it travels (`native_block_entity_update_tag`, `native_block_entity_submit`). `/mek color` sets a
  transporter's colour as the Configurator does. `transporter::round_robin` is `getNewRRPath`'s choice, ready for
  the Logistical Sorter.
- The Configurator (`mekanism_energy::configurator`, runtime `mek_configurator`): its eight modes in the item
  (`custom_data.mekanism.mode`, its model by `custom_model_data`), set with `/mek configurator <mode>`;
  configurating, a sneaking use bumps the used side of a cube's, machine's, factory's or chemical tank's side
  configuration of that type (items, energy, chemicals) through the block's own side button, a transmitter's side
  connection (normal, push, pull, none), a transporter's colour on a connected side, and turns a fluid tank's giving
  down on and off; empty takes a machine's items out into the world; rotate turns a block to face away from the side
  used (sneaking) or clockwise (a plain use, before its screen opens: `native_right_click_block`); wrench takes a
  Mekanism block up into the player's inventory with what its drop keeps (sneaking). In Mekanism 10.7 the
  Configurator holds no energy. `/mek connection` stays for tests.
- Mekanism's multiblock framework (`mekanism_energy::multiblock`, runtime `mek_multiblock`), as its cuboid multiblocks
  form (`FormationProtocol`, `CuboidStructureValidator`, `StructureHelper.fetchCuboid`): the casings, valves, ports
  and structural glass of one manager that touch are a structure (at most 5832); the first and last full planes of
  frames on each axis make its cuboid, from 3x3x3 to 18x18x18, and every block of it is checked, x then y then z: a
  casing on each edge, a casing, valve or glass on the sides, what the multiblock takes inside. A formation runs on the
  level tick after one of its blocks comes or goes, or a block inside it beside one of its blocks changes (a block
  deeper inside goes unseen, as in Mekanism). What a multiblock holds lives in a cache by its id (`MultiblockCache`,
  `inventory_id` in every one of its blocks), kept in the level's saved data as Mekanism's `mekanism_<manager>`
  (`native_attachments`): broken, the cache stays; formed again (under a new id), the caches its blocks name merge
  into it, capped to the new size. Its two slots are mirrored into the blocks whose screen a player opened and into
  its valves and ports (hoppers reach those), and none of its blocks drops them. Formed, it sends red dust sparkles
  over its faces (`SparkleAnimation`, hook points API 167's `native_level_particles`). A Configurator in a
  configurating mode, used plainly on a casing, valve or port that doesn't form, says why when Mekanism does
  (`Couldn't create frame, invalid block at ...`, `Inner structure contains an illegal block ...`); a plain use with an
  empty hand on one that doesn't form opens no screen. `/mek probe` of a part answers the multiblocks it is in,
  `/mek multiblock caches` every cache.
- The Dynamic Tank (`mekanism_energy::dynamic_tank`, runtime `mek_dynamic_tank`): dynamic tank casings with dynamic
  valves and structural glass in the sides and air inside, holding one fluid or one chemical (not both, no
  radioactive one) of `volume * fluidPerTank` mB or `volume * chemicalPerTank` (ATM10's `general.toml`: 350 000 and
  16 000 000 a block of the whole box: 9 450 000 mB of fluid for a 3x3x3, 2 041 200 000 for an 18x18x18). Every valve
  is its fluid handler (hook points API 160) and its chemical handler on every side, without a rate (the null side
  reads only); pipes, tubes, tanks and AE2 fill and drain it there. Its two slots fill and empty containers as its
  edit mode says (`HybridInventorySlot.handleTank`: both, fill, empty; chemical tank items through the chemical
  side); a plain use with a bucket fills or empties it. Its screen is `GuiDynamicTank`'s, with the edit mode tab.
- The Induction Matrix (`mekanism_energy::induction`, runtime `mek_matrix`): induction casings with induction ports and
  structural glass in the sides, and induction cells, providers or air inside. Its Joules live in its cells
  (`energy_containers` in each cell's block entity, kept by a broken cell's item), its room is their capacity and its
  transfer each way a tick its providers' output (ATM10's `tiers.toml` `[induction]`: 8, 64, 512 and 4000 GJ; 256 000,
  2 048 000, 16 384 000 and 131 072 000 J a tick). What goes in and out during a tick is queued and lands in the cells
  at its next tick (`MatrixEnergyContainer`), before the level's blocks tick. A port takes energy in its input mode
  and gives it in its output mode (its `active_state`, shown as `active`; the Configurator's sneaking use toggles it),
  to cables, cubes, machines and Forge Energy (hook points API 154) alike, and in its output mode puts out to the block
  it faces, split among the output ports, the transfer at most. Its slots charge an energy cube's item and take one's
  Joules or an item that converts to energy. Its screen is `GuiInductionMatrix`'s, with `GuiMatrixStats` as a page.

## Differences from Mekanism

- A network's acceptors are sorted by position; Mekanism goes in its `HashMap`'s order. Only who gets a leftover Joule differs.
- A new or loaded cable joins at the end of its first tick, so what it takes starts a tick later than Mekanism's own buffer does.
- A connection to a block with energy that appears with no block changing (an AE2 part, a cube's data) is noticed within a
  second. Cubes and blocks being placed or removed are noticed at once.
- A chunk's block entities load or unload with the chunk, by Horizon's own rules, and not Mekanism's "accessible" ticket level.
- Redstone sensitivity isn't ported; `/mek connection <pos> <side> <type>` sets a transmitter's side as the Configurator does.
- A cable's outline and collision are its core: no arms until content blocks can have several boxes by several properties.
- Cables can't be waterlogged: their six sides use all of a block's 4096 states.
- A generator's peak (solar) or air (heat) is read on its first tick after it is placed or loaded, as Mekanism does; the
  biome's temperature is 26.3's (cooler above the dimension's sea level + 17), which in the overworld is 1.21.1's
  (above y 80).
- The sky's darkening that hides the sun (`skyDarken`) is computed from the same day time as its brightness; Java
  computes it a tick earlier, so at dusk and dawn the solar generator may stop or start a tick apart.
- The solar generator's outline and collision are one box, its panel's height (half a block); its block has no
  `facing` or `active`, which Mekanism's model doesn't show. The heat generator is a full block.
- Lava beside the heat generator is vanilla's lava block, and lava-logging isn't read; the pack tags no mod fluid lava,
  so lava buckets are the only fluid items its fuel slot meets. No heat goes to a heat handler below (none is ported).
- Hoppers take nothing out of the heat generator's fuel slot (an empty bucket stays until a player takes it): a hook
  point for what a slot gives is missing, and coal must stay. Its energy slots charge nothing until items carry energy.
- Generators drop plain: their energy, lava and heat stay behind, as the cube did before its items kept energy.
- Bounding blocks keep no `main` position: the generator a bounding block belongs to is the one whose positions hold
  it. Their outline is empty and they have no collision (the carry's declaration: Mekanism gives them the main
  block's shape, offset), so a player can't hit, use or break one, nor stand on the advanced solar panel; one taken
  away alone (an explosion, `/setblock`) leaves its generator, and none is waterlogged. `/setblock` of a generator
  places none, as in Java.
- The wind generator reads the world's build height as its logical height (`native_world_height`; the Nether's lower
  one has no sky to see), and no dimension tag (`c:no_wind`, which no dimension of ATM10 has); its blades aren't
  drawn (Mekanism's renderer). The advanced solar generator's checks draw their frequency from the bundle's own
  random source and count their time in its own ticks; over a fluid a check reads the sky of the block above
  without its light-blocking. A block set over a wind generator's top or a check is seen at its next look once the
  engine's sky light has settled, a tick or more later (Java's light was there on the oracle's next tick).
- The bio-generator's fuel slot takes from hoppers the items the content's `c:fuels/bio` and `c:fuels/block/bio` tag
  files list (the slot's own use reads the world's tags).
- The pump and plenisher read `waterSourceConversion` and `lavaSourceConversion` as vanilla's defaults (no hook point
  reads game rules), shuffle from the bundle's random source, and play no sound or game event. The plenisher replaces
  air, flowing fluids, `#minecraft:replaceable` and waterloggable blocks (Java also any block that isn't solid), drops
  a replaced block's default state's loot, can't tell an unloaded position from air, and its waterlogging schedules no
  fluid tick. Their reset (a sneaking Configurator, `onSneakRightClick`), redstone control and comparators aren't
  ported; their upgrades go in only through the upgrade slot (no upgrade window). Their outline and collision are
  one box, the part every facing shares, and their block has `facing` only (Java's `active` never changes).
- Machines drop plain too: their energy, upgrades and items in the slots stay behind (Java's item keeps them).
- A machine's container faces are its block's, not its facing's: hoppers reach its input and energy slots (and the
  infuser's infusion slot) and its output from every face, where Java's item side config says which face reaches
  which slot. The item side config drives the ejector only. Its upgrade slots are in a holder beside the screen
  (Mekanism shows them only in its upgrade window), and right click uninstalls all of an upgrade (Mekanism: shift).
- Mekanism pauses a recipe on an error until its slots or energy change; this port checks again every tick, which
  comes to the same. Redstone control stays IGNORED; of the energy items only an energy cube's charges a machine.
- A machine placed with `/setblock` has its item's side config here; Java's has every side `NONE` until set.
- Item slots are saved in the game's `Items` list, not Mekanism's `items`, and the upgrade slots there too.
- A factory sorts its inputs by kinds in the order they are first met in its processes; Mekanism goes in its hash
  map's order, which only decides which kind gets the empty processes first. Hoppers fill a factory's inputs with
  what a recipe takes without Mekanism's check that the process's output can take its result
  (`inputProducesOutput`), which needs the block's slots.
- A sawing process with more than one operation a tick (never at 200 base ticks) gives each successful roll's
  secondary once; Mekanism's `handleOutput` gives it `operations` times over.
- A `/data merge` of a machine's or factory's `progress` sets it; Mekanism's cached recipe keeps counting its own
  ticks (the oracle's factory took 29 more ticks of 800 J after its progress was merged to 0).
- Factories are full blocks without Mekanism's shapes (`BlockShapes`), like the infuser. Tier installers on energy
  cubes, tanks and bins aren't ported. Java refuses an installer while a player has the block's screen open; this
  port doesn't check.
- A creative energy cube's item without saved energy counts as full here, as its drops are; on Java one without the
  `mekanism:energy` component holds nothing (Mekanism's own creative cube items always carry it). Other energy
  items (batteries, Forge Energy items) don't charge machines yet.
- The infuser's infusion bar and the separator's gauges are the chemical's colour, where Mekanism tints its texture;
  chemical items other than the Chemical Tanks aren't ported. Per-tick infusing (`per_tick_usage`), which ATM10's
  recipes don't use, isn't ported. The separator's, washer's and rotary condensentrator's fluid sides (and the
  rotary's fluid ejecting) stay at their defaults; `docs/mekanism-chemicals.md` lists the chemicals' own differences (tank slots, tube networks joining).
- A tag-based recipe takes what the tag holds in the world: with only Mekanism's content mods, the pack's `c:` and
  `minecraft:` tags (in ATM10's `atm10-pack` carry) hold just Mekanism's own items, which `mekanism-content --pack`
  copies in, so a recipe on another mod's items does nothing there.
- Pipe networks split their buffer and their saved shares by position (Mekanism: `HashMap` order): the oracle's three
  basic pipes holding 4750 saved 1583, 1584, 1583, here 1584, 1583, 1583. Two pipes pulling different fluids into one
  empty network in the same tick: the one first by position wins here, Mekanism's block entity order there. The
  large model's bevels (a tenth of a turn off an axis) are flat rectangles here and its rings' trapezoids keep the
  outer edge's texture mapping. Pipes don't show their fluid, and redstone sensitivity isn't ported.
- Fluid tanks have no screen, slots, edit mode or item that keeps their fluid (they drop plain); a fill that doesn't
  fit doesn't overflow into a tank above, and their light is only `active`'s.
- Transporters' targets and paths of equal cost go by position (Mekanism: hash order); paths are found afresh each
  time (Mekanism caches them, `PathfinderCache`). They tick after the level's block entities (Mekanism: in its
  ticker order among them), each in the order it came, as Mekanism's do among themselves. Only pull sides and the
  bundle's machines' ejectors put stacks into them: hoppers and other mods' blocks can't yet (no hook point for an
  item handler of a mod block that takes a stack on a path). A machine's ejector colours and strict input aren't
  ported, nor the coloured glass of a transporter's model (its colour shows in the Configurator's messages), nor
  the Diversion and Restrictive Transporters or the Logistical Sorter. Clients get a transporter's stacks when
  they change and move them on by its speed a tick from the frame they first draw them.
- A multiblock forms on the level tick after its last block comes; Mekanism's tiles look on their tenth tick (the
  oracle formed a 3x3x3 eleven or twelve ticks after it was built). Every formation, even of a structure that was
  formed already (a node loading with its chunk, glass placed beside it), is a new one under a new id, as Mekanism
  removes and forms it anew; caches that merge go in id order (Mekanism: its hash map's), and rejected contents are
  lost (Mekanism drops rejected items at the nearest player and radiates rejected radioactive chemicals). Changes are
  seen in the dimension that ticks next (`native_block_changed` says no dimension). A multiblock's caches are kept in
  its own dimension's level (Mekanism keeps them all in the Overworld's), and a Java world's `mekanism_*.dat` isn't
  read. Structural glass opens no screen of the multiblock (Mekanism's `allowsStructuralGuiAccess`), and comparators
  read nothing of a valve or port. With an item in hand, a casing, valve
  or port that doesn't form opens its (empty) screen as a container block does, so a sneaking use places against it;
  Mekanism opens nothing then and the item acts.
- The Dynamic Tank's slots are the multiblock's, shown on the blocks a player opened and on its valves; a hopper on a
  casing reaches nothing, on a valve both slots. The slots' `isDraining` and `isFilling` aren't saved. A bucket used
  on it acts on the tank's own merged tank, a creative player's bucket changing too. The valves don't show their
  transfer (`activeTicks`), and the tank draws no fluid inside.
- The matrix's cells take and give in position order (Mekanism: its hash map's), which only decides which cell holds
  the Joules. Its tick runs before the level's blocks (`native_level_tick_pre`), as the oracle's matrix ticked before
  the cube placed after it; one placed before its master casing would land a tick sooner in Mekanism. Only an energy
  cube's item charges or discharges in its slots.
- The Configurator's mode is set with `/mek configurator`: Mekanism's mode key and sneak-scroll aren't ported. Its
  messages go to the chat, not the action bar. A transmitter's arms aren't hit (its outline is its core), so a
  sneaking use configures the side used, and on a transporter a connected side bumps the colour, as a use on
  Mekanism's core does. A plain use on a transmitter doesn't toggle redstone sensitivity (not ported); block
  security and the radiation check of the wrench aren't ported; the empty mode takes every slot the block keeps,
  its upgrade holder's too.
