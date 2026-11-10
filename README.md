# torchmaster (Horizon port)

A Horizon port of [torchmaster](Xalcon/TorchMaster) — MIT.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- Analysis clone: `https://github.com/Xalcon/TorchMaster` at
- `a11226c866bf4e9f0695621db5971418dffe3014` (`2026-10-14`, version
- `21.1.14`), read from `common/`, `neoforge/` and the carried assets; the clone
- was deleted after the notes (as required by AGENTS.md).
- Mega Torch** (`torchmaster:megatorch`) vetoes natural spawns of every
- non-friendly** entity category (vanilla: `monster`) in a cube of
- `megaTorchRadius` (default **64**) around it. Source:
- `logic/entityblocking/megatorch/MegatorchEntityBlockingLight.java:27-33`,

## Not yet ported

- Spawners are deliberately not blocked.** Upstream's
- `blockOnlyNaturalSpawns` defaults to true; the pin's `potential_spawns` only
- fires for the natural path. The `false` mode (block spawners too) has no
- hook at the pin and is not carried.
- Phantoms and village sieges.** Upstream also vetoes phantom spawns
- (`PlayerSpawnPhantomsEvent`) and village sieges (`VillageSiegeEvent`); the
- latter now has a pin hook (`village_siege`, API 389), but phantoms do not go
- through `potential_spawns`. Both are slice 2.

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: Xalcon/TorchMaster. Port: AdLf0208. Licence: MIT — see [LICENSE](LICENSE);
the original notices are preserved.
