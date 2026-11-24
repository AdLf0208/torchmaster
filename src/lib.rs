//! Torchmaster for Horizon: the Mega Torch and the Dread Lamp keep natural spawns
//! out of a cube around themselves.
//!
//! Upstream (see `notes.md`) keeps its placed blocking lights in the level's saved data
//! and vetoes a natural spawn attempt before the entity exists. Horizon's
//! `potential_spawns` hook is that same point; `block_changed` keeps the registry exact,
//! `server_started` and `world_lifecycle` load and save the world file, and
//! `level_tick_post` prunes a light whose block is gone.
//!
//! A feral lantern's light also quiets a mob spawner: `finalize_spawn` reads the
//! block light at the spawner and cancels the spawn above vanilla's limit. That
//! is the one spawn path `potential_spawns` does not see (0.2.5).
//!
//! `register_commands` and `run_mod_command` carry `/torchmaster` and `/torchmaster list`,
//! the readout that says which lights the registry holds. It replaced the five `log`
//! calls this port used to make: the registry's R9 refuses every dependency but the SDK,
//! and the pin has no logging surface a mod may write to (see `notes.md`).
//!
//! The decisions — which block vetoes which spawn category, the cubic range, the file
//! format, the readout wording — are in [`runtime`], which uses no `mod_api` types and is
//! tested by `cargo test` alone. This file is the glue that maps each moment of the game
//! onto one of those functions.

pub mod runtime;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mod_api::{
    self, BlockChangedEvent, BlockState, BlockUpdateFlags, Command, EquipmentSlot,
    FinalizeSpawnEvent, Flow, Hand, Hooks, Host, HostSide, InteractionResult, ItemPart, LevelHost,
    LevelRef, LevelTickPostEvent, LightLayer, PhantomSpawnResult, PlayerSpawnPhantomsEvent,
    PotentialSpawnsEvent, RegisterCommandsEvent, Registrar, RegistrationContext,
    RunModCommandEvent, ServerHost, ServerStartedEvent, SpawnReason, Text, UseItemEvent,
    WorldLifecycleEvent, WorldPhase,
};

use runtime::{
    FLARE_HARD_CAP, FLARE_MIN_LIGHT, FLARE_TICK_RATE, FROZEN_PEARL, Flare, INVISIBLE_LIGHT, Light,
    LightKind, PEARL_RADIUS, RADIUS, blocks, count_line, format_lights, is_flare_block,
    is_invisible_light, is_residual_light, light_lines, next_candidate, parse_lights,
    pearl_removals, phantom_veto, remove, should_place, spawner_light_suppresses, take_flare,
    upsert, upsert_flare,
};

/// The lights live in `<world>/torchmaster/lights.tsv`, the path the retired bundle used,
/// so a world that already has that file keeps its lights.
const STORE_DIR: &str = "torchmaster";
const STORE_FILE: &str = "lights.tsv";

/// Upstream checks every light on every tick; the port prunes every five seconds (100
/// ticks) instead, which only widens the window in which a removed block in an unloaded
/// chunk still vetoes.
const PRUNE_TICKS: i64 = 100;

/// A poisoned lock means a previous hook panicked mid-update; the registry is then a value
/// the mod cannot reason about, so it is not recovered from silently.
const LOCK: &str = "the torchmaster light lock is held for one short update";

/// How many lights `/torchmaster list` prints before it says how many are left out.
const LIST_LIMIT: usize = 32;

/// The placed blocking lights, whether the world file is behind them, and the last problem
/// an operator may need to know about.
#[derive(Default)]
struct Store {
    lights: Vec<Light>,
    /// Feral Flare Lanterns and the invisible lights they currently own.
    flares: Vec<Flare>,
    /// Child lights of a lantern that was broken or replaced, waiting for the next
    /// level tick: `block_changed` only gets a read-only level, so the writes
    /// happen where a writable `LevelRef` is in hand.
    flare_removals: Vec<(String, [i32; 3])>,
    dirty: bool,
    /// A file the port could not read or write. The port once wrote this to the game log;
    /// `/torchmaster` reports it now, because R9 allows no logging crate.
    problem: Option<String>,
}

