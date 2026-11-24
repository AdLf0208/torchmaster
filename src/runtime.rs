// Torchmaster for Horizon — the spawn-prevention engine (pure, no game access).
//
// Ported from Torchmaster 21.1.14 (mod id `torchmaster`, MIT) for Minecraft
// 1.21.1 NeoForge. Local-only until the maintainer approves publishing.
//
// Behaviour, from the mod's public documentation and upstream source:
// 1. The **Mega Torch** blocks natural spawns of non-friendly entities (in
//    vanilla, the `monster` category) in a cube of `megaTorchRadius` (default
//    64) around it (`MegatorchEntityBlockingLight.java:27-33`,
//    `VanillaCompat.registerTorchEntities`, `IDistanceLogic.Cubic`).
// 2. The **Dread Lamp** blocks natural spawns of friendly entities in a cube
//    of `dreadLampRadius` (default 64), excluding `MISC`
//    (`DreadLampEntityBlockingLight.java:24-30`,
//    `VanillaCompat.registerDreadLampEntities`).
// 3. The default `blockOnlyNaturalSpawns = true` means only the natural spawn
//    path is vetoed; spawners and every other intentional spawn is untouched.
//    The pin's `potential_spawns` hook is exactly that path.
//
// `IDistanceLogic.Cubic` tests the entity position against
// `[torch - r, torch + r + 1]` on each axis. A mob's x/z are block centre
// (`x + 0.5`) and its y is the feet block's y, so the reachable block cells are
// `[torch - r, torch + r]` on x/z and `[torch - r, torch + r + 1]` on y.
// `in_range` reproduces that exactly instead of silently squaring the cube.
//
// The Feral Flare Lantern (0.2.3) is a different family: it owns no spawn veto. It
// writes `torchmaster:invisible_light` blocks into empty, dark cells around itself
// and removes exactly those when it breaks. Its ids, light level and placement
// numbers are read from the artefact (`ModRegistry.java:82-107`,
// `FeralFlareLanternBlockEntity.java:65-124`, `TorchmasterNeoforgeConfig.java:99-120`).

use std::sync::OnceLock;

pub const MEGA_TORCH: &str = "torchmaster:megatorch";
pub const DREAD_LAMP: &str = "torchmaster:dreadlamp";

/// Upstream's `megaTorchRadius` / `dreadLampRadius` default
/// (`TorchmasterNeoforgeConfig.java:67-76`).
pub const RADIUS: i32 = 64;

/// The lantern block id (`ModRegistry.java:82`).
pub const FERAL_FLARE_LANTERN: &str = "torchmaster:feral_flare_lantern";
/// The invisible light block it writes (`ModRegistry.java:100`).
pub const INVISIBLE_LIGHT: &str = "torchmaster:invisible_light";
/// `feralFlareRadius` (NeoForge default 16, `TorchmasterNeoforgeConfig.java:99-102`).
pub const FLARE_RADIUS: i32 = 16;
/// `feralFlareTickRate` (NeoForge default 5, `TorchmasterNeoforgeConfig.java:104-107`).
pub const FLARE_TICK_RATE: u64 = 5;
/// `feralFlareMinLightLevel` (NeoForge default 10, `TorchmasterNeoforgeConfig.java:109-112`).
pub const FLARE_MIN_LIGHT: i32 = 10;
/// `feralFlareLanternLightCountHardcap` (NeoForge default 255, `:114-120`).
pub const FLARE_HARD_CAP: usize = 255;
/// Both blocks declare `.lightLevel(state -> 15)` (`ModRegistry.java:87,102`).
pub const FLARE_LIGHT_LEVEL: u8 = 15;

/// The two blocking lights this slice carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    MegaTorch,
    DreadLamp,
}

impl LightKind {
    /// The light a block id names, if it is one of the port's blocking lights.
    #[must_use]
    pub fn of_block(name: &str) -> Option<Self> {
        match name {
            MEGA_TORCH => Some(Self::MegaTorch),
            DREAD_LAMP => Some(Self::DreadLamp),
            _ => None,
        }
    }

