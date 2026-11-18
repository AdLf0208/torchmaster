//! Torchmaster for Horizon: the Mega Torch and the Dread Lamp keep natural spawns
//! out of a cube around themselves.
//!
//! Upstream (see `notes.md`) keeps its placed blocking lights in the level's saved data
//! and vetoes a natural spawn attempt before the entity exists. Horizon's
//! `potential_spawns` hook is that same point; `block_changed` keeps the registry exact,
//! `server_started` and `world_lifecycle` load and save the world file, and
//! `level_tick_post` prunes a light whose block is gone.
//!
//! The decisions — which block vetoes which spawn category, the cubic range, the file
//! format — are in [`runtime`], which uses no `mod_api` types and is tested by
//! `cargo test` alone. This file is the glue that maps each moment of the game onto one
//! of those functions.

pub mod runtime;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mod_api::{
    self, BlockChangedEvent, BlockState, Flow, Hooks, Host, HostSide, LevelHost,
    LevelTickPostEvent, PotentialSpawnsEvent, ServerHost, ServerStartedEvent, WorldLifecycleEvent,
    WorldPhase,
};

use runtime::{Light, LightKind, RADIUS, blocks, format_lights, parse_lights, remove, upsert};

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

/// The placed blocking lights, and whether the world file is behind them.
#[derive(Default)]
struct Store {
    lights: Vec<Light>,
    dirty: bool,
}

/// The mod's only state: the registry every hook below reads or writes.
#[derive(Default)]
struct Mod {
    store: Mutex<Store>,
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
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

    /// A block was placed, replaced or removed: a mega torch or a dread lamp enters or
    /// leaves the registry. The event carries both states, so this alone keeps the list
    /// exact for every block write in a loaded chunk.
    fn block_changed(&self, level: &dyn LevelHost, event: &mut BlockChangedEvent) {
        if event.old == event.new {
            return;
        }
        let before = light_kind(level, event.old);
        let after = light_kind(level, event.new);
        if before == after {
            return;
        }
        let dimension = event.dimension.key().id();
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let mut store = self.store.lock().expect(LOCK);
        match after {
            Some(kind) => {
                log::info!(
                    "[torchmaster] registered {} at {dimension} {pos:?}",
                    kind.block()
                );
                upsert(
                    &mut store.lights,
                    Light {
                        dimension: dimension.to_owned(),
                        pos,
                        kind,
                    },
                );
            }
            None => {
                if let Some(kind) = before {
                    log::info!(
                        "[torchmaster] unregistered {} at {dimension} {pos:?}",
                        kind.block()
                    );
                }
                remove(&mut store.lights, dimension, pos);
            }
        }
        store.dirty = true;
    }

    /// The server is up and its world is open: read the blocking lights the last session
    /// saved, once per open. A world that never saved has no file, which is an empty list.
    fn server_started(&self, host: &mut dyn ServerHost, _event: &mut ServerStartedEvent) {
        let loaded = host
            .world_dir()
            .as_deref()
            .map(read_lights)
            .unwrap_or_default();
        log::info!("[torchmaster] loaded {} blocking lights", loaded.len());
        let mut store = self.store.lock().expect(LOCK);
        store.lights = loaded;
        store.dirty = false;
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

    /// After a level's tick: every five seconds, drop the lights of this dimension whose
    /// block is gone and flush a changed registry. A light in an unloaded chunk is kept;
    /// its chunk will report the truth once it loads.
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
        let game: &dyn Host = &*host;
        let Some(time) = level.game_time(game) else {
            return;
        };
        if time % PRUNE_TICKS != 0 {
            return;
        }
        let Some(dimension) = level.dimension().id(game) else {
            return;
        };
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
}

/// The blocking light a block state names, if it is one of the port's two.
fn light_kind(level: &dyn LevelHost, state: BlockState) -> Option<LightKind> {
    level.state_block(state).and_then(LightKind::of_block)
}

/// `<world>/torchmaster/lights.tsv`, parsed; a missing file is an empty registry and any
/// other read error keeps the world playable with the lights it can still see.
fn read_lights(world: &Path) -> Vec<Light> {
    let path = world.join(STORE_DIR).join(STORE_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_lights(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            log::warn!("[torchmaster] read {}: {error}", path.display());
            Vec::new()
        }
    }
}

/// Write the registry to its world file. A world that is already gone cannot be written to,
/// so the change is dropped and the next save of the world re-derives the list.
fn save(store: &Mutex<Store>, world: Option<PathBuf>) {
    let Some(world) = world else {
        log::warn!("[torchmaster] no world folder to save the blocking lights to");
        store.lock().expect(LOCK).dirty = false;
        return;
    };
    let dir = world.join(STORE_DIR);
    if let Err(error) = std::fs::create_dir_all(&dir) {
        log::warn!("[torchmaster] create {}: {error}", dir.display());
        return;
    }
    let path = dir.join(STORE_FILE);
    let text = {
        let store = store.lock().expect(LOCK);
        format_lights(&store.lights)
    };
    match std::fs::write(&path, text) {
        Ok(()) => store.lock().expect(LOCK).dirty = false,
        Err(error) => log::warn!("[torchmaster] write {}: {error}", path.display()),
    }
}
