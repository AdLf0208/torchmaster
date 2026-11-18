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

pub const MEGA_TORCH: &str = "torchmaster:megatorch";
pub const DREAD_LAMP: &str = "torchmaster:dreadlamp";

/// Upstream's `megaTorchRadius` / `dreadLampRadius` default
/// (`TorchmasterNeoforgeConfig.java:67-76`).
pub const RADIUS: i32 = 64;

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
            let kind = match fields.next()? {
                "megatorch" => LightKind::MegaTorch,
                "dreadlamp" => LightKind::DreadLamp,
                _ => return None,
            };
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
        let kind = match light.kind {
            LightKind::MegaTorch => "megatorch",
            LightKind::DreadLamp => "dreadlamp",
        };
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            light.dimension, light.pos[0], light.pos[1], light.pos[2], kind
        ));
    }
    text
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
}
