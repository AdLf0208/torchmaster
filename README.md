# Torchmaster for Horizon

Port of **Torchmaster 21.1.14** (`Xalcon/TorchMaster`, MIT) for Minecraft Java
1.21.1 NeoForge, running on Horizon **`0b14f8a`** (Minecraft 26.3, mixins
`e59da90`). Local-only until the maintainer approves publishing
(ARCHITECTURE.md §8) — the licence is MIT, so the port is publishable with its
source.

One flat `.hmod` carries both halves: `assets/` and `data/` are the content
(two blocks, their items, recipes, loot, tags, models and textures, adapted from
the upstream MIT assets), `src/` is the native half.

| Block | What it does |
|---|---|
| `torchmaster:megatorch` | vetoes natural spawns of every non-friendly category (vanilla: `monster`) in a cube of 64 around it |
| `torchmaster:dreadlamp` | vetoes natural spawns of the friendly categories except `misc` in a cube of 64 |

The veto covers the **natural spawn path only**, as the original's
`blockOnlyNaturalSpawns` default asks: spawn eggs, breeding, spawners and
commands are untouched.

## Layout

```
horizon-mod.toml   the manifest and the only identity (api 0.2, [native] pin)
Cargo.toml         horizon-mod-api from .horizon/sdk/ + log
src/runtime.rs     the pure engine: kinds, cubic range, registry, TSV file (8 tests)
src/lib.rs         the one impl Hooks: potential_spawns, block_changed,
                   server_started, world_lifecycle, level_tick_post
assets/torchmaster/  models, blockstates, textures, lang
data/torchmaster/    block declarations, recipes, loot, creative tab
data/minecraft/tags/ the mineable tags the two blocks belong in
icon.png           128x128, from the publish tree
LICENSE, LICENSE-NOTICE.md   Xalcon's MIT text and the notice, inside the package
```

## Build and check

The SDK is fetched into `.horizon/sdk/` by any `cargo horizon-mod` command and
is never packed. Every step goes through the campaign's slot registry, never a
bare `cargo` in the port:

```sh
ports/tools/build-slot.sh run --job sdk-torch --port ports/torchmaster \
    --step test -- bash -c 'cd ports/torchmaster && cargo test'
ports/tools/build-slot.sh run --job sdk-torch --port ports/torchmaster \
    --step check -- bash -c 'cd ports/torchmaster && cargo fmt --check && cargo clippy --all-targets -- -D warnings'
ports/tools/build-slot.sh run --job sdk-torch --port ports/torchmaster \
    --step build -- bash -c 'cd ports/torchmaster && cargo horizon-mod build'
ports/tools/build-slot.sh run --job sdk-torch --port ports/torchmaster \
    --step check -- bash -c 'cd ports/torchmaster && cargo horizon-mod check torchmaster.hmod'
```

`cargo horizon-mod build` writes `target/horizon-mod/torchmaster.hmod`; copy it
to the port root — that is the delivered package. `cargo horizon-mod check`
validates the manifest, the pin and the content namespaces, but it does **not**
compile the mod or run its hooks; the compile is `cargo test`/`clippy` (the
crate is the mod) and the behaviour is what the e2e in `notes.md` recorded
before the format change.

Use `cargo fmt --check`, not `cargo fmt --all --check`: `--all` also walks the
path dependency in `.horizon/sdk/` and reports the SDK's own formatting.

See [`notes.md`](notes.md) for the behaviour, the hook mapping, the API-388
probe, the retired-format verification and the gap list.
