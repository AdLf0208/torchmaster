// Game-side glue for Torchmaster: the placed blocking lights, their world
// file and the hook bodies. Injected into `crate::game` as
// `torchmaster_state`; it needs the game, so it is not part of the standalone
// library.
//
// Upstream keeps its lights in the level's saved data (`FilteredLightManager`
// extends `SavedData`) and verifies each one still names its block on every
// tick (`cleanupCheck`). The port keeps the same list per dimension in a small
// world file and prunes it on a slower cadence; the spawn veto itself is a
// pure lookup over the list.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use horizon_core::position::BlockPos;
use horizon_world::level::Dimension as LevelDimension;
use horizon_world::registry::BlockStateId;

use crate::game::native_lifecycle::WorldPhase;
use crate::game::torchmaster::{
    Light, LightKind, RADIUS, blocks, format_lights, parse_lights, remove, upsert,
};

/// `<world>/torchmaster/lights.tsv`.
const STORE_DIR: &str = "torchmaster";
const STORE_FILE: &str = "lights.tsv";

/// Upstream prunes every tick; the port does it every 5 seconds instead, which
/// only widens the window in which a removed-but-unloaded block still vetoes.
const PRUNE_TICKS: i64 = 100;

static LIGHTS: Mutex<Vec<Light>> = Mutex::new(Vec::new());
static DIRTY: AtomicBool = AtomicBool::new(false);

fn lights() -> std::sync::MutexGuard<'static, Vec<Light>> {
    LIGHTS.lock().expect("torchmaster light lock poisoned")
}

/// Natural spawning builds its possible spawn list at `pos` (API 388). A
/// mega torch or dread lamp in range vetoes its category by emptying the list.
pub fn on_potential_spawns(dimension: &str, category: &str, pos: [i32; 3], canceled: &mut bool) {
    if *canceled {
        return;
    }
    let lights = lights();
    if blocks(dimension, category, pos, &lights, RADIUS) {
        *canceled = true;
    }
}

/// A block was placed, replaced or removed (API 7): a torch or lamp enters or
/// leaves the registry. `old`/`new` carry the states, so the hook alone is
/// enough to keep the list exact.
#[allow(private_interfaces)]
pub fn on_block_changed(
    level: &crate::game::GameLevel<'_>,
    dimension: LevelDimension,
    pos: BlockPos,
    old: BlockStateId,
    new: BlockStateId,
) {
    if old == new {
        return;
    }
    let before = light_kind(level, old);
    let after = light_kind(level, new);
    if before == after {
        return;
    }
    let dimension = dimension_name(dimension);
    let pos = [pos.x, pos.y, pos.z];
    {
        let mut lights = lights();
        match after {
            Some(kind) => {
                log::info!(
                    "[torchmaster] registered {} at {dimension} {:?}",
                    kind.block(),
                    pos
                );
                upsert(
                    &mut lights,
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
                        "[torchmaster] unregistered {} at {dimension} {:?}",
                        kind.block(),
                        pos
                    );
                }
                remove(&mut lights, dimension, pos)
            }
        }
    }
    DIRTY.store(true, Ordering::Release);
}

/// The world opened or closed: load or save `<world>/torchmaster/lights.tsv`.
#[allow(private_interfaces)]
pub fn on_world_lifecycle(game: &mut crate::game::Game<'_>, phase: WorldPhase) {
    match phase {
        WorldPhase::WorldOpenReady => {
            let world = game.native_world_dir();
            let loaded = world
                .as_deref()
                .map(|world| read_lights(world))
                .unwrap_or_default();
            log::info!("[torchmaster] loaded {} blocking lights", loaded.len());
            *lights() = loaded;
            DIRTY.store(false, Ordering::Release);
        }
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed => {
            save(game.native_world_dir());
        }
        WorldPhase::WorldOpenFailed => {
            lights().clear();
            DIRTY.store(false, Ordering::Release);
        }
    }
}

/// End of a level tick (API 23): every few seconds, drop lights whose block is
/// gone and flush a changed registry. A light in an unloaded chunk is kept;
/// its chunk will report the truth once it loads.
#[allow(private_interfaces)]
pub fn on_level_tick_post(game: &mut crate::game::Game<'_>, dimension: &str) {
    if game.world.game_time % PRUNE_TICKS != 0 {
        return;
    }
    let Some(level) = game.native_level(dimension) else {
        return;
    };
    let mut changed = false;
    {
        let mut lights = lights();
        lights.retain(|light| {
            if light.dimension != dimension {
                return true;
            }
            match game.native_level_block(level, light.pos) {
                Some(name) => {
                    let current = LightKind::of_block(&name) == Some(light.kind);
                    changed |= !current;
                    current
                }
                // The chunk is not loaded: keep the entry until it is.
                None => true,
            }
        });
    }
    if changed {
        DIRTY.store(true, Ordering::Release);
    }
    if DIRTY.load(Ordering::Acquire) {
        save(game.native_world_dir());
    }
}

fn light_kind(level: &crate::game::GameLevel<'_>, state: BlockStateId) -> Option<LightKind> {
    level
        .registry
        .state(state)
        .and_then(|state| LightKind::of_block(&state.name))
}

fn dimension_name(dimension: LevelDimension) -> &'static str {
    match dimension {
        LevelDimension::Overworld => "minecraft:overworld",
        LevelDimension::Nether => "minecraft:the_nether",
        LevelDimension::End => "minecraft:the_end",
    }
}

fn read_lights(world: &Path) -> Vec<Light> {
    let path = world.join(STORE_DIR).join(STORE_FILE);
    match fs::read_to_string(&path) {
        Ok(text) => parse_lights(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            log::warn!("[torchmaster] read {}: {error}", path.display());
            Vec::new()
        }
    }
}

fn save(world: Option<PathBuf>) {
    let Some(world) = world else {
        DIRTY.store(false, Ordering::Release);
        return;
    };
    let dir = world.join(STORE_DIR);
    if let Err(error) = fs::create_dir_all(&dir) {
        log::warn!("[torchmaster] create {}: {error}", dir.display());
        return;
    }
    let path = dir.join(STORE_FILE);
    let text = format_lights(&lights());
    match fs::write(&path, text) {
        Ok(()) => DIRTY.store(false, Ordering::Release),
        Err(error) => log::warn!("[torchmaster] write {}: {error}", path.display()),
    }
}
