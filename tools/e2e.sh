#!/usr/bin/env bash
# In-game check for the Torchmaster slice 1 (mega torch, API 388
# `potential_spawns`), campaign pin 1d63e0a5, Minecraft 26.3.
#
# The arena is a sealed stone box at the Overworld's bottom
# (-32..32 x, -32..32 z, floor y=200, roof y=208), where the world-surface
# heightmap is the roof: a natural-spawn attempt's random y lands inside the
# room about seven times in ten. The player stands in the box and the mob cap
# keeps only the box's own spawns.
#
# Phases:
#   1. control (no torch): natural spawns appear in the box;
#   2. the mega torch is placed ABOVE the roof (y=210, its light sealed by the roof, so
#      its light cannot reach the room): 0 monster spawns over the same wait.
#      This isolates the hook from the block's light level;
#   3. the player moves to a second box 160 blocks away, still with the torch:
#      spawns appear there, proving the veto is radius-bounded, not global;
#   4. the torch is replaced with air: spawns return in the first box.
#
# The API 388 hook was probed independently (before the port) by the probe
# build in `ports/_shared/evidence/torchmaster/probe/`, which cancels every
# candidate with no block involved.
#
# NEGATIVE=1 keeps the scenario but expects phase 2 to spawn monsters; the run
# MUST fail, proving the zero is not vacuous.
#
# Run through the registry:
#   ports/tools/build-slot.sh e2e --job torch-1d63e0a --port ports/torchmaster \
#       -- tools/e2e.sh
set -euo pipefail

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/torchmaster
CTL=${CTL:-$CLONE/ports/_shared/bin/horizon-1d63e0a5/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/torchmaster-e2e.sock}
LOG=${LOG:-/tmp/opencode/torchmaster-e2e.log}
CONTENT=${CONTENT:-$PORT/content}
NEGATIVE=${NEGATIVE:-0}
# EMBEDDED=1 launches the native bundle alone (no --dev-mod).
EMBEDDED=${EMBEDDED:-0}
DEV_ARGS=()
[ "$EMBEDDED" = 1 ] || DEV_ARGS=(--dev-mod "$CONTENT")

mkdir -p "$(dirname "$LOG")"
exec > >(tee "$LOG") 2>&1

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
[ -f "$CONTENT/horizon-mod.toml" ] || { echo "missing content mod: $CONTENT" >&2; exit 1; }
command -v jq >/dev/null || { echo "missing jq" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/torchmaster-home.XXXXXX)
GAMELOG=$SCRATCH/game/logs/latest.log
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
# `ctl entities` reports mobs as kind "mob"; bats are the only ambient mob a
# sealed dark stone box can grow, and the mega torch deliberately leaves them.
mobs() { ctl entities 48 | jq '[.entities[] | select(.kind == "mob" and .type != "minecraft:bat")] | length'; }
item_count() { ctl inventory | jq -r --arg i "$1" '.counts[$i] // 0'; }
block_at() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }

kill_mobs() {
    ctl command "execute in minecraft:overworld run kill @e[type=!player]" >/dev/null || true
    ctl command "kill @e[type=item]" >/dev/null || true
    ctl wait 2 >/dev/null
}

# Wait up to `max` ticks for at least one monster; prints the tick count.
wait_for_mobs() { # wait_for_mobs <max-ticks>
    local waited=0 count
    while [ "$waited" -lt "$1" ]; do
        ctl wait 20 >/dev/null
        waited=$((waited + 20))
        count=$(mobs)
        if [ "$count" -gt 0 ]; then
            echo "$waited"
            return 0
        fi
    done
    echo "$waited"
    return 1
}

# A sealed room: floor at `y`, roof at `y+8`, walls to the roof.
build_room() { # build_room <cx> <cz> <y>
    local cx=$1 cz=$2 y=$3
    local top=$((y + 8))
    ctl command "fill $((cx - 32)) $y $((cz - 32)) $((cx + 32)) $y $((cz + 32)) minecraft:stone"
    ctl command "fill $((cx - 32)) $top $((cz - 32)) $((cx + 32)) $top $((cz + 32)) minecraft:stone"
    ctl command "fill $((cx - 33)) $y $((cz - 33)) $((cx - 33)) $top $((cz + 33)) minecraft:stone"
    ctl command "fill $((cx + 33)) $y $((cz - 33)) $((cx + 33)) $top $((cz + 33)) minecraft:stone"
    ctl command "fill $((cx - 33)) $y $((cz - 33)) $((cx + 33)) $top $((cz - 33)) minecraft:stone"
    ctl command "fill $((cx - 33)) $y $((cz + 33)) $((cx + 33)) $top $((cz + 33)) minecraft:stone"
    ctl command "fill $((cx - 32)) $((y + 1)) $((cz - 32)) $((cx + 32)) $((top - 1)) $((cz + 32)) minecraft:air"
}

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 2
    rm -rf "$SCRATCH" 2>/dev/null || true
    echo "EXIT:$status"
}
trap cleanup EXIT

echo "== launch (world TorchmasterE2E, content --dev-mod content/) =="
cd "$SCRATCH"
"$CTL" --socket "$SOCK" launch \
    --game "$GAME" --world TorchmasterE2E --seed 20261108 --creative --headless --wait \
    --resources "$RES" "${DEV_ARGS[@]}"
