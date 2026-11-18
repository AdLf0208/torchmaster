# Torchmaster — port notes (P1 content/logic, slice 1)

Ported from **Torchmaster** (`torchmaster`, MIT, Xalcon/TorchMaster;
`gradle.properties` version `21.1.14`, Minecraft Java 1.21.1 NeoForge) for
Minecraft 26.3 on Horizon pin `1d63e0a5` (mixins `5a64af93`). Local-only until
the maintainer approves publishing (ARCHITECTURE.md §8, licence rank 0).

Status: **ported-partial — slice 1 (mega torch + dread lamp natural-spawn
veto)**. The pre-spawn veto that blocked the port at `41f822b` is now expressible
through `potential_spawns` v1 (API 388); the probe and the in-game run are
recorded below.

**Format (2026-11-18):** the live port is the flat **SDK package** at nightly
`0b14f8a` (mixins `e59da90`, API `0.2`) — one `.hmod` with `horizon-mod.toml`,
`src/lib.rs`, `src/runtime.rs`, `assets/` and `data/`. The body below is the
history of the retired `.hnmod` bundle that PR #609 submitted; the migration,
the hook mapping and the new numbers are in
[§ Migration to the SDK format](#migration-to-the-sdk-format-2026-11-18-nightly-0b14f8a)
at the end. The retired bundle itself is archived in
`ports/_shared/evidence/torchmaster/retired-hnmod/`.

## Class and tier

`content` / `logic` (the ledger row). The port has a content half (two blocks,
their items, recipes, loot, tags, models/textures) and a native logic half (the
placed-light registry and the spawn veto); no machine or client half is needed
for slice 1. The feral-flare lantern/menu, invisible light, frozen pearl and the
feral-lantern spawner interactions are outside the pre-spawn veto and are a
later slice.

## Behaviour (public docs + upstream source)

Analysis clone: `https://github.com/Xalcon/TorchMaster` at
`a11226c866bf4e9f0695621db5971418dffe3014` (`2026-10-14`, version
`21.1.14`), read from `common/`, `neoforge/` and the carried assets; the clone
was deleted after the notes (as required by AGENTS.md).

1. **Mega Torch** (`torchmaster:megatorch`) vetoes natural spawns of every
   **non-friendly** entity category (vanilla: `monster`) in a cube of
   `megaTorchRadius` (default **64**) around it. Source:
   `logic/entityblocking/megatorch/MegatorchEntityBlockingLight.java:27-33`,
   `compat/VanillaCompat.registerTorchEntities`, `logic/IDistanceLogic.Cubic`,
   `TorchmasterNeoforgeConfig.java:67-70`.
2. **Dread Lamp** (`torchmaster:dreadlamp`) vetoes natural spawns of the
   **friendly** categories except `MISC` (`creature`, `ambient`, `axolotls`,
   `underground_water_creature`, `water_creature`, `water_ambient`) in a cube of
   `dreadLampRadius` (default **64**). Source:
   `logic/entityblocking/dreadlamp/DreadLampEntityBlockingLight.java:24-30`,
   `compat/VanillaCompat.registerDreadLampEntities`.
3. Both blocks are `EntityBlockingLightBlock`s: light level 15, wood sound,
   hardness 1.0, and their shapes are a centred 4×16×4 post (mega torch) and a
   14×14×14 cage (dread lamp). Placement registers the light and removal
   unregisters it (`blocks/EntityBlockingLightBlock.java:72-88`).
4. The default **`blockOnlyNaturalSpawns = true`** means only the natural spawn
   path is vetoed; spawn eggs, breeding, spawners and commands are untouched
   (`events/TorchmasterEventHandler.java:31-70`). This matches the pin's
   `potential_spawns` call site (`NaturalSpawner.mobsAt`), so the port can be
   faithful without a general `MobSpawnEvent` surface.
5. `IDistanceLogic.Cubic` checks `[torch − r, torch + r + 1]` on each axis. A
   mob's x/z are block centre (`x + 0.5`) and its y is the feet block's y, so
   the reachable whole-block cells are `[torch − r, torch + r]` on x/z and one
   extra layer up on y; the engine reproduces that (see `in_range`).

There is **no plain “torch” block** upstream: the mod's two radius blocks are
the Mega Torch and the Dread Lamp. Slice 1 therefore carries both; the brief's
"torch + mega torch" is read as the two blocking lights (the only two
radius-bearing blocks).

## Hooks

| Hook | Helper | What it does |
|---|---|---|
| `potential_spawns` (v1, HEAD, `cancel`) | `potential_spawns.rs` | Natural spawning builds the possible list at `pos`; a matching light in range sets `canceled`, which empties the list. |
| `block_changed` (v1, HEAD) | `block_changed.rs` | A torch/lamp placed, replaced or removed refreshes the in-memory registry from `old`/`new` state names. |
| `world_lifecycle` (v1, HEAD) | `world.rs` | `WorldOpenReady` loads `<world>/torchmaster/lights.tsv`; close saves it. |
| `level_tick_post` (v1, HEAD) | `tick.rs` | Every 100 ticks prunes lights whose block is gone (loaded chunks only) and flushes a changed file. |

All four call sites exist at the pin: `game/native_hooks.rs:6303`
(`native_potential_spawns`), `:1210` (`native_block_changed`, on `GameLevel`),
`:2403` (`native_world_lifecycle`), `:2583` (`native_level_tick_post`).

## Pure engine (`runtime.rs`, 8 tests)

- `LightKind::of_block` / `block` — only `torchmaster:megatorch` and
  `torchmaster:dreadlamp`.
- `LightKind::blocks_category` — mega = `monster`; dread = every friendly
  category except `misc`.
- `in_range` — upstream's cubic test with block-centre x/z and the +1 y layer;
  radius 64 is the upstream default.
- `blocks` — dimension + category + range membership over the light list.
- `upsert` / `remove` — one entry per dimension/position (a swapped block keeps
  one), removal by position.
- `parse_lights` / `format_lights` — the TSV world file; malformed lines are
  ignored.

## The source-mixin harness gate (important)

The pin's `NaturalLevel::spawn_hooks` (`game/natural_spawning.rs:450-451`)
skips `potential_spawns` unless `mod_api::mods()` is non-empty. The launcher
installs a bundle as a trait mod (`launcher/src/native_mods/api.rs` generates a
`main` with `main_with(mods)`), but the **source-mixin build path** injects the
HEAD helper and leaves `crates/horizon-client/src/main.rs` calling
`main_with(Default::default())`. Every other hook still ran in the campaign's
ports because their helpers are injected unconditionally; this one is gated.
The shipped bundle therefore declares an `[api]` trait-mod shim
(`mixins/torchmaster/api`, an empty `Hooks`), which the launcher links into
`mods()`; the veto itself stays the injected HEAD helper. The pin's
horizon-mixins `5a64af9` manifest parser rejects `[api]`, so the port's
`tools/horizon.py` stages a copy with that section removed and writes the
launcher-equivalent harness `main` into the disposable stage (`HARNESS_MAIN`:
one empty `Hooks` impl). This was measured: with the stock main the first-call
log never fired and the probe saw 33 spawns; after the harness main the probe
saw control=8 / probe=0. The harness main is not part of the package.

## Embedded content — one `.hnmod` deliverable (2026-11-10)

The content half now travels **inside** the native bundle (the AE2 pattern, proved
on `charginggadgets`): the built `content/torchmaster.hmod` is carried at
`mixins/torchmaster/data/torchmaster-content.hmod`; `embedded_content.rs` writes it to
`.horizon-cache/` at start and appends it to the dev list from one `content_root`
HEAD injection (`cancellable = true`; bindings `root`, `enabled`, `dev`). One
installed `.hnmod` installs both halves, so the split content+code blocker is
gone. `tools/package.py` packs `data/` and refuses a copy that differs from
`content/torchmaster.hmod`.

Verification (2026-11-10, pin `1d63e0a`, Minecraft 26.3):
- **ladder**: `cargo fmt --check`, `clippy --all-targets -- -D warnings` (0
  warnings), `cargo test` **8 passed / 0 failed**, `horizon.py stage / check / test /
  build` all `cargo_exit: 0` (embedded tests included).
- **artifacts**: `content/torchmaster.hmod` **6,488 B**, SHA-256 `c5c22944bf2d4a5a561353033b43ea71746bad94e1565fdea698d232ae9724c7`,
  **19** entries; `torchmaster.hnmod` **15,467 B**, SHA-256 `2a1bc56ffa49760b9e39136ef6474fcf165c1aeeba68bc60b9bf450d36fc50e9`, **12**
  entries (rebuilt locally from these sources with `tools/package.py`; a second
  run is byte-identical and `horizon-mod check --kind native-mod` passes);
  `mixins/torchmaster/data/torchmaster-content.hmod` is byte-identical to the built `.hmod`.
- **e2e with no `--dev-mod`** (`EMBEDDED=1`): `PASS` + `EXIT:0`; the run printed
  `embedded content: 6488 B, identical to content/torchmaster.hmod`.
- **control** (`e2e-control.log`, `.horizon-cache` made unusable): the sample id
  is unknown, `PASS` + `EXIT:0` - the content really comes from the bundle.
- **fail leg** (`NEGATIVE=1`): `FAIL` + `EXIT:1`.
- `check-e2e-logs.py torchmaster --strict` green (all logs end `EXIT:<n>`);
  `upstream_ready.py torchmaster` -> `READY`, exit 0.

The bundle also carries `mixins/torchmaster/api/` (the `[api]` trait-mod shim); `tools/package.py` now packs nested `.rs`/`.toml` as well as `data/`.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices (fmt, clippy `-D warnings`, tests, expressive enums/newtypes, no clone-to-satisfy-borrowck)

## Verification

- **TDD:** the eight tests were written first; `unit-tdd-red.log` shows the
  stubbed engine (5 failing, 3 enum tests passing) and `unit-tdd-green.log`
  shows 8/8 after the implementation. `cargo fmt --all --check` and
  `cargo clippy --all-targets --locked --offline -- -D warnings` were clean.
- **API 388 probe (before the port):** the probe script (kept at
  `ports/_shared/evidence/torchmaster/probe/`) launched the probe build twice
  in the same sealed sky box; the control grew **8** monsters and
  `TORCHMASTER_PROBE=1` (cancel every candidate) kept the box empty for the
  same 600 ticks: `PROBE PASS: control=8, probe=0` (`probe/probe.log`). The
  final bundle removes the probe branch.
- **Staged ladder at `1d63e0a`:** `stage → check → test → build`, all
  `cargo_exit: 0`; the embedded run is `game::torchmaster::tests` **8 passed,
  0 failed** with 1240 filtered out (`ladder-test.log`), and check/build
  **0 warnings** (`ladder-check.log`, `ladder-build.log`).
- **`tools/e2e.sh` PASS:** content give/place/break, then
  `PASS: control=30, torch=0, outside=13, after_break=51` and `EXIT:0`
  (`e2e-pass.log`). The torch phase logs
  `registered torchmaster:megatorch` and the break logs
  `[torchmaster] unregistered torchmaster:megatorch at minecraft:overworld
  [0, 210, 0]`, and the room's block light reads 0 with the torch above the
  roof.
- **Negative control:** `NEGATIVE=1` expects a spawn with the torch; it exits
  **1** with `FAIL: NEGATIVE=1 expected a spawn with the torch, saw none`
  (`e2e-negative.log`) — the zero is not vacuous.
- **Artifacts:** content `.hmod` **6,488 B**, SHA-256
  `6d192414164de9796695ed660978726cdd9daad5f29d1840ac79d144986cb9d5`;
  native `.hnmod` **10,036 B**, SHA-256
  `1c21a3a960d824823f91a1f3d8d628ae15f1f1ec77d62a2f83f19d0a520be68c`.

## Gaps

- **Spawners are deliberately not blocked.** Upstream's
  `blockOnlyNaturalSpawns` defaults to true; the pin's `potential_spawns` only
  fires for the natural path. The `false` mode (block spawners too) has no
  hook at the pin and is not carried.
- **Phantoms and village sieges.** Upstream also vetoes phantom spawns
  (`PlayerSpawnPhantomsEvent`) and village sieges (`VillageSiegeEvent`); the
  latter now has a pin hook (`village_siege`, API 389), but phantoms do not go
  through `potential_spawns`. Both are slice 2.
- **Radii/config are fixed at the upstream defaults (64).** The brief asks for
  config-free defaults; `megaTorchRadius`, `dreadLampRadius`, the entity-list
  overrides and the aggressive/peaceful switches are not modelled.
- **Category-level veto.** The hook gives the spawn category, not the entity
  list, so a datapack that lists a friendly entity under `monster` would be
  vetoed by the mega torch; upstream filters per entity type. Vanilla lists are
  category-pure, so this is a documented simplification.
- **Dread Lamp in-game phase not automated.** Its recipe/item/block are carried
  and unit-tested; a natural passive-spawn run needs a grass box and the
  400-tick creature cadence, so it is left to slice 2. The mega torch phases
  exercise the shared veto path both blocks use.
- **Feral Flare Lantern, invisible light, frozen pearl, menu/GUI, volume
  renderer, the `/torchmaster` commands and the feral-lantern spawner
  interactions** are not carried.
- **Light-level independence** is proven by placing the torch above the sealed
  roof: the roof blocks its light and the e2e asserts the room's block light is
  0, so the zero spawns are the hook, not the light level. A torch placed inside
  a lit room still vetoes by the hook, but the e2e does not measure that case.
- **Block drop** is carried as the upstream loot table but not asserted in the
  e2e: the scenario runs in creative, where breaking a block drops nothing. The
  give/place/break path and the registration are exercised.

## Licence

MIT (Xalcon/TorchMaster, `LICENSE` copied). Rank 0; derivatives are allowed,
but the port stays local-only until the maintainer approves publishing
(ARCHITECTURE.md §8).

### Mode B + AE2 embedding (2026-11-10) — one listable artifact, re-verified

Pin unchanged (`1d63e0a5…` / `5a64af93…`, schema 6, `game.rs`
`64813cf4…f80f`). The session rebuilt the port from its current sources and
applied the **AE2 embedding pattern** so one `.hnmod` installs the content and
the code (the registry keeps one file per project): `mixins/torchmaster/data/torchmaster-content.hmod`
is the built `content/torchmaster.hmod` byte for byte, `mixins/torchmaster/embedded_content.rs`
compiles it in with `native_data!`, and the `embedded-content` `[[injection]]`
(`target = "horizon_client::game::mods::content_root"`, `kind = "HEAD"`,
`cancellable = true`, bindings `root`/`enabled`/`dev`) writes it to the game's
`.horizon-cache` and appends it to the dev-mod list. `tools/package.py` refuses
an embedded copy that differs from the built hmod.

All compile/e2e steps ran through `ports/tools/build-slot.sh` with
`CARGO_INCREMENTAL=0`; the ladder is the shared `tools/ladder-embed.sh`
(hmod build → embed → hnmod → fmt → clippy → test → stage/check/test/build),
evidence `ports/_shared/evidence/torchmaster/ladder-*`.

| Step | Result |
|---|---|
| `cargo fmt --all --check` / clippy `-D warnings` | exit 0, **0 warnings** |
| `cargo test --locked --offline` | **8 passed**, 0 failed |
| `horizon.py stage/check/test/build` | `cargo_exit: 0` in every step |
| `content/torchmaster.hmod` | **6,488 B**, `c5c22944…724c7`, **19 entries**, "no code: content only" |
| `torchmaster.hnmod` (single artifact, carries the content) | **15,467 B**, `2a1bc56f…50e9`, **12 entries** |
| e2e (with `EMBEDDED=1`, the shipped shape) | default **PASS + `EXIT:0`** / negative **FAIL + `EXIT:1`** |
| `check-e2e-logs.py torchmaster --strict` | green (`logs=… ok=…`, exit 0) |
| `upstream_ready.py --copyleft torchmaster` | **READY**, exit 0 |

Fail-capability: the selftest/negative legs flip a real assertion and exit
non-zero; the embedded shape is the one the artifact ships (no `--dev-mod`), and
the e2e checks the bundle wrote its content out (or the loader reports
`content from 1 mod(s)` when the game consumes the file).

## Reviewer repairs (2026-11-11) — PR review items and what changed


Each item below is quoted from `ports/_shared/horizon-mods/reviews/pr-*-torchmaster.md` (the review is the specification), with what this repair did about it.

- **Reviewer:** pinned hash vs PR-body hash
  **Repair:** The entry pins the artifact in this repository; the repackaged file has a new hash now, so the entry, the release asset and the PR body must be re-aligned on re-push (orchestrator).
- **Reviewer:** licence notice
  **Repair:** `LICENSE` + `LICENSE-NOTICE.md` in the `.hnmod` (and the content half's `data/torchmaster/license/`), with Xalcon's copyright; the jar was unavailable to the audit, which the notice states.
- **Reviewer:** relative link
  **Repair:** Already absolute (`https://github.com/Xalcon/TorchMaster`).
- **Reviewer:** registry artifact delivery
  **Repair:** **Staging (orchestrator, not ours)**: the artifact is not in registry release/staging as `<sha256>.<ext>`; the orchestrator answers it with `horizon-mods#686` (no write access to the registry's releases). Every repackaged artifact below has a fresh sha256 for that re-push, and the author release must be re-uploaded from the port repository.

Skills: superpowers/using-superpowers,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

## Migration to the SDK format (2026-11-18, nightly `0b14f8a`)

Horizon's mod-API merge (#16941, launcher ≥ 0.2.70, `horizon-rust/horizon#17992`)
retired the `.hnmod` container, and the registry closed this port's submission
(**PR #609**, `submit/torchmaster-0.1.0`) on 2026-11-18 with *"republish this as a
native `.hmod` with the SDK from the next Nightly"*. The port is now one flat SDK
package at nightly **`0b14f8acfd97fa2c750e42795249dcc981178ca4`**, mixins
**`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, launcher 0.2.78, API `0.2`.

Everything above this heading describes the retired bundle and stays as history.
The archive of that bundle — its `.hnmod`, plan, per-hook helpers, content
`.hmod`, old ladder and pin files — is
`ports/_shared/evidence/torchmaster/retired-hnmod/` (with a README saying what
each file was). Nothing in it is built, packed or loaded any more.

### Files created, files deleted

| Created | What it is |
|---|---|
| `horizon-mod.toml` | the only manifest: `id`/`name`/`version` 0.2.0/`api = "0.2"`/`side`/`capabilities`, plus `[native] game = "26.3"`, `horizon = 0b14f8a…`, `mixins = e59da90…` |
| `Cargo.toml` | crate `torchmaster` 0.2.0: `horizon-mod-api` from `.horizon/sdk/api/crates/horizon-mod-api` + `log = "0.4"` (already in the SDK's graph) |
| `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks` — five hooks, the registry in a `Mutex<Store>` |
| `src/runtime.rs` | the old `runtime.rs`, **byte-identical** (sha256 `954bf3b2…f28f1c`), 8 tests unchanged |
| `assets/torchmaster/`, `data/torchmaster/`, `data/minecraft/tags/` | the content half, unpacked, straight from `content/` |
| `icon.png` | 128×128, from the publish tree (square, ≤ 512², ≤ 256 KiB) |
| `torchmaster.hmod` | the delivered package, built and checked here |
| `.gitignore` | `/target`, `/.horizon`, `/Cargo.lock` (the lock is generated and packed, not tracked) |

| Deleted | Why |
|---|---|
| `mixins/torchmaster/` (plan, `support.rs`, one helper per hook, `api/` shim, `data/torchmaster-content.hmod`) | the SDK has no plan, no `[[module]]`/`[[injection]]`, no `[[native.mixin]]` here, and no `[api]` section |
| `content/` (`horizon-mod.toml`, `assets/`, `data/`) | content now lives **inside** the same package as `assets/` and `data/` |
| `torchmaster.hnmod` | the retired container |
| `Cargo.toml`/`Cargo.lock`/`compatibility.json`/`rust-toolchain.toml` | the standalone crate that exposed `runtime.rs` and the pin file; the pin is `[native]` now |
| `tools/{horizon.py,package.py,package_content.py,ladder-embed.sh,e2e.sh,stamp_license.py,audit/}` | the old ladder, the `.hnmod` packager, the embedding step and the licence stamper — all retired with the format |
| `AGENTS.md` (port copy) | it described the `.hnmod` workflow and would mislead |

### Hook mapping (every old helper → its SDK method)

| Old (retired format) | New (SDK) | Behaviour |
|---|---|---|
| `potential_spawns` HEAD helper, `canceled` binding | `Hooks::potential_spawns(&self, _host: &dyn Host, event: &mut PotentialSpawnsEvent) -> Flow` | identical: `blocks(...)` on the registry returns `Flow::Cancel`, which empties the candidate list |
| `block_changed` HEAD helper (`old`/`new` `BlockStateId`, `GameLevel`) | `Hooks::block_changed(&self, level: &dyn LevelHost, event: &mut BlockChangedEvent)` | `event.old`/`new` are `BlockState`; `level.state_block(state)` gives the id, so no registry lookup |
| `world_lifecycle` HEAD helper, `WorldPhase::WorldOpenReady` → load | **`Hooks::server_started`** | the SDK's `WorldPhase` has only `WorldOpenFailed`/`WorldCloseFinal`/`WorldCloseFailed`; `server_started` is the one "the world is open" moment, and `host.world_dir()` is `Some` there |
| `world_lifecycle` → `WorldCloseFinal`/`WorldCloseFailed` save, `WorldOpenFailed` clear | same hook, same phases | unchanged |
| `level_tick_post` HEAD helper, `game.world.game_time`, `native_level_block` | `Hooks::level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent)` | same 100-tick cadence; `event.level` is a `LevelRef`, and the level's services are called **on the handle**: `level.game_time(host)`, `level.block(host, pos)`, `block.id(host)`, `level.dimension().id(host)` |
| `embedded_content` + `content_root` HEAD injection + `[api]` shim | **deleted** | the content ships in the same `.hmod` (`assets/`, `data/`), so nothing is written to `.horizon-cache`; and a compiled native mod *is* a `mod_api` mod, so the `mod_api::mods()` gate that needed the shim is satisfied by construction |

The five hooks are the whole surface: no `[[native.mixin]]` was needed, because
every point the port uses is a named SDK hook. `Hooks` methods are the only
place the port touches the game — `runtime.rs` still has no `mod_api` type.

### Manifest mapping

| Retired | SDK |
|---|---|
| `content/horizon-mod.toml` (a second manifest, `api = "0.2"`) | merged into the one `horizon-mod.toml` |
| `mixins/torchmaster/horizon-mixin.toml` `[mod] id/version/priority` | `id`/`version` in `horizon-mod.toml` (priority is `#[listener(priority = …)]`, unused here) |
| `[compatibility] horizon_revision` | `[native] horizon` + `[native] mixins` + `game = "26.3"` |
| `[[module]] runtime` → `crate::game::torchmaster` | `pub mod runtime;` (a normal module of the crate) |
| `[[module]] state` → `crate::game::torchmaster_state` | the glue is in `src/lib.rs`; the registry is the `Mod` struct's `Mutex<Store>` |
| `[[injection]] …` × 5 | one `Hooks` method each |

### Build and check (evidence: `ports/_shared/evidence/torchmaster/sdk/`)

| Step | Command | Result |
|---|---|---|
| Unit tests | `cargo test` | **8 passed**, 0 failed (`sdk-test.log`) |
| Format and lints | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | clean, exit 0, **0 warnings** (`sdk-lint.log`) |
| Package | `cargo horizon-mod build` | `target/horizon-mod/torchmaster.hmod` (`sdk-build.log`) |
| Check | `cargo horizon-mod check torchmaster.hmod` | exit 0: `torchmaster 0.2.0 (native-mod, format native-1/0b14f8ac…/e59da909…, for 26.3)`, `24846 bytes, SHA-256 b06aa76f…` (`sdk-check.log`) |
| Fits | `cargo horizon-mod fits torchmaster.hmod` | `0b14f8a made for`, exit 0 |
| Package contents | `unzip -l` | **28** entries, `LICENSE` + `LICENSE-NOTICE.md` + `README.md` + `icon.png` inside |
| Determinism | two builds compared | byte-identical (`b06aa76f…57257` both times) |

The delivered package is `torchmaster.hmod` in the port root: **24,846 B**, SHA-256
**`b06aa76f34b344b9ba3834b09ec8a6e73a6eeb39ddb01d244ec4e74793025707`**.

### Traps this migration hit (for the rest of the wave)

1. **The host's area traits are the *game's* side, not the mod's.** The mod-facing
   call is a method on the handle with the host as its context:
   `level.game_time(host)`, `level.block(host, pos)`, `block.id(host)`,
   `level.dimension().id(host)`. `host.level_game_time(level)` does not exist —
   `LevelsHost`/`LevelsBackend` hold the backend signatures, and calling one
   gives `E0599 method not found`. `cargo horizon-mod hooks <text>` lists hooks,
   not host methods; read `generated/mod_api_generated_host.rs` for the handles.
2. **`world_dir()`/`instance_dir()` *are* callable on `&dyn ServerHost`** without
   importing `ConfigServerHost`: a trait object resolves the methods of all its
   supertraits. Do not add `use mod_api::prelude::*` "to make it compile" — this
   crate needs no prelude and clippy flags it as an unused import.
3. **`cargo fmt --all --check` walks the path dependency** into `.horizon/sdk/`
   and reports the SDK's own files. Use `cargo fmt --check` on the crate.
4. **Content namespaces are enforced by the builder**: `assets/<id>/` and
   `data/<id>/` only, plus vanilla and `c` tags and vanilla atlases. A file under
   `assets/probe/` fails the build with a plain-English reason.
5. **`check` is not a compile and not a hook run.** It validates the manifest, the
   pin and the package layout and prints the sha. The compile gate is
   `cargo test`/`cargo clippy` on the crate (the crate *is* the mod), and no
   in-game check is possible here — see the gaps below.
6. **The far box cannot take these steps**: it has `rustc 1.93.1`, no
   `cargo-horizon-mod` and no `.horizon/sdk` cache, so every step above ran in a
   local slot with `BUILD_LOCAL=1` (the forwarding rule is for the old ladder).
7. **`log` is a safe extra dependency**: `horizon-mod-api` already depends on
   `log 0.4`, so the version is in the pinned graph and needs no `native-lock`.

### Gaps this migration leaves

- **No in-game run at `0b14f8a`.** The old e2e drove a *port-built*
  `horizon-client` and the content half through `--dev-mod content`; the SDK era
  builds the mod into the game through the launcher (`horizon-launcher`, not
  installed here, and no `0b14f8a` game binary on this box), so
  `cargo horizon-mod run` could not be exercised. The behaviour proof for the
  port is the retired bundle's e2e (below), whose *code* is unchanged apart from
  the five glue mappings tabulated above; the hook names and contracts are the
  ones the pin's own catalog documents. A re-run belongs to the wave's in-game
  pass, not to this migration.
- **`WorldOpenReady` is gone from `WorldPhase`.** Loading happens at
  `server_started`; a world that fails to start never loads, which is what
  `WorldOpenFailed` already cleared in the old design.
- **`potential_spawns`' `entries` list is unused.** The port cancels (which
  empties the list) exactly as before; it does not filter individual entries, so
  a datapack that lists a friendly entity under `monster` is still vetoed by the
  mega torch (the old category-level simplification, unchanged).
- **`uses.json` is new in the package** (generated by the builder: the API
  surface the package's hooks reach, with a hash per entry; `cargo horizon-mod
  fits` reads it). It is not source and is not edited by hand.
- **A custom level is now expressible.** The retired pin's `LevelDimension` was a
  closed three-variant enum, so the light registry could only ever name the
  Overworld, the Nether and the End. The SDK's `Dimension` has
  `Custom(LevelKey)`, and the port keys a light by `event.dimension.key().id()` /
  `level.dimension().id(host)`, so a data-pack dimension works without a code
  change.
- Everything in § "Gaps" above (spawners, phantoms, sieges, config radii, the
  Dread Lamp in-game phase, the feral flare lantern family) is unchanged by this
  migration.

### Changelog for the resubmission (verbatim)

> Supersedes #609, closed because Horizon retired `.hnmod` (horizon-rust/horizon#17992). Changes:
> flat SDK package with `api = "0.2"` and `[native]` pinned to `0b14f8a`; the five hooks rewritten
> as `impl mod_api::Hooks` methods against typed hosts/events (`potential_spawns`, `block_changed`,
> `server_started`, `world_lifecycle`, `level_tick_post`); the content half now ships inside the
> same `.hmod` (`assets/`/`data/`), so the embedded-content injection and the `[api]` shim are gone;
> `runtime.rs` and its 8 tests unchanged.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port
