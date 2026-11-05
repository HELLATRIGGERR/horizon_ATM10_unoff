# Mekanism's chemicals

Mekanism 10.7 has one `Chemical` type (`mekanism.api.chemical`): what were gases, infuse types, pigments and slurries
in 10.6 are all entries of one registry, `mekanism:chemical`. Its machines hold them in tanks, its Pressurized Tubes
move them, its items carry them, Mekanism Generators burns hydrogen and fuses deuterium and tritium, and AE2 stores
them through Applied Mekanistics. ATM10 8.4 registers 112 chemicals: Mekanism's 64, All the Ores' 26 slurries,
Allthemodium's 6, Mekanism More Machines' (`mekmm`) 6, KubeJS's 4, Mekanism Generators' 3, XyCraft World's 2 and
Starbunclemania's source gas. Everything here lives in the Mekanism bundle (`pack/mods/mekanism`); the engine gained
one generic helper (hook points API 162); slices 2 to 6 need no hook point.

## What there was

- The Metallurgic Infuser held its infuse type in a tank of its own (`machine::Tank`: an id string and an amount),
  knew the eight infuse types and their tints by name (`INFUSE_TYPES`), read a recipe's `{"tag": "mekanism:carbon"}`
  as the chemical of the tag's own name, and drew the infusion bar from that table. No other block held a chemical.
- Nothing carried a chemical's facts: their icons, tints, attributes (fuel, radioactivity, coolants: data maps since
  10.7.11) and tags were Mekanism's code and data, read by no tool.

## Design

**Declaration.** A content mod declares each chemical of its namespace as data, as a datapack registry would hold it:
`data/<ns>/mekanism/chemical/<path>.json` is the chemical `<ns>:<path>`. The engine's own declarations
(`data/<id>/horizon/`) take only the engine's kinds, and the chemical registry is Mekanism's, so it is not one of them.

```json
{
  "texture": "mekanism:liquid/liquid",
  "tint": "0xFFFFFFFF",
  "kind": "gas",
  "gaseous": true,
  "validated": false,
  "attributes": {"fuel": {"burn_time": 1, "energy": 200}},
  "tags": ["mekanism:gaseous"]
}
```

`texture` is the icon (`getIcon`, a block atlas sprite); `tint` is `getTint` as Java's int (`0xAARRGGBB`; Mekanism's
own infuse types leave the alpha 0), `color` `getColorRepresentation` when it differs (bio and fungi, whose icons are
already coloured: `ChemicalUtil.chemical` overrides it); `kind` is what the builder made it, by its icon
(`ChemicalBuilder.builder`, `infuseType`, `pigment`, `cleanSlurry`/`dirtySlurry`: `gas`, `infuse_type`, `pigment`,
`slurry`); `gaseous` is `isGaseous` (the builder's flag or `#mekanism:gaseous`); `validated` is
`hasAttributesWithValidation` as the pack runs (radiation on: the radioactive ones), which the default attribute
validator keeps out of ordinary tanks; `solid_tag` a slurry's item tag (`getOreTag`, the `chemical_solid_tag` data
map); `attributes` the final attributes under Mekanism's data map names (`chemical_attribute_fuel`'s `burn_time` and
`energy`, `radioactivity`, `cooled_coolant` and `heated_coolant` with the other variant, `thermal_enthalpy`,
`conductivity`, `temperature`), and `tags` its chemical tags. Data maps and tags are folded in as the pack ends up with
them (after KubeJS), so the registry needs no data map loader of its own; the carried tag files
(`data/<ns>/tags/mekanism/chemical/`) stay in the content as they are.