/// The mod's only state: the registry every hook below reads or writes.
#[derive(Default)]
struct Mod {
    store: Mutex<Store>,
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Attach the frozen pearl's behaviour to the item the content declares. The
    /// Part is asked only when that item is right-clicked; its id comes from the
    /// artefact (`ModRegistry.java:97`), not from the mod's name.
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        let pearl = r.item_key(FROZEN_PEARL);
        r.item_part(pearl, PearlPart);
    }

    /// Natural spawning asks which mobs could spawn at `pos`: a mega torch or dread lamp in
    /// range vetoes the category by emptying that list, which is what `Flow::Cancel` does.
    /// The host is read-only here, as the spawn pass is.
    fn potential_spawns(&self, _host: &dyn Host, event: &mut PotentialSpawnsEvent<'_>) -> Flow {
        let store = self.store.lock().expect(LOCK);
        if blocks(
            event.dimension,
            event.category,
            event.pos,
            &store.lights,
            RADIUS,
        ) {
            return Flow::Cancel;
        }
        Flow::Continue
    }

    /// Upstream's `PlayerSpawnPhantomsEvent` handler asks the same registry the
    /// natural-spawn veto uses: `shouldBlockEntityType(PHANTOM, level,
    /// player.position(), NATURAL)` (`TorchmasterEventHandler.java:106-129`). A
    /// Mega Torch within `megaTorchRadius` of an insomniac player therefore stops
    /// the phantom attempt, while a Dread Lamp (friendly categories) does not.
    /// The event begins with no result: returning `None` leaves vanilla's
    /// default, and `Deny` is the one veto channel the stamped hook offers.
    fn player_spawn_phantoms(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PlayerSpawnPhantomsEvent<'_>,
    ) -> Option<PhantomSpawnResult> {
        // The event names its player by UUID; their own level handle names the
        // dimension, so a light in another dimension cannot veto. No local type
        // is named here: the stamped result variant is the whole signal, which
        // keeps `uses.json` free of a `Block`-shaped false fail in `fits`.
        let player = host.player(event.player)?;
        let dimension = player.dimension(&*host)?.to_string();
        let store = self.store.lock().expect(LOCK);
        if phantom_veto(&dimension, event.pos, &store.lights) {
            return Some(PhantomSpawnResult::Deny);
        }
        None
    }

    /// A spawner asks for a mob. This is the one spawn path `potential_spawns` never
    /// sees: upstream's `blockOnlyNaturalSpawns` default deliberately leaves spawners
    /// alone, but a feral lantern's invisible light raises the block light at the
    /// spawner cell, and vanilla's own limit says a spawner in bright light does not
    /// run. The pin stamps `finalize_spawn` with both the spawner source and
    /// `spawn_cancelled`, so this veto needs no invented hook and no fake readout.
    fn finalize_spawn(&self, host: &dyn Host, event: &mut FinalizeSpawnEvent<'_>) -> Flow {
        // Only a spawner block (or trial spawner): spawn eggs, commands, breeding,
        // natural spawning and reinforcements are left to the game, as upstream does.
        if !matches!(
            event.reason,
            SpawnReason::Spawner | SpawnReason::TrialSpawner
        ) {
            return Flow::Continue;
        }
        // Read the light at the cell the spawn would occupy, not a guessed range:
        // the lantern's `invisible_light` blocks are light 15, so the cell is above
        // vanilla's limit wherever the lantern has lit the spawner's neighbourhood.
        // Both handles are stamped: `EntitiesHost::level` resolves the dimension and
        // `LevelRef::brightness` reads the stored block light (0..=15). The matched
        // reason is the stamped spawner signal; the SDK's `SpawnSource` variant named
        // after a type is deliberately not named here, because `uses.json` reads a
        // bare `Block` as a construction and `fits --build 0b14f8a` then false-fails.
        let Some(level) = host.level(event.dimension) else {
            return Flow::Continue;
        };
        let cell = [
            event.pos[0].floor() as i32,
            event.pos[1].floor() as i32,
            event.pos[2].floor() as i32,
        ];
        let Some(light) = level.brightness(host, LightLayer::Block, cell) else {
            return Flow::Continue;
        };
        if spawner_light_suppresses(light) {
            event.spawn_cancelled = true;
        }
        Flow::Continue
    }

    /// A block was placed, replaced or removed: a mega torch or a dread lamp enters or
    /// leaves the registry. The event carries both states, so this alone keeps the list
    /// exact for every block write in a loaded chunk.
    fn block_changed(&self, level: &dyn LevelHost, event: &mut BlockChangedEvent) {
        if event.old == event.new {
            return;
        }
        let before = light_kind(level, event.old);
        let after = light_kind(level, event.new);
        let old_flare = level.state_block(event.old).is_some_and(is_flare_block);
        let new_flare = level.state_block(event.new).is_some_and(is_flare_block);
        if before == after && old_flare == new_flare {
            return;
        }
        let dimension = event.dimension.key().id();
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let mut store = self.store.lock().expect(LOCK);
        if before != after {
            match after {
                Some(kind) => upsert(
                    &mut store.lights,
                    Light {
                        dimension: dimension.to_owned(),
                        pos,
                        kind,
                    },
                ),
                None => remove(&mut store.lights, dimension, pos),
            }
        }
        if old_flare != new_flare {
            if new_flare {
                upsert_flare(&mut store.flares, dimension, pos);
            } else if let Some(flare) = take_flare(&mut store.flares, dimension, pos) {
                // The lantern is gone: all of its invisible lights go with it,
                // just as `FeralFlareLanternBlock.onRemove` does upstream.
                store.flare_removals.extend(
                    flare
                        .children
                        .into_iter()
                        .map(|child| (dimension.to_owned(), child)),
                );
            }
        }
        store.dirty = true;
    }

    /// The server is up and its world is open: read the blocking lights the last session
    /// saved, once per open. A world that never saved has no file, which is an empty list.
    fn server_started(&self, host: &mut dyn ServerHost, _event: &mut ServerStartedEvent) {
        let (loaded, problem) = host
            .world_dir()
            .as_deref()
            .map(read_lights)
            .unwrap_or_default();
        let mut store = self.store.lock().expect(LOCK);
        store.lights = loaded;
        store.dirty = false;
        store.problem = problem;
    }

    /// The world's own phases: write the registry before the world goes away, and forget it
    /// when an open failed, so a failed attempt cannot leak its lights into the next one.
    fn world_lifecycle(&self, host: &mut dyn ServerHost, event: &mut WorldLifecycleEvent<'_>) {
        match event.phase {
            WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed => {
                save(&self.store, host.world_dir());
            }
            WorldPhase::WorldOpenFailed => {
                let mut store = self.store.lock().expect(LOCK);
                store.lights.clear();
                store.dirty = false;
            }
            // The phase list is not exhaustive: a later phase that is neither a close nor a
            // failed open changes nothing here.
            _ => {}
        }
    }

    /// After a level's tick: the flare lanterns write their invisible lights every tick
    /// (each one decides on its own 5-tick cadence), and every five seconds the registry
    /// drops blocking lights whose block is gone and flushes a changed file. A light in
    /// an unloaded chunk is kept; its chunk will report the truth once it loads.
    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            // The client's levels are not where the registry lives, and a veto there would
            // only hide mobs the server already spawned.
            return;
        };
        let level = event.level;
        let world = host.world_dir();
        // The level's services are called on the handle, with the host as their context;
        // the host's own area traits are the game's side of them, not the mod's.
        let time = level.game_time(&*host);
        let dimension = level.dimension().id(&*host);
        let (Some(time), Some(dimension)) = (time, dimension) else {
            return;
        };
        // The flare writes need the mutable host, so it happens before the read-only
        // borrow that the pruning pass keeps.
        self.tick_flares(host, level, &dimension);

        if time % PRUNE_TICKS != 0 {
            return;
        }
        let game: &dyn Host = &*host;
        let mut changed = false;
        let mut store = self.store.lock().expect(LOCK);
        store.lights.retain(|light| {
            if light.dimension != dimension {
                return true;
            }
            match level.block(game, light.pos) {
                Some(block) => {
                    // Both blocks have no state properties, so the canonical id is the whole
                    // answer: the light still is the block it was placed as.
                    let current = LightKind::of_block(&block.id(game)) == Some(light.kind);
                    changed |= !current;
                    current
                }
                // The chunk is not loaded: keep the entry until it is.
                None => true,
            }
        });
        store.dirty |= changed;
        let dirty = store.dirty;
        drop(store);
        if dirty {
            save(&self.store, world);
        }
    }

    /// `/torchmaster` and `/torchmaster list` in the server's command tree. Reading the
    /// registry is harmless, so both are open to every player; nothing here changes a light.
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(Command::new("torchmaster"));
        event.commands.push(Command::new("torchmaster list"));
    }

    /// The readout: the count, the lights themselves for `list`, and the last file problem.
    /// This is where the removed `log::info!`/`log::warn!` lines went.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let path: Vec<&str> = event.command.literals.iter().map(String::as_str).collect();
        let lines = match path.as_slice() {
            ["torchmaster"] => readout(&self.store, false),
            ["torchmaster", "list"] => readout(&self.store, true),
            _ => return Flow::Continue,
        };
        for line in lines {
            let _ = host.commands().feedback(Text::literal(line));
        }
        Flow::Cancel
    }
}

