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