**Registry.** The bundle reads the declarations once a process (`mek_chemicals::registry`), listing them with the
engine's `native_data_files("mekanism/chemical")` (API 162: NeoForge's `ResourceManager.listResources` over the
content root, every namespace) and reading each with `native_data`. `mekanism_energy::chemical::ChemicalRegistry`
holds them sorted by id, numbers them from 1 in that order for menus' data slots (0 is none), and answers `get`, `is`
(a chemical's tags), `tagged` and the default attribute validator (`passes_default`). Both sides of a game read the
same content root, so a number means the same chemical on the client.

**Stacks.** `ChemicalStack` is Mekanism's: a chemical and a `long` amount, empty when it holds none,
`mekanism:empty` or 0 or less; `copyWithAmount`, `split`, `grow`, `shrink`, `isSameChemical`. Its saved form is
`ChemicalStack.CODEC`'s NBT `{id, amount}` (a long; any number reads), and it reads back as `parseOptional` does: an
id the registry lacks, `mekanism:empty` or an amount below 1 is an empty stack. A recipe's chemical (`{"id",
"amount"}`) is read strictly: such a stack fails, and the recipe with it. `ChemicalStackIngredient` reads Mekanism's
`mapCodecNonEmpty` (`{"chemical"}` or `{"tag"}`, not both, or a `type`: `mekanism:single`, `tag`, `compound`,
`difference`, `intersection`) beside a positive `amount`, and tests a stack by the registry's tags.

**Tanks.** `ChemicalTank` is `BasicChemicalTank`: what it holds and its capacity; its rules are passed in as they
read the machine around them (`TankRules`: `canExtract` and `canInsert` by automation type, and `isValid`, the
validator and the attribute validator). `insert` puts into an empty tank or one of the same chemical what fits when
the stack is valid and may come in that way; `extract` gives up to an amount when it may go out; `setStackSize`,
`growStack` and `shrinkStack` are Mekanism's. The constant predicates are `always`, `internal_only`, `not_external`
and `any`. Machines save their tanks as `chemical_tanks: [{tank: 0b, stored: {id, amount}}]`, the empty ones left
out and the list removed when all are.

**The handler.** Mekanism's `Capabilities.CHEMICAL.block()` is the `ChemicalHandler` trait (`IChemicalHandler`: the
tanks, what each holds and its capacity, `isValid`, insert and extract by tank, and `ChemicalUtils.insert`/`extract`
over the tanks as the default whole-handler forms: a stack fills the tanks holding its chemical first, then the empty
ones; an extraction drains the first chemical it finds across them). `SidedTanks` is one side of a Mekanism block as
`ConfigChemicalTankHolder` and `ProxyChemicalHandler` make it: the tanks its chemical side configuration puts there,
whether that side takes in and gives out (`ChemicalSlotInfo`), `AutomationType.handler(side)` (`EXTERNAL`), and the
null side, which reads every tank and changes nothing; a side whose type is `NONE` has no handler. The bundle answers
for its own blocks (`mek_chemicals::with_handler`, `mek_machines::chemical_handler`); no other native mod has
chemicals yet, so the capability needs no hook point. The ejector (`TileComponentEjector` with `ChemicalUtil.emit`)
splits a tank's contents, up to `chemicalAutoEjectRate` a tick, evenly among the handlers on the faces of its output
type (`EmitUtils.sendToAcceptors`, the energy split's port), as the block itself (`INTERNAL`).

**Chemical tanks and items.** `chemical_tank::TankBlock` is `TileEntityChemicalTank` with its
`ChemicalTankChemicalTank`: `tiers.toml` `[chemical_tanks]` sizes each tier (`<tier>Capacity`, `<tier>Output`); its
own moves (`INTERNAL`: its ejector, its slots) go at most the output a tick (`getInsertRate`/`getExtractRate`),
other blocks' (`EXTERNAL`) any amount (`ChemicalTank::insert_rated`/`extract_rated`). The creative tank simulates
every move (it keeps what it holds, gives without losing, and voids what another block puts in), and only its own
slot fills an empty one, to `Long.MAX_VALUE`. Dumping loses `storage / 400` a tick and stops the ejector; dumping
excess loses up to the output a tick down to `dumpExcessKeepRatio`. Its config is the item's
(`ItemBlockChemicalTank.SIDE_CONFIG`: chemicals `FRONT_OUT_EJECT`, items `FRONT_OUT_NO_EJECT`; none, input, output
and both), saved as `component_config` (`config2`, `eject2`, `config3`, `eject3`) beside `dumping` (an int). A
tank item keeps its tank as the block saves it, `custom_data.mekanism.chemical_tanks` (Mekanism's own
`mekanism:chemicals` component is one the game doesn't take), with its `dumping` and changed `component_config`
when they differ from the item's; an empty untouched tank drops a plain item. `chemical_tank::ItemTank` is the
item's handler (`ComponentBackedChemicalTankTank` behind `Capabilities.CHEMICAL.item()`): a stack of one only, every
move its own (`INTERNAL`) and at most its tier's output, a creative item keeping what it holds.
`ChemicalInventorySlot.fillChemicalTank` and `drainChemicalTank` are `fill_from_item` and `drain_into_item` over any
item handler and a machine's own tank (`OwnTank`: a chemical tank's rules or a plain tank's), with the slots'
insert and extract predicates (`fill_accepts`, `drain_accepts`, `fill_done`).

**Transmitters.** Pressurized Tubes are Mekanism's chemical transmitters, networks of the Universal Cables'
`network.rs` with a chemical (`Network::content`): a buffer as big as their tubes (`[transmitters.chemical]`
`<tier>Capacity`), one chemical at a time (`insert_of` takes only that one, any while the buffer is empty), two
networks or a tube's saved share joining only when compatible (`CompatibleTransmitterValidator`: one holds none or
both the same, the last chemical counting while it moved some last tick; tubes of two chemicals touch without
joining, and join once one empties: `Networks::mend`), a pull side drawing the tier's `<tier>PullAmount` (not its
capacity), each tube's share saved as `boxed_chemical`. They are kept in the bundle per dimension (`mek_tubes`),
through the cables' lifecycle hook points (API 156), and committed, looked at, pulled and emitted after each
level's tick; each side's acceptor is any block with a chemical handler on the touching side, and each tube is a
chemical handler for the blocks around it (normal and pull sides take in, normal and push give out, the null side
reads). Their OBJ model becomes a multipart blockstate as the cables' (`mekanism-content`'s `small_models`).

**AE2 (later).** Applied Mekanistics stores chemicals as an AE2 key type (`MekanismKey`, `AEKeyType` registered with
AE2's `AEKeyTypes`). The port's AE2 keys items and fluids by id and knows no other key type: AE2 first needs its key
type registry opened to mods (the AE2 stream's G6), then chemical cells, the ME Chemical Interface and the storage bus
on `IChemicalHandler` (which then becomes a hook point, as API 160 made fluid handlers one).

**From the NeoForge oracle.** The server oracle has a chemicals-only mode (`-Dhorizon.oracle.chemicals_only=true`,
`ChemicalFacts`): once the pack's data packs are loaded it writes every entry of `mekanism:chemical` (but the empty
one) by reflection (Mekanism is not on its class path): its class, icon, tint, bar colour, translation key,
radioactivity, `hasAttributesWithValidation`, `isGaseous` and the builder's flag, solid tag, its attributes
(`getAttributes`, each as its modern record: `ChemicalFuel`, `ChemicalRadioactivity`, `CooledCoolant`,
`HeatedCoolant`) and its tags; then it halts (about 5 minutes for ATM10 8.4). `mekanism-content --chemicals FILE`
(`MEK_CHEMICALS` in `pack/scripts/mekanism.ts`) declares the chemicals of the content mod's namespace from it and
copies their icons into the GUI atlas (`textures/gui/chemical/<icon ns>/<icon path>.png`).

## The first users

- **The Metallurgic Infuser** (`TileEntityMetallurgicInfuser`): its infusion tank is a `ChemicalTank` of 1000 mB
  (`createModern(1000, alwaysTrueBi, containsRecipeBA(input, type), containsRecipeB, ...)`, the default attribute
  validator) behind its chemical config (`setupIOConfig(CHEMICAL, tank, RIGHT)`: the right side `OUTPUT`, the rest
  `INPUT`, never ejecting). Its infusion slot fills it through `insert` (`fillTankOrConvert`: all of a conversion or
  nothing), recipes test it with their `ChemicalStackIngredient` (tags through the registry, not by name), an
  operation takes from it with the tank's own `extract`, the dump button empties it, and its bar takes the chemical's
  colour from the registry.
- **The Chemical Tanks** (slice 2): the Basic to Creative tanks as blocks (facing the player, `facing` horizontal;
  the carried model and shapes) and items, their ejector into the handlers on their output sides, the drain (`+`)
  and fill (`-`) slots, dump modes, side configuration (chemical and item pages, the chemical eject) and screen
  (`GuiChemicalTank`), the item's tooltip (what it holds, its capacity) and bar (its chemical's colour). Measured on
  the NeoForge oracle (`chem-work/tanks`, t1 and t2): basic 1000, advanced 16 000, elite 128 000, ultimate 512 000 mB
  a tick out of the front; a side of none or output takes nothing; dumping 160 a tick from 64 000 and no eject;
  dumping excess 1000 a tick to 54 400, ejecting; a creative tank of 5 mB gives 5 a tick and keeps them, voids what
  another tank ejects into it, and its fill slot fills it to `Long.MAX_VALUE`; slots move 1000 a tick between a
  basic tank and a basic item, 16 000 between an advanced tank and an ultimate item, 128 000 between an elite tank
  and an ultimate item, nothing into an item of another chemical or a stack of two; a creative item gives 100 a
  tick and keeps them; the drop keeps contents, dump mode and a changed config.
- **Chemical items in machines** (slice 2): the separator's left and right output slots fill a chemical item from
  their tanks (`ChemicalInventorySlot.drain`, 1000 a tick for a basic item: t4), and its item ejector
  (`TWO_OUTPUT_ITEM`, off by default) puts a full one out of the faces of its output type every 11 ticks; the
  infuser's infusion slot empties a chemical item into its tank before it converts items (`fillTankOrConvert`:
  1000 carbon a tick from a basic item).
- **The Pressurized Tubes** (slice 3): Basic to Ultimate. Measured on the oracle (t3, t4): the separator into three
  basic tubes and a basic tank delivers its 2 mB a tick a tick after making them (the tank has 4, 6, 8, 10 three to
  six ticks after placing, the tubes joining at the end of their first tick); an ultimate tank through one basic tube
  moves 4000 a tick, through three 12 000, an advanced 16 000, an elite 256 000, an ultimate all of the tank's 512
  000; a pull side draws 750 (basic), 2000 (advanced) a tick; a push side takes nothing in; a tank side of none
  doesn't connect; a separator with a full tank ejects 1024 a tick (`chemicalAutoEjectRate`) into a tube; three
  basic tubes holding 5000 save 1667, 1667, 1666; a hydrogen line takes no oxygen.
- **The Gas-Burning Generator** (slice 4, `TileEntityGasGenerator`, `mekanism_energy::gas`, runtime `mek_gas`): a
  `VariableCapacityChemicalTank` of `[gas_generator] tankCapacity` (18 000 mB) taking any chemical with a fuel
  attribute (`HAS_FUEL`; `notExternal` out, anything in) on every side but the front (`ChemicalTankHelper.forSide`),
  its fuel slot a `ChemicalInventorySlot.fill`. Each tick, after the ejector (out of its front at `maxOutput`, twice
  what it made last tick, at least twice hydrogen's Joules a tick, the tank's fuel's when loaded) and the slot, it
  burns `min(needed / rate, ceil(256 * stored / capacity), ticks left)` burn ticks of the fuel (`burnTicks` a mB,
  `energyPerTick` each: hydrogen 1 and 200 on ATM10, ethene 20 and 300) into a store of 1000 times hydrogen's
  energy density (200 000 J); a tick's Joules that don't fit rest it. What it is burning (`burnTicks`, `maxBurnTicks`,
  `generationRate`) isn't saved. Measured on a NeoForge 1.21.1 server with Mekanism and Mekanism Generators 10.7.19
  and ATM10's configs (`chem-work/m4`, g1 and g2): 1000 mB of hydrogen burns 15, 15, 14, 14 mB a tick (3000 J each),
  18 000 mB 256 then 253; 1500 J of room take 7 mB; an energy cube on its front gets 400, 102 400, 99 000, 49 800 J;
  hydrogen from a tank on its side goes in, oxygen doesn't; a basic tank item in its fuel slot gives 1000 mB a tick.
- **The Chemical Oxidizer** (slice 4, `TileEntityChemicalOxidizer`, `mekanism_energy::chemical_machine`, runtime
  `mek_chem_machines` beside `mek_machines`): a progress machine of 100 ticks (`chemicalOxidizer`: 200 J a tick,
  80 000 J) turning an item into a chemical (`mekanism:oxidizing`, `ItemStackToChemicalRecipe`) in a 10 000 mB
  `BasicChemicalTank.output`; a tank without room for the output waits with its progress. Its chemical config is
  `setupOutputConfig(CHEMICAL, tank, RIGHT)` (`CHEMICAL_OUT_MACHINE`'s `RIGHT_OUTPUT`, ejecting `chemicalAutoEjectRate`
  a tick), its output slot a `ChemicalInventorySlot.drain`. Measured (`chem-work/m4` o1): a sulfur dust makes 100 mB
  of sulfur dioxide on the hundredth tick of 200 J; 8 speed and 8 energy upgrades take 6 ticks of 3200 J; it ejects
  1024 mB a tick out of its right (west, facing north); its output slot fills a basic tank item 1000 mB a tick.
- **The Chemical Infuser** (slice 4, `TileEntityChemicalInfuser`): two `inputModern` tanks (left and right, 10 000
  mB, each taking what a recipe takes with what the other holds, `EitherSideChemical.containsRecipe`) and a centre
  `output` tank, `mekanism:chemical_infusing` recipes (`ChemicalChemicalToChemicalRecipe`, found and run either way
  round), `2^speed` operations a tick of `chemicalInfuser` Joules (200, times the speed and energy upgrades' factor)
  each, as far as the Joules, both inputs and the output's room allow; `CHEMICAL_INFUSING` config (left `INPUT_1`,
  right `INPUT_2`, front `OUTPUT`, the centre ejecting), its left and right slots `fill`, its output slot `drain`.
  Measured (o1): hydrogen and chlorine make 1 mB of hydrogen chloride for 200 J a tick either way round; 3 speed
  upgrades 8 operations of 1600 J; 8 as far as 80 000 J go (one of 51 200 J); a full output, an empty input or 150 J
  stop it; the centre ejects out of its front; its right took oxygen and then its left no chlorine.
- **The advanced machines** (slice 4, `TileEntityAdvancedElectricMachine`, `mekanism_energy::advanced`, three more
  `MachineKind`s in `mek_machines`): the Osmium Compressor (`mekanism:compressing`, 100 J, 80 000 J), the
  Purification Chamber (`mekanism:purifying`, 200 J, 80 000 J) and the Chemical Injection Chamber
  (`mekanism:injecting`, 400 J, 160 000 J): an item and a chemical in, an item out, 200 ticks; a tank of 210 mB
  (`createModern(210, ..., containsRecipeBA(input, gas), containsRecipeB)`) filled by its chemical slot
  (`fillOrConvert`: a chemical item, else a `mekanism:chemical_conversion` that fits whole: an osmium ingot 200 mB,
  flint 10 of oxygen, salt 2 of hydrogen chloride). A `per_tick_usage` recipe (all of ATM10's) takes its amount times
  a multiplier drawn each tick (`ItemStackConstantChemicalToObjectCachedRecipe`), the last tick's with the item
  (`finishProcessing`); `used_so_far` is saved and starts over only when an operation ends. The compressor's
  multiplier is `constantUse` of `baseTotalUsage` (200, or `round(200 * 16^(speed/8 - chemical/8))` for a machine
  taking chemical upgrades): what is left spread over the ticks left, so an ingot takes 200 mB whatever its speed and
  energy upgrades. The chambers take chemical upgrades and, with `randomizedConsumption` (machine-usage.toml; ATM10's
  file predates it, so Mekanism's default, on), draw `StatUtils.inversePoisson(16^(2 speed/8 - chemical/8))` a tick:
  0 to 3 mB with no upgrades (0.91 on average); speed upgrades raise what an operation takes (8 of them: 256 a tick
  on average, more than the tank, so it stalls), chemical upgrades lower it, energy upgrades change nothing. A tank
  short of the recipe's amount or of the tick's draw waits with the progress. The compressor's chemical config is
  `setupIOConfig` (`OUT_NO_EJECT`: its right gives, the rest take), the chambers' `setupInputConfig` (every side
  takes, nothing gives: `allowExtractingChemical` is off while they draw at random). Measured (`chem-work/m4` a1): the
  compressor 1 mB a tick for 200 ticks of 100 J, 16 a tick for 12 with 8 speed upgrades, 4 a tick for 50 with 4
  speed and 8 energy; an empty tank keeps the progress; an ingot goes in only when all 200 fit; the purification
  chamber took 186 and the injection chamber 165 mB in 200 ticks, 0 to 3 a tick; with 8 speed upgrades nothing in 24
  ticks; with 8 chemical upgrades 16 in 200.
- **The Chemical Dissolution Chamber** (slice 5, `TileEntityChemicalDissolutionChamber`, `mekanism_energy::ore`,
  runtime `mek_ore_machines` beside `mek_machines`): a progress machine of 100 ticks (`chemicalDissolutionChamber`:
  400 J a tick, 160 000 J), an item and a chemical into a chemical (`mekanism:dissolution`,
  `ChemicalDissolutionRecipe`): an input tank of 10 000 mB (`inputModern(containsRecipeBA(input, gas),
  containsRecipeB)`, filled by its chemical slot's `fillOrConvert`) and an output tank of 10 000 mB
  (`BasicChemicalTank.output`, its output slot a `ChemicalInventorySlot.drain`). ATM10's recipes (All the Ores' 39:
  an ore, three raw ores or a raw block and 1 or 2 mB of sulfuric acid a tick into 1000, 2000 or 6000 mB of its dirty
  slurry) are all `per_tick_usage`: `ItemStackConstantChemicalToObjectCachedRecipe.dissolution` takes the recipe's
  amount times `StatUtils.inversePoisson(getGasPerTickMeanMultiplier)` a tick (`advanced::process`), always (the
  chamber's multiplier doesn't look at `randomizedConsumption`), the last tick's with the item; `used_so_far` is
  saved. It takes chemical upgrades (`DEFAULT_ADVANCED_MACHINE_UPGRADES`); its configs are `DISSOLUTION`'s (items
  `EXTRA_MACHINE`, its bottom the chemical slot; chemicals `OUT_EJECT`: its right gives the output tank, ejecting,
  the rest take into the input tank). Measured (`chem-work/m5` c1, Mekanism's own iron recipes): an iron ore makes
  1000 mB of dirty iron on the hundredth tick of 400 J, its acid drawn 0 to 3 mB a tick (91 in 101 ticks); 8 speed
  and 8 energy upgrades 6 ticks of 6400 J and 251 to 281 mB a tick; 8 chemical upgrades 9 mB in 100 ticks; no acid
  keeps the progress.
- **The Chemical Washer** (slice 5, `TileEntityChemicalWasher`): a recipe machine without progress, `2^speed`
  operations a tick (`baselineMaxOperations`) of `chemicalWasher` Joules (200, 80 000 J) each, as far as the Joules,
  its water tank (10 000 mB, `BasicFluidTank.input(containsRecipeAB(fluid, slurry), containsRecipeA)`: a fluid handler
  on its fluid input sides, every side by default, that nothing drains), its slurry tank (10 000 mB, `inputModern`)
  and its output tank's room allow (`TwoInputCachedRecipe.fluidChemicalToChemical`, `ore::wash`); `mekanism:washing`
  recipes (All the Ores' 13: 5 mB of a dirty slurry and 25 of water into 3 of the clean one). Its fluid slot empties
  a container into the tank (`FluidInventorySlot.fill`, `fillTank(outputSlot)`: the empty container to the slot
  below), its slurry output slot drains the output tank into a chemical item; configs `WASHER` (items `MACHINE`,
  chemicals `OUT_EJECT`, fluids `INPUT_ONLY`). Measured (c1): 1 mB of dirty iron and 5 of water into 1 of clean iron a
  tick for 200 J; 3 speed upgrades 8 operations of 1600 J; 8 speed one of 51 200 J as far as 80 000 J go; a water
  bucket in its fluid slot adds 1000 mB the same tick and the bucket goes to the slot below.
- **The Chemical Crystallizer** (slice 5, `TileEntityChemicalCrystallizer`): a progress machine of 200 ticks
  (`chemicalCrystallizer`: 400 J, 160 000 J), a chemical into an item (`mekanism:crystallizing`,
  `OneInputCachedRecipe.crystallizing`: the recipe's mB taken with the item on the last tick), its tank 10 000 mB
  (`inputModern(containsRecipe)`) filled by its chemical slot; configs `CRYSTALLIZER` (items `MACHINE`, chemicals
  `INPUT_ONLY`). All the Ores' 13 recipes make its crystals from 200 mB of a clean slurry. Measured (c1): 200 ticks of
  400 J, a crystal and 200 mB on the 200th; 8 speed and 8 energy upgrades 12 ticks of 6400 J; 150 mB does nothing.
- **The Rotary Condensentrator** (slice 5, `TileEntityRotaryCondensentrator`, `mekanism_energy::rotary`): a
  chemical tank and a fluid tank of 10 000 mB and a mode (`mode`): condensentrating (the default) turns its chemical
  into the fluid of a `mekanism:rotary` recipe (Mekanism's 10, 1 mB into 1), decondensentrating the fluid back,
  `2^speed` operations a tick of `rotaryCondensentrator` Joules (50, 20 000 J). Its tanks take and give by the mode
  (`createModern`: the chemical tank takes while condensentrating and gives while decondensentrating, the fluid tank
  the other way round; the machine itself fills its output, a player empties either); its configs `ROTARY` (chemicals
  `OUT_EJECT_LEFT`, fluids `OUT_EJECT`, both `setupIOConfig(..., true)`: every type takes and gives, the tanks
  decide), the chemical ejecting while decondensentrating; its chemical slots drain and fill chemical items and its
  fluid slot fills or empties a container by the mode, the container to the slot below; the mode button toggles it
  (`NEXT_MODE`). Measured (c1): 1 mB of hydrogen into liquid hydrogen a tick for 50 J, 3 speed upgrades 8 of 400 J,
  1 mB a tick back.
- **The chemical factories** (slice 6, `TileEntityItemStackChemicalToItemStackFactory`, `mekanism_energy::factory`
  on `advanced`'s rules, runtime `mek_factories`): the Basic to Ultimate compressing, purifying, injecting and
  infusing factories, 3 to 9 processes of their machine's recipes on one store of `max(storage / 2, usage) *
  processes` Joules (`Factory.setMachineData`) and one chemical tank of 210 mB a process, the infusing factory's 1000
  (`getInitialChemicalTanks`), filled by a chemical slot after the processes (`fillOrConvert`, slot 6 of a basic
  one), its use so far by process (`used_so_far`, a long array). A per-tick recipe
  (`ItemStackConstantChemicalToObjectCachedRecipe`) takes each process's use a tick out of the shared tank in process
  order, a process short of it waiting with its progress: `constantUse` of `baseTotalUsage` for compressing,
  `inversePoisson` of the mean multiplier for purifying and injecting (`useStatisticalMechanics`:
  `randomizedConsumption` and those two); an infusing recipe takes its amount with the item, and a process short of
  it starts over (its ingredient matches nothing below its amount: `mismatchedRecipe`). The compressing and infusing
  factories' tank gives on their output sides (`createModern`, `setupIOConfig(CHEMICAL, tank, RIGHT)` without
  ejecting), the purifying and injecting ones' only takes (`inputModern`, `setupInputConfig`); their side configs
  and upgrades are their machines' (`ADVANCED_MACHINE`, `ADVANCED_MACHINE_INPUT_ONLY`; the chambers' factories take
  chemical upgrades), their screen `GuiFactory` 11 pixels taller with the chemical bar (7, 76) and its dump button.
  Tier installers turn the Osmium Compressor, Purification Chamber, Chemical Injection Chamber and Metallurgic
  Infuser into their basic factories, and a factory into the next tier, with the tank, the chemical slot and the use
  so far (`AdvancedMachineUpgradeData`: the machine's into process 0). Measured on the oracle (`chem-work/m6` f1 to
  f3): basic compressing and purifying factories store 120 000 J, injecting 240 000, infusing 30 000 (an ultimate
  three times that); an osmium ingot's 200 mB fill a basic compressing factory from 430 to 630 and not from 431, an
  ultimate one's to 1890, redstone's 10 mB a basic infusing one's to 3000 and an ultimate's to 9000; a basic
  compressing factory with a dust a process takes 1 mB and 100 J a process a tick (`used_so_far` `[1, 1, 1]`) and
  gives three ingots on the 200th tick, 30 of its 630 mB left; with 8 speed upgrades 16 mB and 25 600 J a process,
  only the first process on the second tick, then none (short of Joules); 2 mB run the first two processes, the third
  waits; the purifying and injecting factories took 528 and 544 mB in 200 ticks, 0 to 3 a process a tick, about 1/16 on
  average with 8 chemical upgrades; a basic infusing factory takes 10 mB with each item on the 200th tick, and with 15
  mB the first process finishes and the others start over.
- **The Electrolytic Separator** (`TileEntityElectrolyticSeparator`): a fluid tank of 24 000 mB
  (`BasicFluidTank.input(containsRecipe)`, a fluid handler of API 160 on its fluid config's input sides, every side
  by default; nothing drains it), two output tanks of 2400 mB (`BasicChemicalTank.output`), `mekanism:separating`
  recipes (a `SizedFluidIngredient` in, two `ChemicalStack`s out, an `energy_multiplier`), `2^speed` operations a tick
  of `2 * hydrogen's energy density * multiplier` Joules each (`FixedUsageEnergyContainer`: hydrogen's fuel data map,
  200 J/mB on ATM10, so 400 J), as many as the Joules, the input and each output's room allow; dump modes by tank
  (`GasMode`: idle, dumping excess down to `dumpExcessKeepRatio` at most `chemicalAutoEjectRate` a tick, dumping
  `8 * 2^speed` a tick), the machine stopping when both tanks dump excess and are at their target; its chemical config
  `TWO_OUTPUT` (left `OUTPUT_1`, right `OUTPUT_2`, ejecting), item config `TWO_OUTPUT_ITEM`; a water bucket in its
  fluid slot empties into the tank. Its numbers were measured on a NeoForge 1.21.1 server with Mekanism 10.7.19 and
  ATM10's configs over RCON (`chem-work/separator`, outside the checkout).

