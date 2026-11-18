# Torchmaster — a Horizon port

**Torchmaster** is a Minecraft mod. A minecraft mod. This repository is a **port of it to [Horizon](https://github.com/horizon-rust/horizon)**, the Rust engine for Minecraft Java 26.3. It carries the complete source of the port plus the content files it ships, and installs as a single `.hmod` package.

Upstream: [Xalcon/TorchMaster](https://github.com/Xalcon/TorchMaster) — licence **MIT**.

| | |
|---|---|
| Upstream mod | Torchmaster ([source](https://github.com/Xalcon/TorchMaster)) |
| Ported to | Horizon 0b14f8a, Minecraft Java 26.3, mod API `0.2` |
| Package | `torchmaster.hmod` (install it with the Horizon launcher) |
| Licence | **MIT** — the upstream mod's licence; see [Licence and credit](#licence-and-credit) |

## What this port implements

- **Mega Torch** (`torchmaster:megatorch`) vetoes natural spawns of every
- **Dread Lamp** (`torchmaster:dreadlamp`) vetoes natural spawns of the
- Both blocks are `EntityBlockingLightBlock`s: light level 15, wood sound,
- The default **`blockOnlyNaturalSpawns = true`** means only the natural spawn
- `IDistanceLogic.Cubic` checks `[torch − r, torch + r + 1]` on each axis. A
- **Reviewer:** pinned hash vs PR-body hash
- **Reviewer:** licence notice
- **Reviewer:** relative link

## Not yet ported

- **Spawners are deliberately not blocked.** Upstream's
- **Phantoms and village sieges.** Upstream also vetoes phantom spawns
- **Radii/config are fixed at the upstream defaults (64).** The brief asks for
- **Category-level veto.** The hook gives the spawn category, not the entity
- **Dread Lamp in-game phase not automated.** Its recipe/item/block are carried
- **Feral Flare Lantern, invisible light, frozen pearl, menu/GUI, volume
- **Light-level independence** is proven by placing the torch above the sealed
- **Block drop** is carried as the upstream loot table but not asserted in the

## Installing

1. Install the [Horizon launcher](https://github.com/horizon-rust/horizon/releases) (0.2.70 or newer).
2. Add `torchmaster.hmod` to the instance's mods.
3. The package is pinned to a specific Horizon build; the launcher checks that pin.

## Licence and credit

Torchmaster was created by its upstream authors; the code, models and assets in the original mod
remain theirs and are used here under **MIT**. The Rust in this repository is a clean-room
rewrite for Horizon, not a translation of the original source.

- `LICENSE` — the upstream licence text.
- `LICENSE-NOTICE.md` — what this port carries and where it came from.
- The upstream project: https://github.com/Xalcon/TorchMaster

## Technical details

```
horizon-mod.toml   package manifest (id, version, api, [native] pin)
Cargo.toml         the crate; depends on `horizon-mod-api` from the fetched SDK
src/               lib.rs (the one `impl Hooks`), runtime.rs (pure logic + tests), glue
assets/ data/      the content the package ships
tools/             content generators and the e2e scenario
```

- Build: `cargo horizon-mod build` (packages; it does not compile against the game).
- Test: `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- In-game: `ports/tools/launcher-game.sh run torchmaster` in the campaign tree, or install the
  `.hmod` and play; the harness scenario is `tools/e2e.sh`.

This repository is generated from the campaign's port at `ports/torchmaster/`; the port's own
`notes.md` is the full record (decisions, traps, evidence).
