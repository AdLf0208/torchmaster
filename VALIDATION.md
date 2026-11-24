# Validation

The measured record is in [`notes.md`](notes.md); the logs of every run that
proved the port are in `ports/_shared/evidence/torchmaster/`, and the retired
`.hnmod` bundle they were taken on is archived in
`ports/_shared/evidence/torchmaster/retired-hnmod/`.

## The SDK package (2026-11-18, pin `0b14f8a`)

| Check | Command | Result |
|---|---|---|
| Runtime unit tests | `cargo test` | **8 passed**, 0 failed |
| Formatting | `cargo fmt --check` | clean |
| Lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings |
| Package build | `cargo horizon-mod build` | `target/horizon-mod/torchmaster.hmod` |
| Registry check | `cargo horizon-mod check torchmaster.hmod` | exit 0 — `native-mod`, `native-1/0b14f8ac…/e59da909…`, for 26.3 |
| Delivered package | `torchmaster.hmod` | **24,285 B**, sha256 `f97781db…a28f4`, **28** entries, `LICENSE` + `LICENSE-NOTICE.md` inside |

`cargo horizon-mod check` does not compile the mod and does not run its hooks;
the compile gate is `cargo test`/`clippy` on the crate, and no in-game run was
possible at this pin (no launcher-built game on this box). Both are stated in
`notes.md` § "Migration to the SDK format".

## 0.2.2 — R9: SDK-only dependencies, no lock, and the readout (2026-11-23, pin `0a875da`)

| Check | Command | Result |
|---|---|---|
| the rule, before the change | `verify-sdk-package.py torchmaster` on 0.2.1 | **0/1** — `FAIL SDK-only dependencies (R9): log`, `FAIL no packed Cargo.lock` (`../_shared/evidence/torchmaster/r9/gate-before-0.2.1.log`) |
| TDD | tests first, then `count_line`/`light_lines`/`LightKind::name` | compile-red first (`r9/unit-tdd-red.log`), then **11 passed** |
| tests | `cargo test --offline` | **11 passed**, 0 failed (`r9/test.log`, `sdk-test.log`) |
| format and lints | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | clean, 0 warnings |
| package | `rm -f Cargo.lock && cargo horizon-mod build` | 0.2.2 `.hmod`, no `Cargo.lock` inside (`r9/build.log`) |
| registry check | `cargo horizon-mod check torchmaster.hmod` | `torchmaster 0.2.2 (native-mod, native-1/0a875da…/e59da90…, for 26.3)` (`r9/check.log`) |
| fits the pin | `cargo horizon-mod fits torchmaster.hmod --build 0a875da` | `0a875da  made for`, exit 0 |
| fits the previous build | `cargo horizon-mod fits torchmaster.hmod --build 0b14f8a` | `fits`, exit 0 |
| launcher, lock-less | `launcher-game.sh build torchmaster` | `native-0a875da-… built`, `EXIT:0` (`r9/native-build-no-lock.log`) |
| gate | `verify-sdk-package.py torchmaster` | **1/1** (`r9/gate-after.log`) |
| spawn proof | `tools/e2e.sh` | `PASS` + `EXIT:0` — torch 0 vs control 3, outside 4, after break 4; lamp 0 bats vs control 1, 70 monsters still spawning (`../_shared/evidence/torchmaster-e2e/e2e.log`) |
| counterfactual | `SELFTEST=1 tools/e2e.sh` | `FAIL` + `EXIT:1` (`e2e-selftest.log`) |
| differential | `CONTROL=1 … tools/e2e.sh` on `CONTROL_GAME` | `FAIL` + `EXIT:1` — ids and command unknown (`e2e-control.log`) |
| carried recipes + loot | `tools/e2e-dialect.sh` (two specs: megatorch, dreadlamp) | `PASS` + `EXIT:0`; `SELFTEST=1` `FAIL` + `EXIT:1` (`../_shared/evidence/torchmaster/e2e-dialect*.log`) |
| log gate | `check-e2e-logs.py torchmaster-e2e --strict` | `logs=3 ok=3`, exit 0 |

Changed: `log` removed (no logging surface at the pin; `/torchmaster` and `/torchmaster list` carry
the information), `Cargo.lock` deleted, `Cargo.toml` and `horizon-mod.toml` at 0.2.2. Full record in
[`notes.md`](notes.md) §0.2.2.

## 0.2.3 — the Feral Flare Lantern and its invisible light (2026-11-24, pin `0a875da`)

