# Mekanism's chemicals

Mekanism 10.7 has one `Chemical` type (`mekanism.api.chemical`): what were gases, infuse types, pigments and slurries
in 10.6 are all entries of one registry, `mekanism:chemical`. Its machines hold them in tanks, its Pressurized Tubes
move them, its items carry them, Mekanism Generators burns hydrogen and fuses deuterium and tritium, and AE2 stores
them through Applied Mekanistics. ATM10 8.4 registers 112 chemicals: Mekanism's 64, All the Ores' 26 slurries,
Allthemodium's 6, Mekanism More Machines' (`mekmm`) 6, KubeJS's 4, Mekanism Generators' 3, XyCraft World's 2 and
Starbunclemania's source gas. Everything here lives in the Mekanism bundle (`pack/mods/mekanism`); the engine gained
one generic helper (hook points API 162); slices 2 and 3 need no hook point.

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
4. **More chemical machines.** The Chemical Oxidizer (items to chemicals), Chemical Infuser (two chemicals to one),
   Chemical Injection Chamber and Purification Chamber (item and chemical per tick, `ItemStackConstantChemical...`),
   Rotary Condensentrator (fluid and chemical both ways), the brine recipe's sodium and chlorine, the Gas-Burning
   Generator (hydrogen and ethene by their fuel attribute).
5. **Radiation.** Radioactive chemicals (`validated`): the radiation manager, a dumped or leaking radioactive chemical
   polluting the chunk, the Radioactive Waste Barrel, `#mekanism:waste_barrel_decay_blacklist`.
6. **Applied Mekanistics.** After AE2's key type registry (G6): `MekanismKey`, chemical cells, interfaces, the storage
   bus over `IChemicalHandler` (then a hook point, as fluids' API 160), Mekanistic Routers' and Pipez' chemical
   transfer.
7. **The rest of the pack's chemicals.** The carry declares each mod's chemicals in its own content (All the Ores',
   Allthemodium's, `mekmm`'s, KubeJS's in `atm10-pack`), and slurries meet the Chemical Washer and Crystallizer.

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