    /// The block id this light is placed as.
    #[must_use]
    pub fn block(self) -> &'static str {
        match self {
            Self::MegaTorch => MEGA_TORCH,
            Self::DreadLamp => DREAD_LAMP,
        }
    }

    /// The word the world file and the `/torchmaster` readout use for this light.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::MegaTorch => "megatorch",
            Self::DreadLamp => "dreadlamp",
        }
    }

    /// The light a world-file word names.
    #[must_use]
    pub fn of_name(name: &str) -> Option<Self> {
        match name {
            "megatorch" => Some(Self::MegaTorch),
            "dreadlamp" => Some(Self::DreadLamp),
            _ => None,
        }
    }

    /// Whether this light vetoes a spawn of `category` (`MobCategory.name()`).
    /// The mega torch takes every non-friendly category (`monster`); the dread
    /// lamp takes every friendly one except `misc`.
    #[must_use]
    pub fn blocks_category(self, category: &str) -> bool {
        match self {
            Self::MegaTorch => category == "monster",
            Self::DreadLamp => !matches!(category, "monster" | "misc"),
        }
    }
}

/// One placed blocking light.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Light {
    pub dimension: String,
    pub pos: [i32; 3],
    pub kind: LightKind,
}

/// `IDistanceLogic.Cubic.isPositionInRange` for a spawn attempt at `pos`.
#[must_use]
pub fn in_range(light: [i32; 3], pos: [i32; 3], radius: i32) -> bool {
    // The entity is at the block centre on x/z and at the feet block's y;
    // upstream's `[torch - r, torch + r + 1]` therefore covers
    // `[torch - r, torch + r]` in whole x/z cells and one extra y layer.
    let x = f64::from(pos[0]) + 0.5;
    let y = f64::from(pos[1]);
    let z = f64::from(pos[2]) + 0.5;
    let reach = f64::from(radius);
    let within = |value: f64, centre: i32| {
        value >= f64::from(centre) - reach && value <= f64::from(centre) + reach + 1.0
    };
    within(x, light[0]) && within(y, light[1]) && within(z, light[2])
}

/// Whether a candidate spawn at `pos` is vetoed by any light in `lights`.
#[must_use]
pub fn blocks(
    dimension: &str,
    category: &str,
    pos: [i32; 3],
    lights: &[Light],
    radius: i32,
) -> bool {
    lights.iter().any(|light| {
        light.dimension == dimension
            && light.kind.blocks_category(category)
            && in_range(light.pos, pos, radius)
    })
}

/// Insert `light`, replacing a light already at the same dimension and
/// position (a mega torch swapped for a dread lamp keeps one entry).
pub fn upsert(lights: &mut Vec<Light>, light: Light) {
    match lights
        .iter_mut()
        .find(|held| held.dimension == light.dimension && held.pos == light.pos)
    {
        Some(held) => *held = light,
        None => lights.push(light),
    }
}

/// Remove the light at `dimension`/`pos`, if any.
pub fn remove(lights: &mut Vec<Light>, dimension: &str, pos: [i32; 3]) {
    lights.retain(|light| !(light.dimension == dimension && light.pos == pos));
}

/// Parse `<world>/torchmaster/lights.tsv`: `dimension<TAB>x<TAB>y<TAB>z<TAB>kind`.
/// Blank lines, unknown kinds and malformed numbers are ignored.
#[must_use]
pub fn parse_lights(text: &str) -> Vec<Light> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let dimension = fields.next()?;
            let x = fields.next()?.parse::<i32>().ok()?;
            let y = fields.next()?.parse::<i32>().ok()?;
            let z = fields.next()?.parse::<i32>().ok()?;
            let kind = LightKind::of_name(fields.next()?)?;
            if dimension.is_empty() || fields.next().is_some() {
                return None;
            }
            Some(Light {
                dimension: dimension.to_owned(),
                pos: [x, y, z],
                kind,
            })
        })
        .collect()
}

/// The persisted form of `lights`, one light per line.
#[must_use]
pub fn format_lights(lights: &[Light]) -> String {
    let mut text = String::new();
    for light in lights {
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            light.dimension,
            light.pos[0],
            light.pos[1],
            light.pos[2],
            light.kind.name()
        ));
    }
    text
}

/// The one-line summary the `/torchmaster` readout prints: how many blocking lights the
/// registry holds, per kind. This is what the removed `log::info!` call said when a world
/// loaded its lights.
#[must_use]
pub fn count_line(lights: &[Light]) -> String {
    if lights.is_empty() {
        return "torchmaster: no blocking lights".to_owned();
    }
    let mega = lights
        .iter()
        .filter(|light| light.kind == LightKind::MegaTorch)
        .count();
    let dread = lights.len() - mega;
    let mut kinds = Vec::new();
    if mega > 0 {
        kinds.push(format!("{mega} megatorch"));
    }
    if dread > 0 {
        kinds.push(format!("{dread} dreadlamp"));
    }
    let plural = if lights.len() == 1 { "" } else { "s" };
    format!(
        "torchmaster: {} blocking light{plural}: {}",
        lights.len(),
        kinds.join(", ")
    )
}

