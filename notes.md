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

`content` / `logic` (the ledger row). The port has a content half (three blocks,
their items, recipes, loot, tags, models/textures) and a native logic half (the
placed-light registry, the spawn veto and the flare's light writes); no machine
or client half is needed. The frozen pearl, the line-of-sight menu/GUI, the
volume renderer and the feral-lantern spawner interactions remain outside this
tier's shape and are later slices (see §0.2.3).

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
  exercise the shared veto path both blocks use. **Closed in 0.2.2**, on the
  ambient family: see §0.2.2 (control 4 bats, lamp 0, 70 monsters still spawn).
- **Feral Flare Lantern and its invisible light are carried in 0.2.3** (see that
  section). The **frozen pearl, the line-of-sight menu/GUI, the volume renderer
  and the feral-lantern spawner interactions** are still not carried; the
  `/torchmaster` commands were carried in 0.2.2.
- **Light-level independence** is proven by placing the torch above the sealed
  roof: the roof blocks its light and the e2e asserts the room's block light is
  0, so the zero spawns are the hook, not the light level. A torch placed inside
  a lit room still vetoes by the hook, but the e2e does not measure that case.
- **Block drop** is carried as the upstream loot table but not asserted in the
  e2e: the scenario runs in creative, where breaking a block drops nothing. The
  give/place/break path and the registration are exercised. **Closed in 0.2.2**:
  the survival leg drops both blocks, with a vanilla crafting table as control.

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

## 26.3 dialect sweep (2026-11-21) — carried recipes and loot tables, proven in game (0.2.1)

The shared checker `ports/tools/check_261_dialect.py` (generalised from `ports/modularrouters/tools/data_shape.py`
and `ports/createaddition/tools/data_shape.py`) found the 1.21.1 forms still in the package that had shipped
at **0.2.0**; `cargo horizon-mod check`, `fits` and every unit test had passed on them, and the content
was dead in game (a recipe returns nothing; a block drops empty).

| Family | Files | 1.21.1 form that shipped | 26.3 form |
|---|---|---|---|
| recipe ingredients | 2 (`megatorch`, `dreadlamp`): `{"item": …}` / `{"tag": …}` in `key` | `{"item": "…"}` / `{"tag": "…"}` | `"minecraft:…"` / `"#c:…"` |
| loot pools | 2 (`megatorch`, `dreadlamp`): `"conditions"` array | `"conditions": [ … ]`, `bonus_rolls`, `"rolls": 1.0` | one `"condition": {"type": "minecraft:…"}`, no `bonus_rolls`, `"rolls": 1` |

This is the port with a **pre-fix game leg**: `e2e-dialect-red.log` runs the same proof on 0.2.0 and fails both ways — `expected 1 torchmaster:megatorch, got None` (the grid held the exact ingredients) and `torchmaster:megatorch dropped 0 of torchmaster:megatorch` while the vanilla control dropped 1.

**Red, kept:** `ports/_shared/evidence/torchmaster/dialect-check-red.log` — the checker exits 1 on the pre-fix
package itself (`dialect-prefix-0.2.0.hmod`, 0.2.0, kept in the same folder), naming every file.
A reconstructed fixture carrying all three loot shapes plus a `requires_tool` block with no mineable tag is
red too (`../../morered/dialect-check-fixture-red.log`). The checker's `--selftest` fails when any shape is
accepted.

  `recipe megatorch.json: crafting_shaped -> {"count": 1, "item": "torchmaster:megatorch", "slot": 200}`; `the result reached the inventory (1 torchmaster:megatorch), the grid emptied, and minecraft:diamond was consumed`

  `the vanilla control dropped one, torchmaster:megatorch dropped 1 torchmaster:megatorch, and the cell is air`

The in-game leg is `tools/e2e-dialect.sh` (`ports/_shared/evidence/torchmaster/e2e-dialect.log`): `PASS` + `EXIT:0`,
and `SELFTEST=1` inverts the expectation and ends `FAIL` + `EXIT:1` (`e2e-dialect-selftest.log`).
`check-e2e-logs.py torchmaster --strict` exits 0.

**Ladder** (all in `build-slot.sh` leases, `CARGO_INCREMENTAL=0`, `BUILD_LOCAL=1`):

| Step | Command | Result |
|---|---|---|
| tests | `cargo test --offline` | **8 passed**, 0 failed (`dialect-test.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` (twice) | built against **0a875da** (re-pinned from 0b14f8a), second build `cmp`-identical (`dialect-build-1.log`, `dialect-build-2.log`) |
| registry check | `cargo horizon-mod check torchmaster.hmod` | `torchmaster 0.2.1 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **22,418 B**, sha256 `0e5f301e8cd2…` (`dialect-check.log`) |
| fits the pin | `cargo horizon-mod fits torchmaster.hmod --build 0a875da` | `0a875da  made for`, exit 0 (`dialect-fits-0a875da.log`) |
| fits the previous build | `… --build 0b14f8a` | `fits`, exit 0 (`dialect-fits-0b14f8a.log`) |
| the gate | `python3 ports/tools/verify-sdk-package.py torchmaster` | **1/1** (`dialect-gate.log`) |

The **first** 0.2.1 package was 22,418 B, SHA-256 `0e5f301e8cd292e0e20402b6e99916207abe37473ce7a58fb625736fb2eb4066`; the
reviewer repair below replaced it with **22,772 B**, SHA-256
`3b9a1d5f46d1d790522b62ab528d763f7a23115e8ea802a91cf016b3d2665e86`, which is the delivered 0.2.1.

## Packaging repair (2026-11-21) — the launcher's `Cargo.lock` restored (PR #1536)

Reviewer, verbatim: *"Restore the launcher-generated `Cargo.lock`. The 0.2.1 package drops the
lockfile from 0.2.0 but still declares `log = "0.4"`, so a clean launcher build refuses it despite
green registry CI. Attach a receipt from the new archive after repackaging."*

**Why it happened.** The campaign's `rm -f Cargo.lock` rule (§0al of `HANDOVER.md`) exists because
`cargo test` leaves a *crate-wide* lock in the tree and the packer takes the directory listing, so it
rode into SDK-only packages by accident. torchmaster is not SDK-only: `Cargo.toml` declares
`log = "0.4"`, a registry crate the launcher resolves and compiles itself, and it reads the pins from
the package's lock. 0.2.0 happened to carry the 8,840 B crate-wide lock that `cargo test` had left;
the 0.2.1 dialect rebuild began with `rm -f Cargo.lock`, so the lock was absent. `check` and `fits`
cannot see this — they never compile — which is why registry CI was green.

**Red (a clean launcher build of the lock-less package).**
`ports/_shared/evidence/torchmaster/lock/native-build-no-lock.log`:

```
$ horizon-launcher native-build ports/torchmaster --instance e2e --trust-native-code
Using the mod tools for the Nov 20 update kept from an earlier build
Prebuilt game: only the mods compile (optimized)
Compiling the mods against the prebuilt game
error: /home/adlf/.cache/horizon-e2e/root/data/horizon-launcher/native/mods.incoming/torchmaster/Cargo.lock: No such file or directory (os error 2)
EXIT:1
```

The launcher does not generate a lock at build time; it copies the package's, so the missing file is
the whole failure — before a single crate compiles.

**Fix.** `horizon-launcher native-lock ports/torchmaster` (launcher 0.2.92, the release
`ports/tools/launcher-game.sh` verifies) writes the **registry-only** lock the launcher wants — not a
crate-wide one:

```
# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = "log"
version = "0.4.35"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "5d15f86d12da44cd8d4ec3c73984537eb7f4cadcb9776a575a0a8a05ca11005c"

[[package]]
name = "torchmaster"
version = "0.2.1"
dependencies = [
 "log",
]
```

`ports/torchmaster/Cargo.lock` is now that 370 B file and is **tracked** (the `/Cargo.lock` line is
gone from `.gitignore`): `cargo horizon-mod build` does not regenerate it (measured — the lock was
byte-identical, sha `560b3910…`, before and after the package build), so a fresh checkout keeps it.

**The order matters: `cargo test` rewrites it.** A unit-test run resolves the crate's whole graph
(the SDK path crates included) and replaces the registry-only lock with the 8,840 B crate-wide form
(measured: sha `d9b43a1f…` after `cargo test --offline`, test exit 0). That form *also*
launcher-builds (0.2.0 shipped with it), but it is not the launcher's own lock, so the build order in
this port is **`cargo test` → `horizon-launcher native-lock` → `cargo horizon-mod build`**; the
rebuilt package is byte-identical to the delivered file when the lock is the launcher's.

**Green, from a clean cache.** The launcher's mod target dir was removed
(`…/prebuilt-build/0a875da91dbb-9d4b5b847642e8b1/target`, 34 MB) and the build was then run on the
**delivered archive**, extracted to `~/.cache/horizon-e2e/scratch/pkg-torchmaster-3b9a1d5f`
(`lock/native-build-from-delivered-archive.log`):

```
Compiling log v0.4.35
Compiling torchmaster v0.2.1 (…/stage-766e6955f386ac52/native-api/torchmaster)
Compiling horizon_patch v0.0.0 (…/stage-766e6955f386ac52)
 Finished `release` profile [optimized] target(s) in 7.92s
Mods compiled against the prebuilt game in 10.0 s
native-0a875da-07fc23425d50 built and selected for its instance.
EXIT:0
```

`log` — the crate the lock pins — is compiled *from scratch* in that run, then the mod and the patch;
only the prebuilt game (which no lock affects) was warm. That is the reviewer's "clean build".

**Ladder on the delivered bytes.**

| Step | Command | Result |
|---|---|---|
| package | `cargo horizon-mod build` (with the launcher lock present) | 22,772 B, lock survived byte-identical (`lock/package-build.log`) |
| registry check | `cargo horizon-mod check torchmaster.hmod` | `torchmaster 0.2.1 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `22772 bytes, SHA-256 3b9a1d5f…` — exit 0 (`lock/lock-check.log`) |
| fits | `cargo horizon-mod fits torchmaster.hmod --build 0a875da` | `0a875da  made for`, exit 0 (`lock/lock-fits-0a875da.log`) |
| loads | `launcher-game.sh build torchmaster` + `tools/e2e-dialect.sh` in a slot | `PASS` + `EXIT:0`; the recipe crafts and the block drops its loot on the game built from this archive (`e2e-dialect.log`) |
| gate | `python3 ports/tools/verify-sdk-package.py torchmaster` | **0/1** — every line `ok` except `FAIL no packed Cargo.lock` (`lock/verify-sdk-package.log`) |

**The gate's one failure is a tool rule this package must not follow.** `verify-sdk-package.py:113`
asserts `"Cargo.lock" not in names` for every package with `src/`, the SDK-only rule. For torchmaster
it contradicts the reviewer: the lock is the deliverable, and a lock-less rebuild is exactly what the
launcher refused above. The port therefore keeps the lock and reports `0/1` with that single line
rather than dropping the fix; the checker needs an exemption for a package whose `Cargo.toml` declares
a registry dependency (orchestrator action, `ports/tools/` is not mine).

**Receipt, new archive.** `torchmaster.hmod` **22,772 B**, SHA-256
**`3b9a1d5f46d1d790522b62ab528d763f7a23115e8ea802a91cf016b3d2665e86`**, **28** entries, containing
`Cargo.lock` 370 B (`unzip -p … Cargo.lock | sha256sum` = `560b39101d3272031aaf8d97d2254b1a82520433fce6185532ce4cb5a4cebe49`).
The 0.2.0 lock that was dropped was 8,840 B; this one is the launcher's registry-only form, which is
what the reviewer asked to restore. Version stays **0.2.1**: the PR is a packaging repair of the
version under review, not a behaviour change, and its release asset must be re-uploaded from these
bytes.

Skills: superpowers/using-superpowers,systematic-debugging,test-driven-development,verification-before-completion; BeautifulRust/none (no Rust change)

## 0.2.2 (2026-11-23) — R9: the `log` dependency is gone, `/torchmaster` says what it said

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** (mixins `e59da909…`), registry entry ours on
`main` (0.2.1, #1536 merged), so this is an **Update**.

**The rule (R9, read 2026-11-23).** `tools/native-subset/src/manifests.rs` in
`horizon-rust/horizon-mods` refuses a package whose `Cargo.toml` has any dependency other than
`horizon-mod-api = { path = ".horizon/sdk/api/crates/horizon-mod-api" }`, a build script, a
proc-macro, or a table other than `[package]`/`[dependencies]`/`[lib]`/`[lints]` ("no
`[workspace]`, even empty"); R10 then accepts a `Cargo.lock` only when it is the previous
release's, and an SDK-only package needs none. This port declared `log = "0.4"` for five
`log::info!`/`log::warn!` calls and packed the launcher's lock, so R9 refused it — every future
submission of the mod was blocked by it.

**The red, before the change** (`ports/_shared/evidence/torchmaster/r9/gate-before-0.2.1.log`):

```
  FAIL SDK-only dependencies (R9)  refused by R9: log — replace it, do not pack a lock
  FAIL no packed Cargo.lock  28 entries — strip it: R10 only accepts the previous release's lock, …
0/1 package(s) verified
```

**What replaced `log`, and why that choice.** The pin has **no stamped logging or tell hook a mod
may write to**: `ports/_shared/sdk/0a875da/surface.json` has no `log`/`tell` surface, and its only
console-related hook is `hook:console_line` (a notice that a console *line arrived*, server side,
counterpart "none (NeoForge has no log event; mods add a Log4j appender)"). So option (b): the
calls are **deleted**, and the same information is the mod's own command readout, which an operator
can ask for at any time instead of scraping a log:

| removed call | what an operator sees now |
|---|---|
| `block_changed` register / unregister (`lib.rs:91,106`) | `/torchmaster` count and `/torchmaster list` — the light is there or is not, read on demand |
| `server_started` "loaded N blocking lights" (`:125`) | `/torchmaster` after the world opens: `torchmaster: N blocking lights: …` |
| `read_lights` / `save` failures (`:212,222` and the two write/create failures) | the same line, suffixed `last problem: <what failed>` (stored in the registry's `Store::problem`) |

`register_commands` declares `/torchmaster` and `/torchmaster list` (level 0: reading a registry is
harmless and nothing here changes a light); `run_mod_command` answers through
`host.commands().feedback`. No other crate, no build script, no proc-macro.

**Dependencies and lock.** `Cargo.toml` now ends with `[dependencies]` and declares only the SDK
path dependency; `Cargo.lock` is deleted from the tree and never packed (`.gitignore` has
`/Cargo.lock` again, so a `cargo test` probe lock cannot be tracked). **The older "declare a
registry dependency and pack its lock" rule is dead for publishing** — it was this package's
0.2.1 packaging repair (see the section above), and R9/R10 now refuse exactly that shape. The
launcher build shows the lock is no longer needed at all:
`ports/_shared/evidence/torchmaster/r9/native-build-no-lock.log` — `native-build` compiles
torchmaster and `horizon_patch` from the lock-less package, `EXIT:0`, 3 s.

**TDD for the readout** (`src/runtime.rs`): three tests were written first and failed to compile
against the missing functions (`r9/unit-tdd-red.log`), then `count_line`, `light_lines` and
`LightKind::name`/`of_name` were added; `format_lights`/`parse_lights` now use the name pair. Tests
**8 → 11**, all green.

**Ladder** (slot leases, `BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`):

| Step | Command | Result |
|---|---|---|
| tests | `cargo test --offline` | **11 passed**, 0 failed (`r9/unit-tdd-red.log` red, `r9/test.log` and `sdk-test.log` green) |
| fmt + lints | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | clean, 0 warnings (`r9/fmt.log`, `r9/clippy.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` | 0.2.2 hmod, no lock packed (`r9/build.log`) |
| registry check | `cargo horizon-mod check torchmaster.hmod` | `torchmaster 0.2.2 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)` (`r9/check.log`) |
| fits the pin | `cargo horizon-mod fits torchmaster.hmod --build 0a875da` | `0a875da  made for`, exit 0 (`r9/fits-0a875da.log`) |
| fits the previous build | `… --build 0b14f8a` | `fits`, exit 0 (`r9/fits-0b14f8a.log`) |
| launcher build | `launcher-game.sh build torchmaster` on the lock-less package | `native-0a875da-… built`, `EXIT:0` (`r9/native-build-no-lock.log`) |
| the gate | `python3 ports/tools/verify-sdk-package.py torchmaster` | **1/1** (`r9/gate-after.log`) |

**In game, on the delivered package** (`ports/_shared/evidence/torchmaster-e2e/`, the launcher
build named above; `tools/e2e.sh`):

```
PASS: control=3, torch=0, outside=4, after_break=4 monsters;
      lamp_control=1, lamp=0, lamp_outside=16 bats, lamp_box_monsters=70;
      drops torch=1 lamp=1 control=1; /torchmaster readouts matched at every phase
EXIT:0
```

- **Mega Torch**: a sealed dark sky box spawns 3 monsters with no light; with the torch placed
  **above the roof** (room block light `0`, so its *light* cannot explain the zero) **0** monsters
  in 400 ticks; a second box **160 blocks away** still spawns with the same torch placed (out of
  range vetoes nothing); breaking the torch brings 4 back.
- **Dread Lamp**: control 1 bat (the ambient family a sealed dark box can grow); with the lamp at
  the box centre **0 bats in 800 ticks**, while **70 monsters** keep spawning in the same box — the
  veto is friendly-only, in game; a box 160 blocks away still grows 16 bats with the lamp placed.
- **Readout**: `/torchmaster` reads `no blocking lights` fresh, `1 blocking light: 1 megatorch`
  placed (and the `list` line names `minecraft:overworld 2 201 0 megatorch`), `…: 1 dreadlamp`
  with the lamp, and `no blocking lights` after each break; `/torchmaster nonsense` answers nothing.
- **Drops**: survival breaks drop `torchmaster:megatorch` 1, `torchmaster:dreadlamp` 1, and the
  vanilla `minecraft:crafting_table` control 1 (`break` reply `drops`).
- **Control leg** (`CONTROL=1` on `CONTROL_GAME`, the same Nightly without the package):
  give 0→0, `setblock` leaves air, `/torchmaster` answers 0 lines → `FAIL` + `EXIT:1`.
- **Counterfactual** (`SELFTEST=1`): the mega-torch assertion inverted; the run saw 0 monsters in
  the vetoed box and exited `FAIL` + `EXIT:1`. `check-e2e-logs.py torchmaster-e2e --strict`:
  `logs=3 ok=3`.

**Two measured traps for the next harness:**

1. **A `ctl tp` into a chunk that is still loading drops the player through the floor.** The first
   drop phase teleported back to box A and `ctl break` answered *"block … is 18.1 blocks away, out
   of reach"* — the player had fallen through a floor that was not loaded yet (probe
   `~/.cache`-free, `/tmp/opencode/tm-break-probe2.sh`). The harness now builds each arena by
   teleporting to the column first, waiting, then filling, and runs the drop phase inside the box
   the player already stands in.
2. **The passive `creature` cap is zero in a small level**: `10 × spawnable_chunks / 289` with 22
   loaded chunks gives 0, so no cow/pig can spawn there even on lit grass (measured: 1200 ticks,
   none). `ambient` (bats, cap `15 × 22/289 = 1`) does spawn, and it is exactly the family the
   Dread Lamp vetoes, so the lamp leg uses bats.

**Both carried families, on the delivered bytes.** The dialect leg (`tools/e2e-dialect.sh`) now runs
one spec per family — `tools/dialect-proof.json` (megatorch) and
`tools/dialect-proof-dreadlamp.json` (dreadlamp, added here): each recipe crafts 1 of its block from
the grid and consumes an ingredient, each loot table drops 1 with a vanilla crafting table as the
control, `PASS` + `EXIT:0`; `SELFTEST=1` inverts both proofs and exits `FAIL` + `EXIT:1`. The
prover itself only handles one recipe/loot pair per run, hence two specs rather than a tool change.

**Coverage now closed** (the gap rows this slice settles): the Dread Lamp has an in-game phase, both
carried recipes and both loot tables are exercised on the delivered package, and the block drop is
asserted in survival with a vanilla control.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices (no clone-to-satisfy-borrowck, named consts, `expect` with a reason, pure functions kept free of `mod_api`)

## 0.2.3 (2026-11-24) — the Feral Flare Lantern and its invisible light

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** (mixins `e59da909…`), registry entry ours
on `main` (0.2.2 published), so this is an **Update** to **0.2.3**.

### What the artefact actually declares (read, not inferred)

Source: the upstream clone at `ports/_shared/upstream/torchmaster` commit
`a11226c866bf4e9f0695621db5971418dffe3014` (21.1.14, the installed version's tag), files
`common/src/main/java/net/xalcon/torchmaster/{ModRegistry,blocks/FeralFlareLanternBlock,
blocks/FeralFlareLanternBlockEntity,blocks/InvisibleLightBlock}.java` and
`neoforge/.../TorchmasterNeoforgeConfig.java`. The pack's installed jar is 21.1.9; the
21.1.10 line-of-sight/GUI addition is deliberately not carried (tier `logic`, no menu/GUI).

| Rule | Measured value | Source |
|---|---|---|
| lantern id | `torchmaster:feral_flare_lantern` | `ModRegistry.java:82` |
| invisible light id | `torchmaster:invisible_light` | `ModRegistry.java:100` |
| both blocks' light level | **15** (`.lightLevel(state -> 15)`) | `ModRegistry.java:87,102` |
| placement radius | **16** (`feralFlareRadius`, NeoForge default) | `TorchmasterNeoforgeConfig.java:99-102` |
| attempt cadence | every **5** ticks (`feralFlareTickRate`) | `:104-107` |
| minimum target light | **10** (`feralFlareMinLightLevel`) | `:109-112` |
| hard cap | **255** (`feralFlareLanternLightCountHardcap`) | `:114-120` |
| target cell | empty (`isEmptyBlock`) **and** block light `< 10`, in a loaded chunk | `FeralFlareLanternBlockEntity.java:99-118` |
| light lifetime | **until the lantern is removed/replaced** (`onRemove`/`destroy` → `removeChildLights`), not a timed expiry | `FeralFlareLanternBlock.java:82-97`, `…BlockEntity.java:177-188` |
| placement shape | `DirectionalBlock`, `facing` = clicked face's opposite, default `south`, centred 4×16×4 post, hardness 1.0, lantern sound | `FeralFlareLanternBlock.java:30-39,72-79`, `ModRegistry.java:82-88` |
| invisible light shape | `AirBlock`: no collision, replaceable, air, light 15 | `InvisibleLightBlock.java`, `ModRegistry.java:100-107` |

**Deviation — deterministic cell order.** Upstream picks a uniformly random cell of the
`2r × 2r × 2r` cube each attempt (`…BlockEntity.java:78-84`). The pin exposes no per-lantern
random stream at `level_tick_post`, and a random walk would not place a countable light in a
bounded in-game wait (the lantern's own light 15 excludes the first 15-block shell). The port
walks the same radius in a fixed nearest-first order (`runtime::flare_offsets`), keeping the
empty/dark/loaded checks, the 5-tick cadence and the cap. The first light lands six blocks from
the lantern (the nearest cell whose own light is below 10), inside the proof's count box.

**Upstream defect fixed.** The cap test is `childLights.size() > hardcap` (`:74`), which places a
256th light with a cap of 255. `runtime::should_place` uses `children < hardcap`, so 255 is the
real maximum; the red/green unit test pins it. A user would only notice at the cap, where
upstream's last write exceeds the configured safety limit the comment warns about.

### TDD and ladder

The five new tests were written first and failed to compile against the missing symbols
(`flare/unit-tdd-red.log`, 42 `E0425`/`E0433` errors); `flare/unit-tdd-green.log` then showed
**16 passed, 0 failed**. One honest mid-cycle red (`15 passed; 1 failed`) was a mistaken test
closure that used the absolute y coordinate as the radius; it was fixed in the test, not in the
engine. `flare/lint.log`: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings`
clean after replacing `ticks % rate == 0` with `is_multiple_of` on rustc 1.95.

The five hooks are unchanged in name; `block_changed` now also registers/removes lanterns and
queues their child lights, and `level_tick_post` calls a new private `Mod::tick_flares` before
the 100-tick prune. Writes use `LevelRef::set_block`/`remove_block` with `NEIGHBORS | CLIENTS`
and the pin's `BlockState` for `torchmaster:invisible_light`; both are stamped (`surface.json`
`method:LevelWriteHost::{set_block,remove_block}`, `handle:LevelRef::{set_block,remove_block}`).

Carry-over content: the block declarations (`end_rod` template for the directional lantern,
`structure_void` for the air-like light), the upstream 16×16 texture and models, the recipe in
26.3 dialect with its three `c` item tags declared locally, the self-drop loot table, the
`mineable/pickaxe` tag, lang and the creative tab. A third dialect spec
(`tools/dialect-proof-flare.json`) makes the existing `e2e-dialect.sh` prove the new recipe and
loot table alongside the two older ones.

### Coverage

| Family | State |
|---|---|
| Mega Torch spawn veto, Dread Lamp spawn veto, both recipes/loot/drops, `/torchmaster` readout | tested (the seven existing legs, re-run on 0.2.3) |
| Feral Flare Lantern places `invisible_light` in an empty dark cell within radius 16 | tested (`e2e.sh` phase E, `ctl blocks` count > 0) |
| Both blocks declare light 15 | tested (`ctl light` at the lantern > 0; content `light: 15` on both) |
| Breaking the lantern removes its child lights | tested (count returns to 0 after 20 ticks) |
| Stone in the same sealed box writes no light | tested (differential, phase E2) |
| Deterministic nearest-first placement, 5-tick cadence, 255 cap, child registry | tested (5 unit tests) |
| Frozen pearl, line-of-sight menu/GUI, volume renderer, spawner interactions | not ported (tier `logic`; no menu/renderer/entity surface carried) |
| Child-light persistence across a world restart | blocked(persistence): the block registry is rebuilt from `block_changed`, which does not fire for blocks loaded from disk; residual lights need a Frozen-Pearl-equivalent cleanup |

### Final measurements on the delivered bytes

`horizon-mod.toml`/`Cargo.toml` at **0.2.3**; `native-build` printed
`Compiling torchmaster (1 of 1)` and selected `native-0a875da-fa100b01b09e`
(`flare/native-build.log`). Delivered `torchmaster.hmod` **34,758 B**, SHA-256
**`51042af1bcf1d087f6c0b11b048502df53ea10a3ea70663e9c5941c17d3ce6d8`**, 40 entries,
no `Cargo.lock`; `check` names `0.2.3`, `0a875da  made for` and `0b14f8a  fits`
(`flare/build-check-fits.log`), and `verify-sdk-package.py torchmaster` is **1/1** with
`package matches the tree: every packed file is the tree's bytes`.

In game on that build (`ports/_shared/evidence/torchmaster-e2e/`):
`PASS: … flare=7 lights, after break 0, stone control 0 …` + `EXIT:0`;
`SELFTEST=flare` inverted the placed-lantern assertion, saw 7 lights and ended
`FAIL` + `EXIT:1`; the unmodded `CONTROL=1` leg and the original `SELFTEST=1` leg both redid
their `EXIT:1`; `check-e2e-logs.py torchmaster-e2e --strict` → `logs=4 ok=4`.
The dialect leg proved all **three** recipes and loot tables on the delivered archive
(megatorch, dreadlamp, flare), `PASS` + `EXIT:0`, with its `SELFTEST=1` `FAIL` + `EXIT:1`.

Harness note: `ctl light` prints the stored light as **hex** (`15` → `"f"`), so the flare
assertion parses `16#$value` before comparing; the first attempt failed on that formatting
only (`[: f: integer expected`), not on the mod.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

## Next slice

**Family:** the frozen pearl / residual-light cleanup (the remaining half of the flare family
that is still expressible at this tier).
**Gap line:** "The frozen pearl, the line-of-sight menu/GUI, the volume renderer and the
feral-lantern spawner interactions are still not carried" (§0.2.3 above) and the
`blocked(persistence)` row in the coverage table.
**Tier fit:** `logic` — a right-clickable item that scans loaded blocks for
`torchmaster:invisible_light` and removes them, with a durability component; no GUI, renderer
or entity work. The pin stamps `LevelRef::remove_block` and the item-use hooks.
**In-game assertion:** place a lantern, let it write lights, break it with a residual light left
in place (or load a world with one), then right-click a frozen pearl at that cell; `ctl blocks
-8 200 -8 8 205 8 | jq -r '.counts["torchmaster:invisible_light"]'` must go from `> 0` to `0`,
while the same pearl used on a vanilla-lit cell changes no count and does not lose a charge.
`SELFTEST=pearl` inverts the removal assertion and ends `EXIT:1`.