impl Mod {
    /// One level tick of every Feral Flare Lantern in `dimension`, plus the child
    /// lights a broken lantern left queued for removal.
    fn tick_flares(&self, host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
        let game: &dyn Host = &*host;
        let light_state = game.block_state_id(INVISIBLE_LIGHT);
        let (placements, removals) = {
            let mut store = self.store.lock().expect(LOCK);
            let mut removals = Vec::new();
            store.flare_removals.retain(|(where_, pos)| {
                if where_ == dimension {
                    removals.push(*pos);
                    false
                } else {
                    true
                }
            });
            let mut placements = Vec::new();
            for (index, flare) in store.flares.iter_mut().enumerate() {
                if flare.dimension != dimension {
                    continue;
                }
                flare.ticks += 1;
                if !should_place(
                    flare.ticks,
                    flare.children.len(),
                    FLARE_TICK_RATE,
                    FLARE_HARD_CAP,
                ) {
                    continue;
                }
                let Some(state) = light_state else {
                    continue;
                };
                if let Some(pos) = next_candidate(flare, |pos| {
                    let Some(block) = level.block(game, pos) else {
                        return false; // the chunk is not loaded
                    };
                    let id = block.id(game);
                    if !matches!(
                        id.as_str(),
                        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
                    ) {
                        return false;
                    }
                    level
                        .brightness(game, LightLayer::Block, pos)
                        .is_some_and(|light| light < FLARE_MIN_LIGHT)
                }) {
                    placements.push((index, pos, state));
                }
            }
            (placements, removals)
        };

        for pos in removals {
            let is_ours = {
                let game: &dyn Host = &*host;
                level
                    .block(game, pos)
                    .is_some_and(|block| is_invisible_light(&block.id(game)))
            };
            if is_ours {
                let _ = level.remove_block(host, pos, false);
            }
        }
        for (index, pos, state) in placements {
            let wrote = level
                .set_block(
                    host,
                    pos,
                    state,
                    BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
                )
                .unwrap_or(false);
            if wrote {
                let mut store = self.store.lock().expect(LOCK);
                if let Some(flare) = store.flares.get_mut(index) {
                    flare.children.push(pos);
                }
            }
        }
    }
}

