# Mekanism on this fork

Mekanism's energy blocks, its processing machines and factories, its chemicals, their tanks and tubes, its Mechanical Pipes, Fluid Tanks and Logistical Transporters and its Configurator,
and Mekanism Generators' Solar and Heat Generators for All the Mods 10, as a native mod of this fork of Horizon. It is the native bundle in this folder (`horizon-mixin.toml`, `hooks/`, `runtime/`), a crate of
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
- Forge Energy at a cube's edge (hook points API 416): each side as an `IEnergyStorage`, and the ejector feeding any block
  with Forge Energy, such as AE2's Energy Acceptor, at `feConversionRate` Joules to the FE.
- Basic, Advanced, Elite and Ultimate Universal Cables (block entity lifecycle hook points, API 418): networks with one
  buffer as big as their cables together, kept in the bundle per dimension; they connect to cubes and to any block with
  Forge Energy, emit to all of them each tick, pull through a pull side, and keep their shares across saves, restarts
  and chunks unloading. Their OBJ model becomes a multipart blockstate in `mekanism-content`.
- The Solar Generator: Joules from the sun by its biome (the helpers `native_biome_climate` and `native_dimension_type` of hook
  points API 419), the time of day and the weather, Minecraft 1.21.1's maths for the sun as Mekanism reads it on ATM10,
  out of its bottom, at ATM10's `solarGeneration = 150` (`generators.toml`) and 96 000 J (`generator-storage.toml`).
- The Heat Generator: a lava tank (24 000 mB) filled from lava buckets and furnace fuel in its fuel slot (`burnTime / 20`
  mB; a hopper puts only fuel there), heat from burning 10 mB a tick (800), from lava on each face (30) and in an
  ultra-warm dimension (100), a heat capacitor that loses heat to the biome's air and turns it into Joules at the Carnot
  efficiency, out of its front; `facing` and `active` (light 13) in its block, `active` going off after
  `blockDeactivationDelay`. Its tank is a fluid handler (hook points API 422) on every side but the front: any block
  (AE2's export and storage buses) fills it with lava, none drains it; its fuel slot fills it through the item's fluid
  handler (a whole bucket, or what a container gives).
- Both generators: Forge Energy on their energy face, cables connecting there, a screen like Mekanism's
  (`GuiSolarGenerator`, `GuiHeatGenerator`), and `/mek probe` answers for tests.
- The Enrichment Chamber, the Crusher and the Energized Smelter (`TileEntityElectricMachine`) and the Metallurgic
  Infuser, on ATM10's `machine-usage.toml` (50 J a tick), `machine-storage.toml` (20 000 J) and `general.toml`
  (`maxUpgradeMultiplier = 16`): ATM10's final `mekanism:enriching`, `mekanism:crushing` and
  `mekanism:metallurgic_infusing` recipes (`native_recipes_of_type`, from the carries the content mods come from) and
  the world's smelting recipes (`native_cooking_recipes`, hook points API 421), item, tag, compound and intersection
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
  the content mods (`data/<ns>/mekanism/chemical/`) and read with hook points API 423 (`native_data_files`);
  chemical stacks and tanks in Mekanism's saved form; the chemical handler of the bundle's blocks by side
  (`IChemicalHandler`): the infuser's tank takes an infuse type a recipe takes on its input sides and gives it out of
  its right side.
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
  (`TileEntityFactory`): 3, 5, 7 and 9 processes in Mekanism's slot order (input, output and the sawing factories'
  secondary by process, the combining factory's shared extra slot, the energy slot), each with its own recipe and
  progress (the saved `progress` int array) on one store of `max(storage / 2, usage) * processes` Joules (30 000,
  50 000, 70 000 and 90 000 J of a smelting factory on ATM10; 60 000 J of a basic combining factory), each process
  taking the machine's Joules a tick in turn, so the last ones wait when the store runs short; a factory's recipe for
  a process is one whose output stacks with what its output slots hold. Their upgrades and side configuration as the
  machines', their screens like `GuiFactory` (a down arrow of progress for each process, the ultimate's 34 pixels
  wider panel, the sawing factories' 21 pixels taller one) with the sorting tab (`GuiSortingTab`): sorting on, the
  inputs are spread evenly over the processes whenever their slots changed (`sortInventory`), in multiples of what an
  operation takes, empty processes joining the kinds that can fill more of them.
- Tier installers (hook points API 424: `native_use_item_on` with `native_used_stack`, `native_instabuild`,
  and upstream's `native_default_state`): a sneaking player's basic installer turns an Enrichment Chamber, Crusher, Energized
  Smelter, Combiner or Precision Sawmill into the basic factory of its type, an advanced, elite or ultimate one a
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
  connecting to any block with a fluid handler on the touching side (hook points API 422: the bundle's tanks and
  machines, cauldrons, AE2), emitting to all of them each tick with the even split, pulling through a pull side at
  the tier's pull amount (250, 1000, 8000, 32 000 mB on ATM10) when all of it fits, each pipe's share saved as
  `fluid`, the network a fluid handler on each side for the blocks around it (normal and pull sides take in, normal
  and push give out); the large OBJ transmitter model as a multipart blockstate.
- Basic to Creative Fluid Tanks (`[fluid_tanks]`, `mekanism_energy::fluid_tank`, runtime `mek_fluid_tanks`), as the
  pipes' sources and sinks: one tank on every side that other blocks fill and drain without a rate limit, giving
  down at its output a tick while active (`active_state`; at most what a fluid tank below still takes), a creative
  tank keeping what it gives and voiding what it gets; saved as Mekanism saves it (`fluid_tanks`).
- Basic, Advanced, Elite and Ultimate Logistical Transporters (`[transmitters.items]`, `mekanism_energy::transporter`,
  runtime `mek_transporters`, hook points API 425): stacks travel through touching transporters as moving items,
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
- Factories are full blocks without Mekanism's shapes (`BlockShapes`), like the infuser. The infusing factory (and
  the compressing, injecting and purifying ones of the chemical machines) isn't ported, so an installer leaves the
  Metallurgic Infuser as it is; nor are tier installers on energy cubes, tanks and bins. Java refuses an installer
  while a player has the block's screen open; this port doesn't check.
- A creative energy cube's item without saved energy counts as full here, as its drops are; on Java one without the
  `mekanism:energy` component holds nothing (Mekanism's own creative cube items always carry it). Other energy
  items (batteries, Forge Energy items) don't charge machines yet.
- The infuser's infusion bar and the separator's gauges are the chemical's colour, where Mekanism tints its texture;
  chemical items other than the Chemical Tanks aren't ported. Per-tick infusing (`per_tick_usage`), which ATM10's
  recipes don't use, isn't ported. The separator's fluid sides stay at their default (every side input);
  `docs/mekanism-chemicals.md` lists the chemicals' own differences (tank slots, tube networks joining).
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
- The Configurator's mode is set with `/mek configurator`: Mekanism's mode key and sneak-scroll aren't ported. Its
  messages go to the chat, not the action bar. A transmitter's arms aren't hit (its outline is its core), so a
  sneaking use configures the side used, and on a transporter a connected side bumps the colour, as a use on
  Mekanism's core does. A plain use on a transmitter doesn't toggle redstone sensitivity (not ported); block
  security and the radiation check of the wrench aren't ported; the empty mode takes every slot the block keeps,
  its upgrade holder's too.