ctl wait-ready 120
if [ "$EMBEDDED" = 1 ]; then
    cache=$SCRATCH/.horizon-cache/torchmaster-embedded-content.hmod
    [ -f "$cache" ] || fail "the bundle did not write its embedded content to $cache"
    cmp -s "$cache" "$PORT/content/torchmaster.hmod" \
        || fail "the embedded content differs from content/torchmaster.hmod"
    echo "  embedded content: $(wc -c <"$cache" | tr -d ' ') B, identical to content/torchmaster.hmod"
fi
ctl command "gamerule spawn_mobs true"
ctl command "gamerule spawn_monsters true"
ctl command "gamerule advance_time false"
ctl command "gamerule random_tick_speed 0"
ctl command "difficulty hard"
ctl command "time set noon"
ctl command "weather clear"

echo "== content: both blocks exist, are giveable, place and break =="
ctl give torchmaster:megatorch 1 >/dev/null || fail "give megatorch failed"
ctl give torchmaster:dreadlamp 1 >/dev/null || fail "give dreadlamp failed"
ctl wait 2 >/dev/null
[ "$(item_count torchmaster:megatorch)" -ge 1 ] || fail "megatorch not in inventory"
[ "$(item_count torchmaster:dreadlamp)" -ge 1 ] || fail "dreadlamp not in inventory"

# Build the arena first, then a small loot check on the floor: a placed torch
# registers, a broken one drops itself and unregisters (the spawn phases below
# prove the registry effect).
echo "== arena A (sealed sky box at y=200..208) =="
build_room 0 0 200
ctl command "tp @s 0.5 201 0.5" >/dev/null
ctl wait 20 >/dev/null
[ "$(block_at 0 200 0)" = "minecraft:stone" ] || fail "arena floor was not built"
ctl command "setblock 2 201 0 torchmaster:megatorch"
ctl wait 2 >/dev/null
[ "$(block_at 2 201 0)" = "torchmaster:megatorch" ] || fail "setblock megatorch did not take"
ctl break 2 201 0 --timeout 20 >/dev/null || fail "break megatorch failed"
ctl wait 2 >/dev/null
[ "$(block_at 2 201 0)" = "minecraft:air" ] || fail "megatorch did not break"
# The world is creative, so a break drops nothing; the carried loot table is not
# exercised here (content give/place is, and the block-change path is).
ctl command "kill @e[type=item]" >/dev/null || true
ctl wait 2 >/dev/null

echo "== phase 1: control (no torch) =="
kill_mobs
ctl wait 300 >/dev/null
control_count=$(mobs)
echo "control: $control_count monsters after 300 ticks"
[ "$control_count" -gt 0 ] || fail "control saw no monsters"

echo "== phase 2: mega torch above the roof (hook-only, light sealed out) =="
ctl command "setblock 0 210 0 torchmaster:megatorch"
ctl wait 2 >/dev/null
[ "$(block_at 0 210 0)" = "torchmaster:megatorch" ] || fail "the torch above the roof was not placed"
grep -q "registered torchmaster:megatorch" "$GAMELOG" || fail "the torch was not registered"
echo "registry: $(grep -a "registered torchmaster:megatorch" "$GAMELOG" | tail -1)"
light=$(ctl light 0 201 0 0 201 0 | jq -r '.block')
[ "$light" = "0" ] || fail "the torch's light reached the room (block light $light)"
kill_mobs
ctl wait 300 >/dev/null
torch_count=$(mobs)
echo "with the torch: $torch_count monsters after 300 ticks (control $control_count)"
if [ "$NEGATIVE" = 1 ]; then
    # A scenario that cannot fail is not evidence: this run must exit 1.
    [ "$torch_count" -gt 0 ] || fail "NEGATIVE=1 expected a spawn with the torch, saw none"
    echo "NEGATIVE CONTROL FAILED AS REQUIRED: $torch_count monsters in the vetoed box"
    exit 1
fi
[ "$torch_count" -eq 0 ] || fail "the mega torch let $torch_count monsters spawn in range"

echo "== phase 3: a second box 160 blocks away still spawns =="
build_room 160 0 200
ctl command "tp @s 160.5 201 0.5" >/dev/null
ctl wait 40 >/dev/null
kill_mobs
ctl wait 300 >/dev/null
outside_count=$(mobs)
echo "outside the radius: $outside_count monsters after 300 ticks"
[ "$outside_count" -gt 0 ] || fail "outside box saw no monsters"

echo "== phase 4: breaking the torch returns spawns to box A =="
ctl command "tp @s 0.5 201 0.5" >/dev/null
ctl wait 40 >/dev/null
ctl command "setblock 0 210 0 minecraft:air"
ctl wait 2 >/dev/null
[ "$(block_at 0 210 0)" = "minecraft:air" ] || fail "the torch was not removed"
grep -q "unregistered torchmaster:megatorch" "$GAMELOG" || fail "the torch was not unregistered"
echo "registry: $(grep -a "unregistered torchmaster:megatorch" "$GAMELOG" | tail -1)"
kill_mobs
ctl wait 300 >/dev/null
returned_count=$(mobs)
echo "after the break: $returned_count monsters after 300 ticks"
[ "$returned_count" -gt 0 ] || fail "spawns did not return after the torch broke"

echo "PASS: control=$control_count, torch=$torch_count, outside=$outside_count, after_break=$returned_count"
# The cleanup trap prints EXIT:<status> as the last line.