## Slices

1. **The registry, stacks, tanks, the handler; the infuser and the separator (done).** As above. ATM10 8.4 (5
   November 2026): the chemicals-only run gives 112 chemicals in 8 namespaces (58 of a liquid icon, 28 slurries, 18
   pigments, 8 infuse types; 14 gaseous, 4 radioactive and validated, 3 fuels, one coolant pair); the content mods
   declare Mekanism's 64 and Generators' 3. Tests: `mekanism-energy` (`chemical::tests`: declarations, the registry and
   its numbers and tags, stacks and their saved and recipe forms, the tank's rules, the handler's order, ingredients,
   the ejector's split; `separator::tests`: the NeoForge measurements; `menu`, `machine`, `config`),
   `mekanism-content` (the declaration from the oracle's facts), the game's `native_hooks` (`data_files`), and the
   world tests `electrolytic-separator` and `infuser-chemical-tank`.
2. **Chemical tanks and chemical items (done).** As above: `chemical_tank` (the block's and the item's tanks, the
   slots' moves), `config` (`[chemical_tanks]`), `menu::TankView`, the runtime `mek_tanks` (blocks, items, screen)
   and the separator's and infuser's chemical item slots. Tests: `mekanism-energy` (`chemical_tank::tests`: the
   oracle's tank, dump, creative and slot numbers; `config`, `menu`, `tier`), the world test `chemical-tank`.
3. **Pressurized Tubes (done).** As above: `network` (content, compatibility, `mend`, pull amounts), `config`
   (`[transmitters.chemical]`), the runtime `mek_tubes`, `mekanism-content`'s tube models. Tests: `mekanism-energy`
   (`network::tests`: one chemical a network, two that touch without joining, the oracle's shares),
   `mekanism-content` (`transmitter::tests`), the world test `pressurized-tube`.
4. **More chemical machines.** The Gas-Burning Generator (done: as above; the world test `gas-burning-generator`,
   `mekanism-energy`'s `gas::tests`), the Chemical Oxidizer (items to chemicals) and Chemical Infuser (two chemicals to
   one) (done: as above; the world test `chemical-oxidizer-infuser`, `chemical_machine::tests`, `machine` and `menu`),
   the Osmium Compressor, Purification Chamber and Chemical Injection Chamber (done: as above; the world test
   `advanced-machines`, `advanced::tests`, `machine`, `config`).
5. **The ore chain and the Rotary Condensentrator (done).** As above: the Chemical Dissolution Chamber, Chemical
   Washer and Chemical Crystallizer (`ore`), the Rotary Condensentrator (`rotary`), four more `MachineKind`s run by
   `mek_ore_machines` and `mek_rotary` on `mek_machines`' block entity, energy, config, handlers and probe; the fluid handler of
   API 160 for the washer's and the rotary's fluid tanks. All the Ores' slurries are declared by the content step in a
   content mod of its carry (`mekanism-content` with the oracle's facts and no overlay of its own:
   `MEK_ATO_CONTENT_DIR`), so its 65 recipes of the chain load. Tests: `mekanism-energy` (`ore::tests`,
   `rotary::tests`: the oracle's numbers), `mekanism-content` (another carry's chemicals), the world tests `ore-chain`
   (an ore through the three into All the Ores' crystals) and `rotary-condensentrator`. Mekanism has no factory of
   the ore chain's machines.
6. **The chemical factories (done).** As above: `factory`'s compressing, purifying, injecting and infusing types
   and their tank, slot and per-process use, `mek_factories` running them on `advanced::process`, their installers
   from the four machines, their screens and chemical handler. Tests: `mekanism-energy` (`factory::tests`), the world
   test `chemical-factories`.
7. **Radiation.** Radioactive chemicals (`validated`): the radiation manager, a dumped or leaking radioactive chemical
   polluting the chunk, the Radioactive Waste Barrel, `#mekanism:waste_barrel_decay_blacklist`.
8. **Applied Mekanistics.** After AE2's key type registry (G6): `MekanismKey`, chemical cells, interfaces, the storage
   bus over `IChemicalHandler` (then a hook point, as fluids' API 160), Mekanistic Routers' and Pipez' chemical
   transfer.
9. **The rest of the pack's chemicals.** The carry declares each mod's chemicals in its own content (All the Ores',
   which the content step declares for now, Allthemodium's, `mekmm`'s, KubeJS's in `atm10-pack`).

## Differences from Java

- The registry is read once a process from the content's declarations: a world data pack that changes chemical tags
  or data maps doesn't change them here, and the registry's numbers (menus) are this content's order.
- Gauges and the infusion bar are filled with the chemical's colour (`getColorRepresentation`), not its icon tinted.
- The separator's fluid config is fixed at its default (every side `INPUT`): the side configuration window has no
  fluid page yet. Hoppers put chemical items into its output slots but take none out (the item ejector does, once
  full): no hook point says when a slot gives.
- Its fluid slot empties a container only when it is alone in the slot.
- The fluid gauge is water's blue or a mod fluid's tint, not the fluid's texture.
- Redstone control stays IGNORED, as for the other machines.
- The Gas-Burning Generator's energy slot charges nothing (items keep no energy yet); its max output after a load is
  twice its tank's fuel's Joules a tick, as Java's `recheckOutput`, and a fill by another block into an empty tank is
  noticed at its next tick, not at once.
- The oxidizer's and chemical infuser's output slot items leave by their item ejector once full (as the
  separator's); hoppers take none out.
- The chambers' chemical config is the randomized one whatever `randomizedConsumption` says (with it off Mekanism
  gives them the compressor's `setupIOConfig`); their draws come from the bundle's own random source, not the game's.
- A machine short of a tick's Joules waits without looking at its inputs (Java may still reset the progress when
  its errors change that tick).
- A chemical tank's slots take only chemical tank items (alone; the fill slot one holding a chemical), whatever the
  tank holds, as `native_block_entity_slot_accepts` sees no block entity; hoppers fill them and take nothing out.
  The infuser's infusion slot takes any tank item holding a chemical. No other chemical items (jetpacks, scuba
  tanks, gauge droppers) are ported.
- A tank item keeps its contents in `custom_data.mekanism` (as the energy cube's), not `mekanism:chemicals`; its
  redstone control, security and owner aren't ported.
- A tank's outline and collision are one box around its body and valve (a declaration's shapes by property need
  the template's own property, and the block entity's template, the beacon, has no `facing`).
- Two tube networks that become compatible (one empties) join at the next look (within a second), where Java's
  `recheckConnection` runs on the change. A tube touching two networks of different chemicals joins the bigger (the
  last of equals); Java's validator takes the first it finds.
- A network's acceptors are sorted by position (Java: `HashMap` order); only who gets a leftover mB differs. Which
  of two tanks ejecting different chemicals into an empty network first gets in follows the port's block entity
  order, not Java's. Radiation, the tubes' contents rendering and redstone sensitivity aren't ported.
- The washer's and the rotary condensentrator's fluid configs stay at their defaults (no fluid page). The rotary's
  fluid isn't ejected (no helper fills another block's fluid handler): it leaves by a block that drains it (AE2's
  buses) or its fluid slot; its chemical fill slot's emptied items stay for the player or a hopper. The washer's two
  fluid slots sit beside the panel below the upgrade holder (Mekanism: at (180, 71) and (180, 102)); the rotary's
  mode button is a plain button with an arrow.
- The dissolution chamber's draws come from the bundle's own random source. A container in the washer's or the
  rotary's fluid slot moves to the slot below one at a time, as Java's; a stack of mod containers is filled or emptied
  one item a tick.
- All the Ores' slurries come from a content mod the content step makes of its carry (its namespace, its chemicals
  from the oracle's facts), not from the carry itself (slice 9).
- The chemical factories' random draws come from the bundle's own random source, as the chambers'. Their chemical
  slot takes only chemical tank items holding a chemical (alone) and what converts to a chemical their recipes
  take; hoppers fill it and take nothing out.
