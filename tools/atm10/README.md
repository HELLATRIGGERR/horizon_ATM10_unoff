Scripts that turn the ATM10 instance into carries and run the full pack: see docs/atm10-port.md, "The full pack".
They only read the instance. Paths come from the environment:

- `ATM10_INSTANCE`: the CurseForge instance (read only).
- `ATM10_WORK`: the `horizon-port-pack` work folder (`pack.json`, `inventory.json`, `all-keys.txt`, `oracle/`,
  `mods/<key>/carry/`, `leave-out.txt`, `play/`).
- `ATM10_DATA`: generated data beside it (the pack server's run folder, `dump/`).
- `HORIZON_NEOFORGE_HOME`: a NeoForge server of the instance's version; `HORIZON_ORACLE_JAVA`: Java 21 for it.
- `HORIZON_PORT_PACK`: the tool's exe; `HORIZON_RESOURCES`: prepared 26.3 resources.
- `HORIZON_REPO` (play-atm10.ts): the checkout whose game, resources and `content/ae2` are the defaults.
