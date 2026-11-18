# Horizon mods

The mod docs live in the modding book: [introduction](modding/src/introduction.md), [installing and using mods](modding/src/start/installing.md), [making a content mod](modding/src/how-to/content-mods.md) and [publishing](modding/src/how-to/publish.md).

milk_fluid = false           # optional: turn on NeoForge's milk fluid (docs/mod-fluids.md)

When a player places the block, its template sets the template's properties
and `placement` says how each declared one is set, by one of these rules:

```json
{ "template": "minecraft:stone",
  "state_properties": { "facing": ["north", "south", "east", "west"] },
  "placement": { "facing": "horizontal_look" } }
```

| Rule | The value placed | As vanilla's |
|---|---|---|
| `horizontal_look_opposite` | toward the player, level: the opposite of where they look | furnace |
| `horizontal_look` | where the player looks, level | bed |
| `look_opposite` | toward the player: the opposite of the nearest of the six directions they look in | dispenser |
| `look` | the nearest of the six directions the player looks in | observer |
| `clicked_face` | the face clicked | end rod |
| `clicked_axis` | the clicked face's axis | log |
| `water` | `true` in a water source | any waterlogged block |
| `default` | the declared default | |

A property has every value its rule can set (the four level directions, all
six, `x` `y` `z`, or `false` `true`), or the block is refused. A declared
property `placement` leaves out takes the rule most Java blocks give it:
`horizontal_look_opposite` for a `facing` or `direction` of exactly the four
level directions, `look_opposite` for one of all six, `clicked_axis` for an
`axis` of `x`, `y` and `z`, and `water` for a boolean `waterlogged`. Any other
keeps its default.

A copy of one of vanilla's rails carries minecarts when the mod adds it to
`#minecraft:rails`, as vanilla asks of any rail (`BaseRailBlock.isRail`). A copy
of `minecraft:powered_rail` speeds carts up and stops them as that rail does,
and a copy of `minecraft:activator_rail` activates them.
Running Java mods unchanged needs a Java bridge, which is only ever a separate, optional download, never installed or enabled by default. The player decides whether to enable it. Promotion out of beta requires MSPT and memory benchmarks against the Java client with the same mods ([horizon-rust/horizon#13914](https://github.com/horizon-rust/horizon/issues/13914)).
