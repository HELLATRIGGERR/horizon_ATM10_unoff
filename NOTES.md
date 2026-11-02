# AE2 more screens: working notes
Branch feat/ae2-more-screens, target C:\horizon-work\target-screens, shots in .build/shots-more.
Scripts: .build/screens-build.sh, screens-check.sh, screens-test.sh <suite>, screens-look.sh <name> (look-<name>.ts in the scratchpad; look-craft.ts, look-parts.ts).
The scratchpad is shared with other workers: name patch files screens-*.

Approach: the placeholder menu keeps its places, so click-slot still reaches them (ae2_hidden_click for the hidden ones). The screen draws from the style and a JSON view payload, and presses by menu place or by an index place past the menu (100.. = filters).

Part 1, crafting screens: ae2_widgets.rs, ae2_craft_screens.rs, ae2:craft_view. machine-screens.ts has the flow.
Part 2, parts: ae2_part_screens.rs, ae2:part_view from ae2_bus_ui.rs (ae2_part_view_of, ae2_part_place). Storage bus clear and partition, filters by index, priority tab (PriorityTarget::Part), import bus key types (part data key_types, MixedHost filters), level emitter text through the energy editor channel, Interface stock amount as SetStockAmountScreen in ae2_priority_screen.rs. Annihilation plane has no screen in AE2 19.2.18. Suite: part-screens.ts.
Part 3: ae2_status_screens.rs (power unit, network status table, anchor, pattern access chrome; ae2:status_view, ae2:anchor_view), ae2_workbench_screen.rs (63 filters, toolbar; ae2:workbench_view), ME Chest layout: side faces open me_chest.json through the machine menu, top opens the terminal (tests changed to interact up).

Machine load: "Control timeout: status" means the machine was overloaded, retry.

Still different from AE2:
- Fluid filters on bus and workbench show the bucket or wrapped item, not the fluid sprite.
- No toolbox panel on part screens.
- Network status: no power input or channel power line (the port does not track them), no missing-channel red cell, no Export Grid button.
- Pattern access: the handle shows the page, a press above or below the middle pages; no drag; no terminal style button.
- The workbench's fuzzy button only cycles forward, upgrades stay at 4 slots at most.
- ME Chest top face assumes up; a rotated chest is not handled.
- set_processing_pattern_amount still uses the old menu.