| Check | Command | Result |
|---|---|---|
| artefact rules | upstream 21.1.14 clone, `ModRegistry.java` / `…BlockEntity.java` / `TorchmasterNeoforgeConfig.java` | id `torchmaster:feral_flare_lantern`, light `torchmaster:invisible_light`, light level **15**, radius **16**, cadence **5** ticks, min light **10**, cap **255**, lifetime until the lantern breaks |
| TDD | tests first, then the pure flare engine | 42-error compile red (`../_shared/evidence/torchmaster/flare/unit-tdd-red.log`) → **16 passed** (`unit-tdd-green.log`) |
| tests | `cargo test --offline` | **16 passed**, 0 failed (`flare/unit-tdd-green.log`, regenerated `sdk-test.log`) |
| fmt + lints | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | clean, 0 warnings (`flare/lint.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` | 0.2.3 `.hmod`, no `Cargo.lock` inside |
| registry check | `cargo horizon-mod check torchmaster.hmod` | `torchmaster 0.2.3 (native-mod, native-1/0a875da…/e59da90…, for 26.3)` |
| fits | `cargo horizon-mod fits torchmaster.hmod --build 0a875da` / `--build 0b14f8a` | `0a875da  made for` / `fits`, both exit 0 |
| gate | `python3 ports/tools/verify-sdk-package.py torchmaster` | **1/1**, including `package matches the tree` |
| delivered | `torchmaster.hmod` | **34,758 B**, SHA-256 `51042af1bcf1d087f6c0b11b048502df53ea10a3ea70663e9c5941c17d3ce6d8`, 40 entries |
| launcher | `launcher-game.sh build torchmaster` | `Compiling torchmaster (1 of 1)`, `native-0a875da-fa100b01b09e`, `EXIT:0` (`../_shared/evidence/torchmaster/flare/native-build.log`) |
| flare proof | `tools/e2e.sh` phase E + E2 | `PASS` + `EXIT:0`: 60 ticks → **7** invisible lights, lantern block light **15**, breaking → **0**, stone differential **0** (`../_shared/evidence/torchmaster-e2e/e2e.log`) |
| counterfactual | `SELFTEST=flare tools/e2e.sh` | `FAIL` + `EXIT:1` (`e2e-selftest-flare.log`) |
| carried recipe + loot | `tools/e2e-dialect.sh` (three specs: megatorch, dreadlamp, flare) | `PASS` + `EXIT:0`; `SELFTEST=1` `FAIL` + `EXIT:1` |
| log gate | `check-e2e-logs.py torchmaster-e2e --strict` | all logs end `EXIT:<n>`, exit 0 |

Full record in [`notes.md`](notes.md) §0.2.3.

## The retired `.hnmod` bundle (pin `1d63e0a`, kept for the record)

| Check | Result |
|---|---|
| `cargo fmt --all --check` / `cargo clippy -- -D warnings` | clean, 0 warnings |
| `cargo test --locked --offline` (standalone) | 8 passed |
| `stage → check → test → build` via the old `tools/horizon.py` | all `cargo_exit: 0` |
| `tools/e2e.sh` | `PASS: control=30, torch=0, outside=13, after_break=51`, `EXIT:0` |
| `NEGATIVE=1 tools/e2e.sh` | exits 1 (`expected a spawn with the torch, saw none`) |
| content `.hmod` | 6,488 B, SHA-256 `6d192414…cb9d5` |
| native `.hnmod` | 10,036 B, SHA-256 `1c21a3a9…e68c` |

## 26.3 dialect sweep (2026-11-21) — 0.2.1

| Check | Command | Result |
|---|---|---|
| static dialect | `python3 ports/tools/check_261_dialect.py torchmaster.hmod` | exit 0; pre-fix package exit 1 (`../_shared/evidence/torchmaster/dialect-check-red.log`) |
| tests | `cargo test --offline` | exit 0 (`dialect-test.log`) |
| package | `cargo horizon-mod build` twice after `rm -f Cargo.lock` | `cmp`-identical, built against 0a875da |
| registry check | `cargo horizon-mod check torchmaster.hmod` | **22,418 B**, SHA-256 `0e5f301e8cd292e0e20402b6e99916207abe37473ce7a58fb625736fb2eb4066`, exit 0 |
| fits the pin | `cargo horizon-mod fits torchmaster.hmod --build 0a875da` | `0a875da  made for`, exit 0 |
| the gate | `python3 ports/tools/verify-sdk-package.py torchmaster` | 1/1 |
| in game | `tools/e2e-dialect.sh` | `PASS` + `EXIT:0`; `SELFTEST=1` `FAIL` + `EXIT:1`; `check-e2e-logs.py torchmaster --strict` exit 0 |

Changed: 2 recipes + 2 loot tables. Proof: megatorch crafts 1 and drops 1; the pre-fix 0.2.0 game leg is red on both (`e2e-dialect-red.log`). Full record in [`notes.md`](notes.md) §"26.3 dialect sweep".