/// One readout line per light, in registry order, at most `max` of them: the placed light's
/// dimension, position and kind. A longer registry ends with a line saying how many were
/// left out, so the readout is never silently truncated.
#[must_use]
pub fn light_lines(lights: &[Light], max: usize) -> Vec<String> {
    let mut lines: Vec<String> = lights
        .iter()
        .take(max)
        .map(|light| {
            format!(
                "torchmaster: {} {} {} {} {}",
                light.dimension,
                light.pos[0],
                light.pos[1],
                light.pos[2],
                light.kind.name()
            )
        })
        .collect();
    if lights.len() > max {
        lines.push(format!("torchmaster: … {} more", lights.len() - max));
    }
    lines
}

/// One placed Feral Flare Lantern and the invisible lights it currently owns.
///
/// Upstream keeps this in the lantern's block entity (`lights`, `ticks`,
/// `useLoS` in `FeralFlareLanternBlockEntity.java:132-159`). The port keeps the
/// same shape in the mod's store: the lights live until the lantern breaks or
/// is replaced, when all of them are removed (`removeChildLights`, `:177-188`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flare {
    pub dimension: String,
    pub pos: [i32; 3],
    pub children: Vec<[i32; 3]>,
    /// Where the next placement attempt resumes in [`flare_offsets`].
    pub cursor: usize,
    /// Ticks since placement; placement is attempted every [`FLARE_TICK_RATE`].
    pub ticks: u64,
}

impl Flare {
    /// A freshly placed lantern owns no lights and starts at the nearest cell.
    #[must_use]
    pub fn new(dimension: String, pos: [i32; 3]) -> Self {
        Self {
            dimension,
            pos,
            children: Vec::new(),
            cursor: 0,
            ticks: 0,
        }
    }
}

/// Whether a block id names the lantern (as opposed to either blocking light).
#[must_use]
pub fn is_flare_block(name: &str) -> bool {
    name == FERAL_FLARE_LANTERN
}

/// Whether a block id names the invisible light the lantern writes.
#[must_use]
pub fn is_invisible_light(name: &str) -> bool {
    name == INVISIBLE_LIGHT
}

/// The candidate offsets, nearest first, for one placement attempt.
///
/// Upstream picks a uniformly random cell of a `2r × 2r × 2r` cube
/// (`FeralFlareLanternBlockEntity.java:78-84`). The port walks the same radius in
/// a fixed, nearest-first order instead: the pin exposes no per-lantern random
/// stream at the level tick, and a deterministic order makes the placement
/// observable within a bounded in-game wait and reproducible in a unit test.
/// The set of cells, the empty/dark checks and the cap are unchanged.
#[must_use]
pub fn flare_offsets() -> &'static [[i32; 3]] {
    static OFFSETS: OnceLock<Vec<[i32; 3]>> = OnceLock::new();
    OFFSETS.get_or_init(|| {
        let axis: Vec<i32> = (0..=FLARE_RADIUS)
            .flat_map(|d| if d == 0 { vec![0] } else { vec![d, -d] })
            .collect();
        let mut offsets = Vec::with_capacity(axis.len().saturating_pow(3));
        for &x in &axis {
            for &y in &axis {
                for &z in &axis {
                    offsets.push([x, y, z]);
                }
            }
        }
        offsets.sort_by_key(|o| {
            let max = o.iter().map(|v| v.abs()).max().unwrap_or(0);
            let manhattan = o.iter().map(|v| v.abs()).sum::<i32>();
            (max, manhattan, o[0], o[1], o[2])
        });
        offsets
    })
}

/// The next empty, dark cell in the lantern's radius, or `None` if none is left.
///
/// `valid` is the game-side check (loaded, `minecraft:air`, block light below
/// [`FLARE_MIN_LIGHT`]); keeping it a closure leaves this function pure.
pub fn next_candidate<F>(flare: &mut Flare, valid: F) -> Option<[i32; 3]>
where
    F: Fn([i32; 3]) -> bool,
{
    let offsets = flare_offsets();
    for step in 0..offsets.len() {
        let index = (flare.cursor + step) % offsets.len();
        let [dx, dy, dz] = offsets[index];
        let pos = [flare.pos[0] + dx, flare.pos[1] + dy, flare.pos[2] + dz];
        if valid(pos) {
            flare.cursor = (index + 1) % offsets.len();
            return Some(pos);
        }
    }
    None
}