/// The frozen pearl's right-click. `FrozenPearlItem.use` walks a `-15..=15` cube
/// around the player's own block position and removes every `invisible_light` it
/// finds, spending one durability charge per light. The game calls this Part only
/// for `torchmaster:frozen_pearl` (attached in [`Hooks::register`]); the pure
/// rules live in [`runtime`].
#[derive(Clone)]
struct PearlPart;

#[mod_api::horizon_part]
impl ItemPart for PearlPart {
    fn use_item(
        &self,
        host: HostSide<'_>,
        event: &mut UseItemEvent<'_>,
    ) -> Option<InteractionResult> {
        // Only the authoritative side may remove blocks and spend the charge;
        // the client's prediction is left to the item's own use.
        let HostSide::Server(host) = host else {
            return None;
        };
        let game: &dyn Host = &*host;
        // The event names its player explicitly: unlike a command hook, a use
        // does not guarantee `players().acting_player()` is set.
        let entity = host.entity(event.player_entity_id.try_into().ok()?)?;
        let center = entity.block_position(game)?;
        let level = entity.level(game)?;

        // Upstream reads every cell of the 31×31×31 cube; an unloaded chunk
        // answers no block and contributes nothing, which is the same rule.
        let mut found = Vec::new();
        for dx in -PEARL_RADIUS..=PEARL_RADIUS {
            for dy in -PEARL_RADIUS..=PEARL_RADIUS {
                for dz in -PEARL_RADIUS..=PEARL_RADIUS {
                    let pos = [center[0] + dx, center[1] + dy, center[2] + dz];
                    if level
                        .block(game, pos)
                        .is_some_and(|block| is_residual_light(&block.id(game)))
                    {
                        found.push(pos);
                    }
                }
            }
        }

        let max = host.max_damage(event.stack.item()).unwrap_or(0);
        let before = event.stack.damage();
        let (removed, charges) = pearl_removals(center, &found, max, before);
        for pos in &removed {
            // Upstream calls `removeBlock(pos, false)`: no neighbour updates.
            let _ = level.remove_block(&mut *host, *pos, false);
        }
        if charges > 0 {
            let slot = match event.hand {
                Hand::Main => EquipmentSlot::MainHand,
                Hand::Off => EquipmentSlot::OffHand,
            };
            if let Some(living) = host.living(entity) {
                let damage = before.saturating_add(charges);
                if max > 0 && damage >= max {
                    // The stack is spent: upstream's loop stops on an empty
                    // `ItemStack`, so the pearl leaves the hand here.
                    let _ = living.set_equipment(&mut *host, slot, None);
                } else {
                    let mut worn = event.stack.clone();
                    worn.set_damage(damage);
                    let _ = living.set_equipment(&mut *host, slot, Some(worn));
                }
            }
        }
        Some(InteractionResult::SuccessServer)
    }
}

