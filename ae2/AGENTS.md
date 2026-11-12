# Native AE2 for Horizon

Follow the root CONTRIBUTING.md and preserve upstream licenses. This is an experimental source-derived AE2 port; never equate registered assets or passing helper tests with full parity or ATM10 support.

Keep gameplay in Rust and preserve physical item/fluid custody, save behavior, power/channel rules, and the normal integrated-server path. Coordinate shared Cargo work. Automated clients must use --background and isolated profiles, never capture the real pointer or system clipboard/controller, and render at 30 FPS, one fixture at a time unless a run asks for more (`test-world.ts --jobs N`, `AE2_RENDER_SLOTS`). Only open a visible game when explicitly requested.

Do not commit proprietary game files, prepared resources, downloaded assets/JARs, personal worlds/profiles, or secrets. New developer scripts use TypeScript and Bun. The retained Horizon subtree contains upstream documentation; the root policy governs this source-derived port.