/// Whether this tick should place a light: every `rate` ticks and below the cap.
///
/// Upstream tests `childLights.size() > hardcap`, which places one more light
/// than the configured cap. The port uses `>=`, so 255 really is the maximum.
#[must_use]
pub fn should_place(ticks: u64, children: usize, rate: u64, hardcap: usize) -> bool {
    rate > 0 && ticks > 0 && ticks.is_multiple_of(rate) && children < hardcap
}

/// Register a placed lantern, keeping one entry per dimension and position.
pub fn upsert_flare(flares: &mut Vec<Flare>, dimension: &str, pos: [i32; 3]) {
    if !flares
        .iter()
        .any(|flare| flare.dimension == dimension && flare.pos == pos)
    {
        flares.push(Flare::new(dimension.to_owned(), pos));
    }
}

/// Remove and return the lantern at `dimension`/`pos`, with its child lights.
pub fn take_flare(flares: &mut Vec<Flare>, dimension: &str, pos: [i32; 3]) -> Option<Flare> {
    flares
        .iter()
        .position(|flare| flare.dimension == dimension && flare.pos == pos)
        .map(|index| flares.remove(index))
}

// ---- the Frozen Pearl and residual-light cleanup (0.2.4) ---------------------------

/// The pearl item id (`ModRegistry.java:97`).
pub const FROZEN_PEARL: &str = "torchmaster:frozen_pearl";
/// Upstream's `use` walks `-15..=15` on **each** axis around the player's own
/// block position (`FrozenPearlItem.java:35-44`): a 31×31×31 cube. It is not the
/// lantern's radius and it is not centred on a clicked block — read from the jar.
pub const PEARL_RADIUS: i32 = 15;
/// `frozenPearlDurability` (NeoForge default **1024**,
/// `TorchmasterNeoforgeConfig.java:122-125`): one charge per removed light, `0`
/// disables durability.
pub const PEARL_DURABILITY: u32 = 1024;

/// Whether a block id names the residual light the pearl removes. Upstream tests
/// the block object against `ModRegistry.blockInvisibleLight`; the port compares
/// the canonical block id the SDK gives it.
#[must_use]
pub fn is_residual_light(name: &str) -> bool {
    name == INVISIBLE_LIGHT
}

/// Whether `pos` is inside the pearl's cube around `center`.
#[must_use]
pub fn in_pearl_range(center: [i32; 3], pos: [i32; 3]) -> bool {
    (0..3).all(|axis| (pos[axis] - center[axis]).abs() <= PEARL_RADIUS)
}

/// The charges one use spends: one per light removed, capped by the charges the
/// stack has left. A `max` of zero means durability is disabled, which costs
/// nothing — exactly the config's `0` setting.
#[must_use]
pub fn pearl_charges(removed: usize, max: u32, damage: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    let left = max.saturating_sub(damage) as usize;
    removed.min(left) as u32
}

/// Which in-range lights a pearl removes and what the use costs.
///
/// Upstream walks the cube and returns early as soon as the stack empties, so a
/// worn pearl can only remove as many lights as it has charges left. The port
/// keeps that ordering: it removes the first `charges` of the in-range lights,
/// never touching a light outside the player's 31-block cube.
#[must_use]
pub fn pearl_removals(
    center: [i32; 3],
    lights: &[[i32; 3]],
    max: u32,
    damage: u32,
) -> (Vec<[i32; 3]>, u32) {
    let in_range: Vec<[i32; 3]> = lights
        .iter()
        .copied()
        .filter(|pos| in_pearl_range(center, *pos))
        .collect();
    let charges = pearl_charges(in_range.len(), max, damage);
    let take = if max == 0 {
        in_range.len()
    } else {
        charges as usize
    };
    (in_range.into_iter().take(take).collect(), charges)
}

// ---- the feral-lantern spawner interaction (0.2.5) ---------------------------------

/// Vanilla's light limit for a monster to spawn: `getMaxLocalRawBrightness` above
/// this (the dimension's `monster_spawn_light_level`, default 7 — the SDK exposes
/// both `DimensionSettings.monster_spawn_light_level` and the level-dimension
/// counterpart at this pin). A spawner at or below it runs; above it, it does not.
pub const SPAWNER_LIGHT_LIMIT: i32 = 7;