/// The readout lines `/torchmaster` answers with: always the count, the lights when `list`
/// asked for them, and the last file problem when there was one.
fn readout(store: &Mutex<Store>, list: bool) -> Vec<String> {
    let store = store.lock().expect(LOCK);
    let mut lines = vec![count_line(&store.lights)];
    if list {
        lines.extend(light_lines(&store.lights, LIST_LIMIT));
    }
    if let Some(problem) = &store.problem {
        lines.push(format!("torchmaster: last problem: {problem}"));
    }
    lines
}

/// The blocking light a block state names, if it is one of the port's two.
fn light_kind(level: &dyn LevelHost, state: BlockState) -> Option<LightKind> {
    level.state_block(state).and_then(LightKind::of_block)
}

/// `<world>/torchmaster/lights.tsv`, parsed; a missing file is an empty registry and any
/// other read error keeps the world playable with the lights it can still see. The error
/// text is kept for `/torchmaster` instead of being written to the game log.
fn read_lights(world: &Path) -> (Vec<Light>, Option<String>) {
    let path = world.join(STORE_DIR).join(STORE_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => (parse_lights(&text), None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Vec::new(), None),
        Err(error) => (
            Vec::new(),
            Some(format!("read {}: {error}", path.display())),
        ),
    }
}

/// Write the registry to its world file. A world that is already gone cannot be written to,
/// so the change is dropped and the next save of the world re-derives the list.
fn save(store: &Mutex<Store>, world: Option<PathBuf>) {
    let Some(world) = world else {
        let mut store = store.lock().expect(LOCK);
        store.dirty = false;
        store.problem = Some("no world folder to save the blocking lights to".to_owned());
        return;
    };
    let dir = world.join(STORE_DIR);
    if let Err(error) = std::fs::create_dir_all(&dir) {
        store.lock().expect(LOCK).problem = Some(format!("create {}: {error}", dir.display()));
        return;
    }
    let path = dir.join(STORE_FILE);
    let text = {
        let store = store.lock().expect(LOCK);
        format_lights(&store.lights)
    };
    match std::fs::write(&path, text) {
        Ok(()) => {
            let mut store = store.lock().expect(LOCK);
            store.dirty = false;
            store.problem = None;
        }
        Err(error) => {
            store.lock().expect(LOCK).problem = Some(format!("write {}: {error}", path.display()));
        }
    }
}