/// Whether the light at a spawner suppresses the spawn it just asked for.
///
/// The feral lantern's own block declares light 15 and writes `invisible_light`
/// blocks of light 15 into empty cells whose light is below 10 (all three numbers
/// are read from the artefact — `ModRegistry.java:87,102` and
/// `TorchmasterNeoforgeConfig.java:99-112`). Any of that light at the spawner
/// exceeds vanilla's limit, which is how the lantern keeps a spawner quiet.
#[must_use]
pub fn spawner_light_suppresses(block_light: i32) -> bool {
    block_light > SPAWNER_LIGHT_LIMIT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(dimension: &str, pos: [i32; 3], kind: LightKind) -> Light {
        Light {
            dimension: dimension.to_owned(),
            pos,
            kind,
        }
    }

    #[test]
    fn the_mega_torch_vetoes_only_the_monster_category() {
        assert!(LightKind::MegaTorch.blocks_category("monster"));
        for category in [
            "creature",
            "ambient",
            "axolotls",
            "underground_water_creature",
            "water_creature",
            "water_ambient",
            "misc",
        ] {
            assert!(
                !LightKind::MegaTorch.blocks_category(category),
                "{category}"
            );
        }
    }

    #[test]
    fn the_dread_lamp_vetoes_passive_categories_but_not_monsters_or_misc() {
        for category in [
            "creature",
            "ambient",
            "axolotls",
            "underground_water_creature",
            "water_creature",
            "water_ambient",
        ] {
            assert!(LightKind::DreadLamp.blocks_category(category), "{category}");
        }
        assert!(!LightKind::DreadLamp.blocks_category("monster"));
        assert!(!LightKind::DreadLamp.blocks_category("misc"));
    }

    #[test]
    fn only_the_two_carried_blocks_are_lights() {
        assert_eq!(
            LightKind::of_block("torchmaster:megatorch"),
            Some(LightKind::MegaTorch)
        );
        assert_eq!(
            LightKind::of_block("torchmaster:dreadlamp"),
            Some(LightKind::DreadLamp)
        );
        assert_eq!(LightKind::of_block("minecraft:torch"), None);
        assert_eq!(LightKind::of_block("torchmaster:feral_flare_lantern"), None);
        assert_eq!(LightKind::MegaTorch.block(), "torchmaster:megatorch");
        assert_eq!(LightKind::DreadLamp.block(), "torchmaster:dreadlamp");
    }

    #[test]
    fn the_cubic_range_matches_upstreams_entity_positions() {
        // The default radius is upstream's 64.
        assert_eq!(RADIUS, 64);
        let light = [0, 0, 0];
        // x/z are entity block centres: [-r, r] in whole blocks.
        assert!(in_range(light, [64, 0, 0], RADIUS));
        assert!(!in_range(light, [65, 0, 0], RADIUS));
        assert!(in_range(light, [-64, 0, 0], RADIUS));
        assert!(!in_range(light, [-65, 0, 0], RADIUS));
        assert!(in_range(light, [0, 0, 64], RADIUS));
        assert!(!in_range(light, [0, 0, 65], RADIUS));
        // y is the feet block's y, and upstream's +1 gives one more layer up.
        assert!(in_range(light, [0, 65, 0], RADIUS));
        assert!(!in_range(light, [0, 66, 0], RADIUS));
        assert!(in_range(light, [0, -64, 0], RADIUS));
        assert!(!in_range(light, [0, -65, 0], RADIUS));
        // Radius 0 is the torch's own cell (the y layer and the x/z cell).
        assert!(in_range(light, [0, 0, 0], 0));
        assert!(in_range(light, [0, 1, 0], 0));
        assert!(!in_range(light, [1, 0, 0], 0));
    }

    #[test]
    fn blocks_needs_the_dimension_the_category_and_the_range() {
        let lights = vec![light(
            "minecraft:overworld",
            [0, 64, 0],
            LightKind::MegaTorch,
        )];
        assert!(blocks(
            "minecraft:overworld",
            "monster",
            [10, 64, 10],
            &lights,
            RADIUS
        ));
        assert!(blocks(
            "minecraft:overworld",
            "monster",
            [0, 129, 0],
            &lights,
            RADIUS
        ));
        // A different dimension, a category the torch ignores, or a far cell.
        assert!(!blocks(
            "minecraft:the_nether",
            "monster",
            [10, 64, 10],
            &lights,
            RADIUS
        ));
        assert!(!blocks(
            "minecraft:overworld",
            "creature",
            [10, 64, 10],
            &lights,
            RADIUS
        ));
        assert!(!blocks(
            "minecraft:overworld",
            "monster",
            [65, 64, 0],
            &lights,
            RADIUS
        ));

        // The dread lamp vetoes creatures in its own dimension only.
        let lamps = vec![light(
            "minecraft:overworld",
            [0, 64, 0],
            LightKind::DreadLamp,
        )];
        assert!(blocks(
            "minecraft:overworld",
            "creature",
            [10, 64, 10],
            &lamps,
            RADIUS
        ));
        assert!(!blocks(
            "minecraft:overworld",
            "monster",
            [10, 64, 10],
            &lamps,
            RADIUS
        ));
        assert!(!blocks(
            "minecraft:the_end",
            "creature",
            [10, 64, 10],
            &lamps,
            RADIUS
        ));
    }

    #[test]
    fn upsert_keeps_one_light_per_position_and_remove_clears_it() {
        let mut lights = Vec::new();
        upsert(
            &mut lights,
            light("minecraft:overworld", [0, 64, 0], LightKind::MegaTorch),
        );
        assert_eq!(lights.len(), 1);
        // Replacing the block at the same position swaps the kind in place.
        upsert(
            &mut lights,
            light("minecraft:overworld", [0, 64, 0], LightKind::DreadLamp),
        );
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0].kind, LightKind::DreadLamp);
        // The same coordinates in another dimension are another light.
        upsert(
            &mut lights,
            light("minecraft:the_nether", [0, 64, 0], LightKind::MegaTorch),
        );
        assert_eq!(lights.len(), 2);
        remove(&mut lights, "minecraft:overworld", [0, 64, 0]);
        assert_eq!(
            lights,
            vec![light(
                "minecraft:the_nether",
                [0, 64, 0],
                LightKind::MegaTorch
            )]
        );
        // Removing an absent light is a no-op.
        remove(&mut lights, "minecraft:overworld", [0, 64, 0]);
        assert_eq!(lights.len(), 1);
    }

    #[test]
    fn lights_persist_as_one_tsv_line_each() {
        let lights = vec![
            light("minecraft:overworld", [12, -60, -4], LightKind::MegaTorch),
            light("minecraft:the_nether", [0, 64, 0], LightKind::DreadLamp),
        ];
        let text = format_lights(&lights);
        assert_eq!(
            text,
            "minecraft:overworld\t12\t-60\t-4\tmegatorch\n\
             minecraft:the_nether\t0\t64\t0\tdreadlamp\n"
        );
        assert_eq!(parse_lights(&text), lights);
    }

    #[test]
    fn the_readout_counts_the_lights_by_kind() {
        assert_eq!(count_line(&[]), "torchmaster: no blocking lights");
        assert_eq!(
            count_line(&[light(
                "minecraft:overworld",
                [0, 64, 0],
                LightKind::MegaTorch
            )]),
            "torchmaster: 1 blocking light: 1 megatorch"
        );
        let both = vec![
            light("minecraft:overworld", [0, 64, 0], LightKind::MegaTorch),
            light("minecraft:overworld", [8, 64, 0], LightKind::DreadLamp),
            light("minecraft:the_nether", [0, 64, 0], LightKind::MegaTorch),
        ];
        assert_eq!(
            count_line(&both),
            "torchmaster: 3 blocking lights: 2 megatorch, 1 dreadlamp"
        );
    }

    #[test]
    fn the_readout_lists_lights_in_registry_order_and_caps_the_list() {
        let lights = vec![
            light("minecraft:overworld", [2, 201, 0], LightKind::MegaTorch),
            light("minecraft:the_nether", [0, 64, 0], LightKind::DreadLamp),
        ];
        assert_eq!(
            light_lines(&lights, 8),
            vec![
                "torchmaster: minecraft:overworld 2 201 0 megatorch",
                "torchmaster: minecraft:the_nether 0 64 0 dreadlamp",
            ]
        );
        let many: Vec<Light> = (0..40)
            .map(|x| light("minecraft:overworld", [x, 64, 0], LightKind::MegaTorch))
            .collect();
        let capped = light_lines(&many, 2);
        assert_eq!(capped.len(), 3, "two lines plus the left-out line");
        assert_eq!(capped[2], "torchmaster: … 38 more");
    }

    #[test]
    fn the_world_file_word_and_the_kind_name_each_other() {
        for kind in [LightKind::MegaTorch, LightKind::DreadLamp] {
            assert_eq!(LightKind::of_name(kind.name()), Some(kind));
        }
        assert_eq!(LightKind::of_name("feral_flare_lantern"), None);
    }

    #[test]
    fn malformed_persistence_lines_are_ignored() {
        let text = "\
            minecraft:overworld\t1\t2\t3\tmegatorch\n\
            \n\
            bogus line\n\
            minecraft:overworld\tx\t2\t3\tmegatorch\n\
            minecraft:overworld\t1\t2\t3\tferal_flare_lantern\n\
            minecraft:overworld\t-7\t-8\t9\tdreadlamp\n";
        assert_eq!(
            parse_lights(text),
            vec![
                light("minecraft:overworld", [1, 2, 3], LightKind::MegaTorch),
                light("minecraft:overworld", [-7, -8, 9], LightKind::DreadLamp),
            ]
        );
    }

    // ---- the Feral Flare Lantern and its invisible light (0.2.3) --------------------

    #[test]
    fn the_flare_ids_and_upstream_defaults_are_what_the_artefact_declares() {
        assert_eq!(FERAL_FLARE_LANTERN, "torchmaster:feral_flare_lantern");
        assert_eq!(INVISIBLE_LIGHT, "torchmaster:invisible_light");
        assert!(is_flare_block(FERAL_FLARE_LANTERN));
        assert!(!is_flare_block(MEGA_TORCH));
        assert!(is_invisible_light(INVISIBLE_LIGHT));
        assert!(!is_invisible_light("minecraft:air"));
        // TorchmasterNeoforgeConfig.java:99-120 and ModRegistry.java:82-107.
        assert_eq!(FLARE_RADIUS, 16);
        assert_eq!(FLARE_TICK_RATE, 5);
        assert_eq!(FLARE_MIN_LIGHT, 10);
        assert_eq!(FLARE_HARD_CAP, 255);
        assert_eq!(FLARE_LIGHT_LEVEL, 15);
    }

    #[test]
    fn the_first_light_goes_into_the_nearest_empty_dark_cell() {
        let mut flare = Flare::new("minecraft:overworld".to_owned(), [0, 201, 0]);
        // The lantern's own light is 15, so cells closer than 6 are too bright; the
        // box's floor is at y=200. These are the rules lib.rs checks in game.
        let dark_and_empty = |pos: [i32; 3]| {
            let distance = pos[0].abs().max((pos[1] - 201).abs()).max(pos[2].abs());
            pos[1] == 201 && pos[0].abs() <= 8 && pos[2].abs() <= 8 && distance >= 6
        };
        assert_eq!(
            next_candidate(&mut flare, dark_and_empty),
            Some([-6, 201, 0])
        );
        // The cursor moves on: the next attempt must not pick the same cell twice.
        assert_ne!(
            next_candidate(&mut flare, dark_and_empty),
            Some([-6, 201, 0])
        );
    }

    #[test]
    fn no_candidate_is_returned_when_the_whole_radius_is_unusable() {
        let mut flare = Flare::new("minecraft:overworld".to_owned(), [0, 201, 0]);
        assert_eq!(next_candidate(&mut flare, |_| false), None);
    }

    #[test]
    fn a_flare_only_places_on_its_cadence_and_stops_at_the_hard_cap() {
        assert!(!should_place(0, 0, FLARE_TICK_RATE, FLARE_HARD_CAP));
        assert!(!should_place(4, 0, FLARE_TICK_RATE, FLARE_HARD_CAP));
        assert!(should_place(5, 0, FLARE_TICK_RATE, FLARE_HARD_CAP));
        assert!(!should_place(
            5,
            FLARE_HARD_CAP,
            FLARE_TICK_RATE,
            FLARE_HARD_CAP
        ));
        // Upstream tests `size() > hardcap`, which allows 256 with a cap of 255.
        // The port fixes that off-by-one: the cap is the last allowed light.
        assert!(should_place(
            5,
            FLARE_HARD_CAP - 1,
            FLARE_TICK_RATE,
            FLARE_HARD_CAP
        ));
        assert!(!should_place(10, FLARE_HARD_CAP, 5, 0));
    }

    #[test]
    fn the_flare_registry_keeps_one_lantern_per_position_and_returns_its_children() {
        let mut flares = Vec::new();
        upsert_flare(&mut flares, "minecraft:overworld", [0, 201, 0]);
        upsert_flare(&mut flares, "minecraft:overworld", [0, 201, 0]);
        assert_eq!(flares.len(), 1);
        upsert_flare(&mut flares, "minecraft:the_end", [0, 201, 0]);
        assert_eq!(flares.len(), 2);
        let mut flare = take_flare(&mut flares, "minecraft:overworld", [0, 201, 0])
            .expect("the placed lantern is in the registry");
        flare.children.push([-6, 201, 0]);
        assert_eq!(flare.children, vec![[-6, 201, 0]]);
        assert_eq!(flares.len(), 1);
        assert!(take_flare(&mut flares, "minecraft:overworld", [0, 201, 0]).is_none());
    }

    // ---- the Frozen Pearl and residual-light cleanup (0.2.4) ------------------------

    #[test]
    fn the_pearl_rules_are_what_the_artefact_declares() {
        assert_eq!(FROZEN_PEARL, "torchmaster:frozen_pearl");
        assert_eq!(PEARL_RADIUS, 15);
        assert_eq!(PEARL_DURABILITY, 1024);
        assert!(is_residual_light(INVISIBLE_LIGHT));
        assert!(!is_residual_light(MEGA_TORCH));
        assert!(!is_residual_light("minecraft:torch"));
    }

    #[test]
    fn the_pearl_cube_is_a_31_block_cube_centred_on_the_player() {
        let c = [0, 201, 0];
        assert!(in_pearl_range(c, [15, 201, 15]));
        assert!(in_pearl_range(c, [-15, 186, -15]));
        assert!(!in_pearl_range(c, [16, 201, 0]));
        assert!(!in_pearl_range(c, [0, 201, -16]));
        assert!(!in_pearl_range(c, [0, 217, 0]));
        // It is the player's own block position that centres the cube, not a clicked block.
        assert!(in_pearl_range([2, 201, 3], [17, 201, 3]));
        assert!(!in_pearl_range([2, 201, 3], [18, 201, 3]));
    }

    #[test]
    fn each_removed_light_costs_one_charge_and_zero_disables_durability() {
        assert_eq!(pearl_charges(3, PEARL_DURABILITY, 0), 3);
        assert_eq!(pearl_charges(0, PEARL_DURABILITY, 0), 0);
        assert_eq!(pearl_charges(3, 0, 0), 0);
    }

    #[test]
    fn a_worn_pearl_stops_when_it_runs_out_of_charges() {
        let center = [0, 201, 0];
        let lights = vec![[1, 201, 0], [2, 201, 0], [3, 201, 0], [40, 201, 0]];
        // 40 is outside the 31-block cube and is always left alone.
        let (removed, charges) = pearl_removals(center, &lights, PEARL_DURABILITY, 0);
        assert_eq!(removed, vec![[1, 201, 0], [2, 201, 0], [3, 201, 0]]);
        assert_eq!(charges, 3);
        // One charge left: only one light is removed before the pearl empties.
        let (removed, charges) = pearl_removals(center, &lights, 4, 3);
        assert_eq!(removed, vec![[1, 201, 0]]);
        assert_eq!(charges, 1);
        // With durability disabled every in-range light goes, at no charge.
        let (removed, charges) = pearl_removals(center, &lights, 0, 0);
        assert_eq!(removed.len(), 3);
        assert_eq!(charges, 0);
    }

    // ---- the feral-lantern spawner interaction (0.2.5) ------------------------------

    #[test]
    fn a_spawner_is_suppressed_only_above_vanillas_light_limit() {
        assert_eq!(SPAWNER_LIGHT_LIMIT, 7);
        // A sealed dark box at light 0 spawns; the exact limit is still allowed.
        assert!(!spawner_light_suppresses(0));
        assert!(!spawner_light_suppresses(SPAWNER_LIGHT_LIMIT));
        // One above the limit and both of the lantern's light values suppress it.
        assert!(spawner_light_suppresses(SPAWNER_LIGHT_LIMIT + 1));
        assert!(spawner_light_suppresses(i32::from(FLARE_LIGHT_LEVEL)));
        assert!(spawner_light_suppresses(FLARE_MIN_LIGHT));
    }
}
