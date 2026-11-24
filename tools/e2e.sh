#!/usr/bin/env bash
# In-game proof for Torchmaster 0.2.2 (pin 0a875da, Minecraft 26.3): the Mega Torch
# and the Dread Lamp veto natural spawns, a light out of range vetoes nothing, a broken
# light drops its item, and `/torchmaster` is the readout that replaced the five `log`
# calls R9 refused.
#
# A native `.hmod` cannot load on an unmodded game, so GAME must be a game the launcher
# built with this port:
#
#   eval "$(ports/tools/launcher-game.sh env torchmaster)"
#   ports/tools/launcher-game.sh run torchmaster            # build + leased e2e
#
# Three legs, all required (each is a separate log, all ending in `EXIT:<n>`):
#
#   * default  — the proof below; `PASS` + `EXIT:0`.
#   * CONTROL=1 — the *same* launch on `CONTROL_GAME`, the same Nightly without the
#     package: the block ids are refused and `/torchmaster` is unknown, so the run must
#     fail. That is the differential that makes the green leg mean something.
#   * SELFTEST=1 — the mega-torch phase's assertion inverted: the run must see monsters
#     in the vetoed box and exit 1. That is the counterfactual.
#   * SELFTEST=flare — the same run, but only the flare-light assertion is inverted
#     (the earlier phases stay green); it must see no light and exit 1.
#   * SELFTEST=pearl — only the pearl's removal assertion is inverted; it must see
#     the residual lights still there and exit 1.
#   * SELFTEST=spawner — only the spawner assertion is inverted: a lit lantern must
#     quiet the spawner, so seeing the same count as the dark spawner must exit 1.
#   * SELFTEST=phantom — only the phantom assertion is inverted: a Mega Torch in
#     range must stop phantoms near an insomniac player, so seeing any must exit 1.
#
#   E. the Feral Flare Lantern writes `torchmaster:invisible_light` blocks in the sealed
#      dark box and emits light; breaking it takes the count back to zero. A plain stone
#      block in a second sealed box writes none (differential).
#   F. a Frozen Pearl right-click removes residual `invisible_light` in the player's
#      31-block cube and spends a charge per light; used on a vanilla-lit cell with no
#      residual light in range it changes no count and loses no charge (differential).
#   G. a zombie spawner in a sealed dark box spawns mobs; a lit feral lantern beside it
#      raises the block light at the spawner above vanilla's limit and quietens it.
#      The assertion reads the *type-filtered* `ctl entities 16 minecraft:zombie` count.
#   H. an insomniac player (a seeded `time_since_rest` stat) under an open night sky
#      grows phantoms; a Mega Torch within 64 blocks stops them through the stamped
#      `player_spawn_phantoms` veto. The assertion parses the type-filtered
#      `ctl entities 48 minecraft:phantom` count, never an absence of error.
#
# The arenas are sealed stone boxes in the Overworld sky (floor y=200, roof y=208), where
# the world surface is far below, so the only natural-spawn attempts near the player are
# the box's own:
#
#   A. dark box at x=0: control monsters appear; with the Mega Torch placed *above the
#      roof* (its light is sealed out, the room's block light reads 0) none do; a second
#      box 160 blocks away still spawns with the same torch placed (out of range vetoes
#      nothing); breaking the torch brings the spawns back.
#   B. the same dark box A: control bats (the ambient family a sealed dark box can grow)
#      appear; with the Dread Lamp at the box centre none do, while monsters keep
#      spawning in the same box (the lamp vetoes friendly categories only); standing in
#      box B 160 blocks away bats appear again with the lamp still placed.
#   C. `/torchmaster` and `/torchmaster list` parse to the fresh, placed and broken
#      registry states, and a bogus subcommand answers with no readout at all.
#   D. survival breaks: the Mega Torch and the Dread Lamp each drop themselves, with a
#      vanilla crafting table broken the same way as the control.
#
# Never `/tmp`: E2E_TMP is the launcher wrapper's `~/.cache/horizon-e2e/scratch`.
set -Eeuo pipefail
export LC_ALL=C

CLONE=${CLONE:-/home/adlf/Desktop/all_the_mods_10_rust_rewrite}
PORT=$(cd "$(dirname "$0")/.." && pwd)
MOD=$(basename "$PORT")
CTL=${CTL:-}
GAME=${GAME:-}
CONTROL_GAME=${CONTROL_GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
E2E_TMP=${E2E_TMP:-$HOME/.cache/horizon-e2e/scratch}
SOCK=${SOCK:-$E2E_TMP/${MOD}-e2e.sock}
SELFTEST=${SELFTEST:-0}
CONTROL=${CONTROL:-0}
KEEP=${KEEP:-0}

EVIDENCE=$CLONE/ports/_shared/evidence/${MOD}-e2e
LEG=e2e
[ "$SELFTEST" = 1 ] && LEG=e2e-selftest
[ "$SELFTEST" != 0 ] && [ "$SELFTEST" != 1 ] && LEG=e2e-selftest-$SELFTEST
[ "$CONTROL" = 1 ] && LEG=e2e-control
LOG=${LOG:-$EVIDENCE/$LEG.log}

MEGA=torchmaster:megatorch
LAMP=torchmaster:dreadlamp
FLARE=torchmaster:feral_flare_lantern
LIGHT=torchmaster:invisible_light
PEARL=torchmaster:frozen_pearl
# The friendly families (passive `creature` and `ambient`); the Dread Lamp vetoes every
# one of them, so a monster is any mob whose type is not in this list.
FRIENDLY='["minecraft:pig","minecraft:cow","minecraft:sheep","minecraft:chicken","minecraft:rabbit","minecraft:horse","minecraft:donkey","minecraft:mule","minecraft:llama","minecraft:fox","minecraft:wolf","minecraft:bee","minecraft:parrot","minecraft:bat","minecraft:squid","minecraft:glow_squid","minecraft:axolotl","minecraft:turtle","minecraft:frog","minecraft:armadillo","minecraft:goat","minecraft:camel","minecraft:sniffer","minecraft:cat","minecraft:ocelot","minecraft:panda","minecraft:polar_bear","minecraft:villager","minecraft:mooshroom","minecraft:allay","minecraft:tadpole","minecraft:dolphin","minecraft:strider"]'

command -v jq >/dev/null || { echo "missing jq" >&2; exit 1; }
if [ "$CONTROL" = 1 ]; then
    # The same Nightly without the package: the differential leg.
    [ -n "$CONTROL_GAME" ] || {
        echo "CONTROL=1 needs CONTROL_GAME (the unmodded Nightly export)" >&2
        exit 1
    }
    GAME=$CONTROL_GAME
fi
[ -n "$GAME" ] || {
    echo "GAME is required: the SDK package is built into a game by the launcher." >&2
    echo "  ports/tools/launcher-game.sh build $MOD  then  eval \"\$(ports/tools/launcher-game.sh env $MOD)\"" >&2
    exit 1
}
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
[ -f "$PORT/$MOD.hmod" ] || { echo "missing package: $PORT/$MOD.hmod" >&2; exit 1; }

mkdir -p "$E2E_TMP" "$EVIDENCE" "$(dirname "$SOCK")"
exec > >(tee "$LOG") 2>&1

SCRATCH=$(mktemp -d "$E2E_TMP/${MOD}-home.XXXXXX")
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$XDG_CONFIG_HOME"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
say() { echo "== $* =="; }

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    [ "$KEEP" = 1 ] || rm -rf "$SCRATCH" 2>/dev/null || true
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # world
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch --game "$GAME" --world "$1" --seed 20261123 \
        --creative --offscreen --wait --resources "$RES" >/dev/null
    ctl wait-ready 180
    [ "$(ctl state | jq -r '.screen')" = "playing" ] || fail "the game is not playing after launch"
    ctl wait 10
}

# ---- the readout the log calls became ------------------------------------------------
# `ctl messages` answers newest first, so the command's own reply is the newest run of
# `torchmaster:` lines. Anything else at the top means the command answered nothing.
readout() { # "/torchmaster" | "/torchmaster list"
    ctl chat "$1" >/dev/null
    ctl wait 2 >/dev/null
    ctl messages 10 | jq -r '.chat[]' 2>/dev/null \
        | awk 'NR == 1 && !/^torchmaster: / {exit} /^torchmaster: / {print; next} {exit}'
}

# ---- counting the arena -------------------------------------------------------------
# Mobs inside the sealed box `cx`,`cz` (33-block half width, y 200..209) that are not in
# the friendly list: the monster families the Mega Torch vetoes.
box_mobs() { # cx cz
    ctl entities 48 | jq -r --argjson f "$FRIENDLY" --argjson cx "$1" --argjson cz "$2" '
        [ .entities[]
          | select(.kind == "mob")
          | select(.type as $t | $f | index($t) == null)
          | select((.pos[0] | floor) >= $cx - 33 and (.pos[0] | floor) <= $cx + 33)
          | select((.pos[1] | floor) >= 200 and (.pos[1] | floor) <= 209)
          | select((.pos[2] | floor) >= $cz - 33 and (.pos[2] | floor) <= $cz + 33)
        ] | length'
}

# The ambient family the Dread Lamp vetoes that a sealed dark box can grow.
box_bats() { # cx cz
    ctl entities 48 | jq -r --argjson cx "$1" --argjson cz "$2" '
        [ .entities[]
          | select(.kind == "mob" and .type == "minecraft:bat")
          | select((.pos[0] | floor) >= $cx - 33 and (.pos[0] | floor) <= $cx + 33)
          | select((.pos[1] | floor) >= 200 and (.pos[1] | floor) <= 209)
          | select((.pos[2] | floor) >= $cz - 33 and (.pos[2] | floor) <= $cz + 33)
        ] | length'
}

# What the box holds, so a failing wait says which family the game actually spawned.
census() { # cx cz
    ctl entities 48 | jq -r --argjson cx "$1" --argjson cz "$2" '
        [ .entities[]
          | select(.kind == "mob")
          | select((.pos[0] | floor) >= $cx - 33 and (.pos[0] | floor) <= $cx + 33)
          | select((.pos[1] | floor) >= 200 and (.pos[1] | floor) <= 209)
          | select((.pos[2] | floor) >= $cz - 33 and (.pos[2] | floor) <= $cz + 33)
          | .type ] | group_by(.) | map("\(.[0]) x\(length)") | join(", ")'
}

# Wait up to `max` ticks for `want` mobs of a family in a box; prints the tick count.
wait_for_mobs() { # cx cz max
    local cx=$1 cz=$2 max=$3 waited=0 count
    while [ "$waited" -lt "$max" ]; do
        ctl wait 20 >/dev/null
        waited=$((waited + 20))
        count=$(box_mobs "$cx" "$cz")
        if [ "$count" -gt 0 ]; then
            echo "$waited"
            return 0
        fi
    done
    echo "$waited"
    return 1
}

# Wait up to `max` ticks for a bat in a box; prints the tick count.
wait_for_bats() { # cx cz max
    local cx=$1 cz=$2 max=$3 waited=0 count
    while [ "$waited" -lt "$max" ]; do
        ctl wait 20 >/dev/null
        waited=$((waited + 20))
        count=$(box_bats "$cx" "$cz")
        if [ "$count" -gt 0 ]; then
            echo "$waited"
            return 0
        fi
    done
    echo "$waited"
    return 1
}

kill_mobs() {
    ctl command "kill @e[type=!player]" >/dev/null || true
    ctl command "kill @e[type=item]" >/dev/null || true
    ctl wait 3 >/dev/null
}

block_id() { ctl block "$1" "$2" "$3" | jq -r '.block // "unknown"'; }
light_block() { ctl light "$1" "$2" "$3" "$4" "$5" "$6" | jq -r '.block'; }
invisible_lights() {
    ctl blocks "$1" "$2" "$3" "$4" "$5" "$6" | jq -r --arg i "$LIGHT" '.counts[$i] // 0'
}
# The spawner assertion parses the count of one entity type, never the absence of
# an error: `ctl entities 16 minecraft:zombie` answers a JSON list of exactly zombies.
zombie_count() { ctl entities 16 minecraft:zombie | jq -r '.entities | length'; }
# A phantom is spawned 20..34 blocks above the player, so a 16-block search
# misses it until it dives; the type-filtered count over 48 blocks is the count
# that can actually fail. `ctl entities 48 minecraft:phantom` answers exactly
# phantoms, never an error-free absence.
phantom_count() {
    ctl entities 48 minecraft:phantom \
        | jq -r '[.entities[] | select((.health // 0) > 0)] | length'
}
# Wait up to `max` ticks for a phantom, printing the waited ticks. Returns 1 if
# none appeared; the caller still reads the count so a zero is never a vacuous
# absence-of-error pass.
wait_for_phantoms() { # max
    local max=$1 waited=0 count
    while [ "$waited" -lt "$max" ]; do
        ctl wait 100 >/dev/null
        waited=$((waited + 100))
        count=$(phantom_count)
        if [ "$count" -gt 0 ]; then
            echo "$waited"
            return 0
        fi
    done
    echo "$waited"
    return 1
}
# The pearl's remaining wear, summed over the carried stacks. `ctl inventory`
# reports `damage` only when it is nonzero, so an unworn pearl reads 0.
pearl_damage() {
    ctl inventory | jq -r --arg i "$PEARL" \
        '[ (.hotbar[]?, .main[]?) | select(.item == $i) | (.damage // 0) ] | add // 0'
}
drop_count() { # x y z item -> what the break's `drops` says it dropped
    local reply drops picked status
    reply=$(ctl break "$1" "$2" "$3" --timeout 60) && status=0 || status=$?
    echo "  break at $1 $2 $3: $reply" >&2
    [ "$status" = 0 ] || return 1
    drops=$(printf '%s' "$reply" | jq -r --arg i "$4" '.drops[$i] // 0' | awk '{s+=$1} END {print s+0}')
    # `picked_up` repeats what was already in `drops` when the player grabbed the item on
    # the same tick, so the number to assert on is `drops`; picked_up is context only.
    picked=$(printf '%s' "$reply" | jq -r --arg i "$4" '.picked_up[$i] // 0' | awk '{s+=$1} END {print s+0}')
    echo "  drops $4: $drops (picked up $picked)" >&2
    echo "$drops"
}

# A sealed room: stone floor at `y`, roof at `y+8` of `roof`, walls to the roof. The
# console's `fill` only writes loaded chunks, so the player is teleported to the column
# first (and falls to the ground outside the box while the chunks load); every caller
# teleports back inside once it is built.
build_room() { # cx cz y roof-block
    local cx=$1 cz=$2 y=$3 roof=$4
    local top=$((y + 8))
    ctl tp "$cx.5" 201 "$cz.5" >/dev/null
    ctl wait 60 >/dev/null
    ctl command "fill $((cx - 32)) $y $((cz - 32)) $((cx + 32)) $y $((cz + 32)) minecraft:stone" >/dev/null
    ctl command "fill $((cx - 32)) $top $((cz - 32)) $((cx + 32)) $top $((cz + 32)) $roof" >/dev/null
    ctl command "fill $((cx - 33)) $y $((cz - 33)) $((cx - 33)) $top $((cz + 33)) minecraft:stone" >/dev/null
    ctl command "fill $((cx + 33)) $y $((cz - 33)) $((cx + 33)) $top $((cz + 33)) minecraft:stone" >/dev/null
    ctl command "fill $((cx - 33)) $y $((cz - 33)) $((cx + 33)) $top $((cz - 33)) minecraft:stone" >/dev/null
    ctl command "fill $((cx - 33)) $y $((cz + 33)) $((cx + 33)) $top $((cz + 33)) minecraft:stone" >/dev/null
    ctl command "fill $((cx - 32)) $((y + 1)) $((cz - 32)) $((cx + 32)) $((top - 1)) $((cz + 32)) minecraft:air" >/dev/null
    ctl wait 5 >/dev/null
}


# Phantom spawning needs an insomniac player: the game grows
# `minecraft:time_since_rest` one tick at a time and refuses a spawn until
# 72000. Waiting that long in real time is not a harness; seed the stat instead.
# A fresh world only knows its owner after the first save, so open it once,
# close it, write the stat for the offline UUID of "Player", then reopen the
# same world with the insomnia fixture in place.
echo "== seed: open the world once so its player stats file exists =="
launch "TorchmasterR9"
ctl quit >/dev/null 2>&1 || true
sleep 5
# The game roots worlds at <cwd>/saves/<folder> (game/world_loading.rs:419).
STATS_DIR=$SCRATCH/saves/TorchmasterR9/players/stats
mkdir -p "$STATS_DIR"
# The launcher gives the owner a random name (e.g. Player89710), so their UUID
# is NOT offline_uuid("Player"). Seed every stats file the world already wrote
# and every UUID it has player data for; one of them is the owner the server
# thread loads. That is what makes the insomnia fixture independent of the name.
INSOMNIA='{"stats":{"minecraft:custom":{"minecraft:time_since_rest":1000000}},"DataVersion":3953}'
for f in "$STATS_DIR"/*.json; do
    [ -f "$f" ] || continue
    printf '%s\n' "$INSOMNIA" > "$f"
done
for dat in "$SCRATCH/saves/TorchmasterR9/players/data"/*.dat; do
    [ -f "$dat" ] || continue
    printf '%s\n' "$INSOMNIA" > "$STATS_DIR/$(basename "$dat" .dat).json"
done
echo "  saved player data: $(ls "$SCRATCH/saves/TorchmasterR9/players/data" 2>/dev/null | tr '\n' ' ')"
echo "  stats files seeded: $(for f in "$STATS_DIR"/*.json; do [ -f "$f" ] && basename "$f"; done | tr '\n' ' ')"

echo "== launch (launcher build of $MOD, offscreen, creative) =="
launch "TorchmasterR9"

if [ "$CONTROL" = 1 ]; then
    say "control: the unmodded Nightly must know nothing of the package"
    ctl command "fill -4 99 -4 4 99 4 minecraft:stone" >/dev/null
    ctl command "fill -4 100 -4 4 110 4 minecraft:air" >/dev/null
    ctl tp 0.5 100 3.5 >/dev/null
    ctl wait 10
    before=$(ctl inventory | jq -r --arg i "$MEGA" '.counts[$i] // 0')
    ctl command "give @a $MEGA 1" >/dev/null || true
    ctl wait 3
    after=$(ctl inventory | jq -r --arg i "$MEGA" '.counts[$i] // 0')
    ctl setblock 0 100 0 "$MEGA" >/dev/null || true
    ctl wait 3
    placed=$(block_id 0 100 0)
    reply=$(readout "/torchmaster" | wc -l | tr -d ' ')
    echo "  megatorch before/after give: $before/$after; block at 0 100 0: $placed; readout lines: $reply"
    [ "$after" = "$before" ] || fail "the unmodded game gave $MEGA, so the id was known"
    [ "$placed" = "minecraft:air" ] || fail "the unmodded game placed $placed, so the id was known"
    [ "$reply" = "0" ] || fail "the unmodded game answered /torchmaster, so the command was known"
    fail "the unmodded control game refuses every id and the readout, as it must; the same assertions pass on the packaged game, so they measure the package"
fi

ctl command "gamerule doMobSpawning true" >/dev/null || true
ctl command "gamerule doDaylightCycle false" >/dev/null || true
ctl command "gamerule spawn_monsters true" >/dev/null || true
ctl command "gamerule random_tick_speed 0" >/dev/null || true
ctl command "difficulty hard" >/dev/null
ctl command "time set noon" >/dev/null
ctl command "weather clear" >/dev/null

say "arena A: a sealed dark box at y=200, the player inside it"
build_room 0 0 200 minecraft:stone
ctl tp 0.5 201 0.5 >/dev/null
ctl wait 20
[ "$(block_id 0 200 0)" = "minecraft:stone" ] || fail "arena A's floor was not built"

# ---- C. the readout ---------------------------------------------------------------
say "C. /torchmaster is the readout: fresh, placed, listed, broken"
fresh=$(readout "/torchmaster")
echo "  /torchmaster -> $fresh"
[ "$fresh" = "torchmaster: no blocking lights" ] || fail "a fresh world reads '$fresh'"

ctl command "clear @a" >/dev/null
ctl give "$MEGA" 1 >/dev/null || fail "give $MEGA failed"
ctl give "$LAMP" 1 >/dev/null || fail "give $LAMP failed"
ctl wait 3
for id in "$MEGA" "$LAMP"; do
    held=$(ctl inventory | jq -r --arg i "$id" '.counts[$i] // 0')
    echo "  inventory: $id $held"
    [ "$held" -ge 1 ] || fail "$id was not given, so the package's item is not registered"
done

ctl setblock 2 201 0 "$MEGA" >/dev/null
ctl wait 3
[ "$(block_id 2 201 0)" = "$MEGA" ] || fail "the Mega Torch did not place ($(block_id 2 201 0))"
placed=$(readout "/torchmaster")
echo "  /torchmaster -> $placed"
[ "$placed" = "torchmaster: 1 blocking light: 1 megatorch" ] || fail "the readout is '$placed'"
listed=$(readout "/torchmaster list")
echo "  /torchmaster list -> $(echo "$listed" | tr '\n' '|')"
echo "$listed" | grep -qx "torchmaster: minecraft:overworld 2 201 0 megatorch" \
    || fail "the list does not name the placed torch: $(echo "$listed" | tr '\n' '|')"
bogus=$(readout "/torchmaster nonsense")
[ -z "$bogus" ] || fail "a bogus subcommand answered '$bogus'"
ctl break 2 201 0 --timeout 30 >/dev/null || fail "breaking the creative Megatorch failed"
ctl wait 3
[ "$(block_id 2 201 0)" = "minecraft:air" ] || fail "the Megatorch did not break"
broken=$(readout "/torchmaster")
echo "  /torchmaster after the break -> $broken"
[ "$broken" = "torchmaster: no blocking lights" ] || fail "the broken torch is still registered: '$broken'"

# ---- A. monster veto --------------------------------------------------------------
say "A. the Mega Torch vetoes monster spawns in its cube"

say "A1. control: no light in the box"
kill_mobs
if waited=$(wait_for_mobs 0 0 400); then
    control_hostile=$(box_mobs 0 0)
else
    control_hostile=0
fi
echo "  control: $control_hostile monsters in the box after $waited ticks"
[ "$control_hostile" -gt 0 ] || fail "control saw no monsters in a dark sealed box"

say "A2. the Mega Torch above the roof: its light is sealed out, the hook is not"
ctl command "setblock 0 210 0 $MEGA" >/dev/null
ctl wait 3
[ "$(block_id 0 210 0)" = "$MEGA" ] || fail "the torch above the roof was not placed"
hook_state=$(readout "/torchmaster")
echo "  /torchmaster -> $hook_state"
[ "$hook_state" = "torchmaster: 1 blocking light: 1 megatorch" ] \
    || fail "the torch above the roof is not in the registry: '$hook_state'"
room_light=$(light_block 0 201 0 0 201 0)
echo "  block light in the room: $room_light"
[ "$room_light" = "0" ] || fail "the torch's light reached the room (block light $room_light)"
kill_mobs
if waited=$(wait_for_mobs 0 0 400); then
    torch_hostile=$(box_mobs 0 0)
else
    torch_hostile=0
fi
echo "  with the torch: $torch_hostile monsters after $waited ticks (control $control_hostile)"
if [ "$SELFTEST" = 1 ]; then
    # Inverted on purpose: the vetoed box must show spawns, so the run MUST fail.
    [ "$torch_hostile" -gt 0 ] || fail "SELFTEST: expected a spawn with the torch in range, saw none"
    fail "SELFTEST: the mega torch let $torch_hostile monsters spawn in its cube, so the green assertion can fail (this exit 1 is the proof)"
fi
[ "$torch_hostile" -eq 0 ] || fail "the Mega Torch let $torch_hostile monsters spawn in its cube"

say "A3. a second box 160 blocks away, the same torch still placed: out of range vetoes nothing"
build_room 160 0 200 minecraft:stone
ctl tp 160.5 201 0.5 >/dev/null
ctl wait 40
[ "$(block_id 160 200 0)" = "minecraft:stone" ] || fail "arena B's floor was not built"
kill_mobs
if waited=$(wait_for_mobs 160 0 400); then
    outside_hostile=$(box_mobs 160 0)
else
    outside_hostile=0
fi
echo "  outside the radius: $outside_hostile monsters after $waited ticks"
[ "$outside_hostile" -gt 0 ] || fail "a box 160 blocks from the torch saw no monsters, so the veto is not range-bounded"

say "A4. breaking the torch brings the spawns back to box A"
ctl tp 0.5 201 0.5 >/dev/null
ctl wait 40
ctl command "setblock 0 210 0 minecraft:air" >/dev/null
ctl wait 3
[ "$(block_id 0 210 0)" = "minecraft:air" ] || fail "the torch was not removed"
returned_state=$(readout "/torchmaster")
echo "  /torchmaster after the break -> $returned_state"
[ "$returned_state" = "torchmaster: no blocking lights" ] || fail "the torch is still registered: '$returned_state'"
kill_mobs
if waited=$(wait_for_mobs 0 0 400); then
    returned_hostile=$(box_mobs 0 0)
else
    returned_hostile=0
fi
echo "  after the break: $returned_hostile monsters after $waited ticks"
[ "$returned_hostile" -gt 0 ] || fail "spawns did not return after the torch broke"

# ---- B. the Dread Lamp vetoes the ambient family, and only in range ----------------
# Bats are the ambient family a sealed dark box can grow (the passive `creature` cap is
# zero in a level this small: `10 * spawnable_chunks / 289`), and the Dread Lamp vetoes
# every friendly category. Monsters must keep spawning in the same box with the lamp
# placed: that is the in-game category differential.
say "B. the Dread Lamp in box A: ambient spawns stop, monsters do not"
ctl tp 0.5 201 0.5 >/dev/null
ctl wait 20
kill_mobs
if waited=$(wait_for_bats 0 0 800); then
    control_bats=$(box_bats 0 0)
else
    control_bats=0
fi
echo "  control: $control_bats bats after $waited ticks; box census: $(census 0 0)"
[ "$control_bats" -gt 0 ] || fail "no bat spawned in the dark control box"

say "B2. the Dread Lamp at the box centre vetoes the ambient family"
ctl command "setblock 0 201 0 $LAMP" >/dev/null
ctl wait 3
[ "$(block_id 0 201 0)" = "$LAMP" ] || fail "the Dread Lamp did not place"
lamp_state=$(readout "/torchmaster")
echo "  /torchmaster -> $lamp_state"
[ "$lamp_state" = "torchmaster: 1 blocking light: 1 dreadlamp" ] || fail "the lamp is not in the registry: '$lamp_state'"
kill_mobs
if waited=$(wait_for_bats 0 0 800); then
    lamp_bats=$(box_bats 0 0)
else
    lamp_bats=0
fi
echo "  with the lamp: $lamp_bats bats after $waited ticks (control $control_bats); box census: $(census 0 0)"
if [ "$SELFTEST" = 2 ]; then
    [ "$lamp_bats" -gt 0 ] || fail "SELFTEST: expected a bat with the lamp in range, saw none"
    fail "SELFTEST: the Dread Lamp let $lamp_bats bats spawn in its cube (this exit 1 is the proof)"
fi
[ "$lamp_bats" -eq 0 ] || fail "the Dread Lamp let $lamp_bats bats spawn in its cube"
if waited=$(wait_for_mobs 0 0 400); then
    lamp_hostile=$(box_mobs 0 0)
else
    lamp_hostile=0
fi
echo "  with the lamp: $lamp_hostile monsters after $waited ticks (the lamp must not veto them)"
[ "$lamp_hostile" -gt 0 ] || fail "the Dread Lamp stopped monster spawns too, so it is not category-specific"

say "B3. box B 160 blocks away, the lamp still placed: out of range vetoes nothing"
ctl tp 160.5 201 0.5 >/dev/null
ctl wait 40
kill_mobs
if waited=$(wait_for_bats 160 0 800); then
    outside_bats=$(box_bats 160 0)
else
    outside_bats=0
fi
echo "  outside the lamp's radius: $outside_bats bats after $waited ticks; box census: $(census 160 0)"
[ "$outside_bats" -gt 0 ] || fail "a dark box 160 blocks from the lamp saw no bat, so the lamp's veto is not range-bounded"

# ---- D. a broken light drops its item ---------------------------------------------
# The player stays in box B, whose floor is already built: a `ctl tp` into a chunk that
# is still loading can drop the player through a floor and out of reach of the block.
say "D. survival breaks drop the block; a vanilla crafting table is the control"
ctl command "difficulty peaceful" >/dev/null
ctl command "gamemode survival @a" >/dev/null
ctl command "clear @a" >/dev/null
ctl wait 3
ctl setblock 162 201 0 "minecraft:crafting_table" >/dev/null
ctl setblock 164 201 0 "$MEGA" >/dev/null
ctl setblock 166 201 0 "$LAMP" >/dev/null
ctl wait 3
echo "  placed: crafting_table $(block_id 162 201 0), megatorch $(block_id 164 201 0), dreadlamp $(block_id 166 201 0)"
[ "$(block_id 164 201 0)" = "$MEGA" ] || fail "the survival Megatorch did not place"
[ "$(block_id 166 201 0)" = "$LAMP" ] || fail "the survival Dread Lamp did not place"
ctl tp 160.5 201 0.5 >/dev/null; ctl wait 3 >/dev/null
control_drop=$(drop_count 162 201 0 minecraft:crafting_table)
ctl tp 162.5 201 0.5 >/dev/null; ctl wait 3 >/dev/null
torch_drop=$(drop_count 164 201 0 "$MEGA")
ctl tp 164.5 201 0.5 >/dev/null; ctl wait 3 >/dev/null
lamp_drop=$(drop_count 166 201 0 "$LAMP")
echo "  drops: crafting_table $control_drop, megatorch $torch_drop, dreadlamp $lamp_drop"
[ "${control_drop:-0}" -ge 1 ] || fail "the vanilla crafting table dropped nothing, so the break/drop measurement is broken"
[ "${torch_drop:-0}" -ge 1 ] || fail "the broken Megatorch dropped no $MEGA (control dropped $control_drop)"
[ "${lamp_drop:-0}" -ge 1 ] || fail "the broken Dread Lamp dropped no $LAMP (control dropped $control_drop)"
ctl command "gamemode creative @a" >/dev/null

# ---- E. the Feral Flare Lantern and its invisible light ------------------------------
say "E. the Feral Flare Lantern writes invisible light, and breaking it takes it back"
ctl command "gamemode creative @a" >/dev/null
ctl command "setblock 0 201 0 minecraft:air" >/dev/null   # the Dread Lamp from B
ctl tp 0.5 201 0.5 >/dev/null
ctl wait 20
[ "$(block_id 0 201 0)" = "minecraft:air" ] || fail "arena A was not cleared for the flare leg"

ctl setblock 0 201 0 "$FLARE" >/dev/null
ctl wait 60
flare_count=$(invisible_lights -8 200 -8 8 205 8)
flare_lit_hex=$(light_block 0 201 0 0 201 0)
# `ctl light` prints each layer as hex (15 -> "f"); parse it before comparing.
flare_lit=$((16#$flare_lit_hex))
echo "  lantern at 0 201 0: invisible lights in the room = $flare_count, its block light = $flare_lit (0x$flare_lit_hex)"
[ "$(block_id 0 201 0)" = "$FLARE" ] || fail "the Feral Flare Lantern did not place"
[ "$flare_count" -gt 0 ] || fail "the lantern wrote no $LIGHT in 60 ticks"
[ "$flare_lit" -gt 0 ] || fail "the lantern declares no light (block light $flare_lit)"
if [ "$SELFTEST" = flare ]; then
    # Inverted on purpose: the placed lantern must have written a light, so this
    # run MUST fail. Every earlier phase stays green, isolating this assertion.
    [ "$flare_count" -eq 0 ] || fail "SELFTEST: expected no invisible light with the lantern placed, saw $flare_count"
    fail "SELFTEST: the inverted flare assertion passed (exit 1 is the counterfactual)"
fi

ctl command "setblock 0 201 0 minecraft:air" >/dev/null
ctl wait 20
after_flare=$(invisible_lights -8 200 -8 8 205 8)
echo "  after breaking it: invisible lights in the room = $after_flare"
[ "$after_flare" -eq 0 ] || fail "breaking the lantern left $after_flare invisible lights behind"

say "E2. differential: a plain stone block writes no invisible light"
build_room 320 0 200 minecraft:stone
ctl tp 320.5 201 0.5 >/dev/null
ctl wait 20
ctl command "setblock 320 201 0 minecraft:stone" >/dev/null
ctl wait 60
stone_lights=$(invisible_lights 312 200 312 328 205 328)
echo "  stone in a sealed dark box: invisible lights = $stone_lights"
[ "$(block_id 320 201 0)" = "minecraft:stone" ] || fail "the stone differential was not placed"
[ "$stone_lights" -eq 0 ] || fail "a plain stone block wrote $stone_lights invisible lights"

# ---- F. the Frozen Pearl clears residual light and spends one charge per light --------
# The lantern was broken in phase E, so nothing rewrites these cells. Writing the
# residual lights directly stands in for a world loaded with a lantern's lights
# (the blocked(persistence) gap) and makes the count deterministic.
say "F. the Frozen Pearl removes residual light in the player's cube, one charge each"
ctl command "gamemode creative @a" >/dev/null
ctl command "difficulty peaceful" >/dev/null
# Build/reload the arena while creative: a survival `ctl tp` into a chunk that is
# still loading drops the player through the floor (and can kill them). Switching
# to survival happens only after the player stands on the loaded floor.
build_room 0 0 200 minecraft:stone
ctl tp 0.5 201 0.5 >/dev/null
ctl wait 20
floor=$(block_id 0 200 0)
[ "$floor" = "minecraft:stone" ] || fail "arena A was not loaded for the pearl leg ($floor)"
ctl command "setblock -2 201 0 $LIGHT" >/dev/null
ctl command "setblock -3 201 0 $LIGHT" >/dev/null
ctl command "setblock -4 201 0 $LIGHT" >/dev/null
ctl wait 5
[ "$(block_id -2 201 0)" = "$LIGHT" ] || fail "the residual light fixture was not written ($(block_id -2 201 0))"
pearl_before=$(invisible_lights -8 200 -8 8 205 8)
echo "  residual lights before the pearl: $pearl_before"
[ "$pearl_before" -gt 0 ] || fail "the residual-light fixture was not written"

ctl command "gamemode survival @a" >/dev/null
ctl command "clear @a" >/dev/null
give_reply=$(ctl give "$PEARL" 1) || fail "give $PEARL failed: $give_reply"
ctl wait 10
echo "  give: $give_reply"
echo "  hotbar: $(ctl inventory | jq -c '[.hotbar[] | select(.item == "'"$PEARL"'")]')"
pearl_slot=$(ctl inventory | jq -r --arg i "$PEARL" '[.hotbar[] | select(.item == $i) | .slot][0] // empty')
[ -n "$pearl_slot" ] || fail "the frozen pearl is not in the hotbar after give"
# `ctl inventory` numbers slots from 0; `ctl select` takes hotbar positions 1-9.
pearl_slot=$((pearl_slot + 1))
ctl select "$pearl_slot" >/dev/null || fail "select slot $pearl_slot failed"
ctl wait 4
echo "  held: $(ctl state | jq -c '.held')"
dmg_before=$(pearl_damage)
echo "  pearl damage before: $dmg_before"
ctl use-item >/dev/null || fail "ctl use-item failed"
ctl wait 4
pearl_after=$(invisible_lights -8 200 -8 8 205 8)
dmg_after=$(pearl_damage)
echo "  after the pearl: invisible lights $pearl_after; pearl damage $dmg_before -> $dmg_after"
if [ "$SELFTEST" = pearl ]; then
    # Inverted on purpose: the residual lights must remain, so this run MUST fail.
    [ "$pearl_after" -eq "$pearl_before" ] \
        || fail "SELFTEST: expected $pearl_before residual lights to remain, saw $pearl_after"
    fail "SELFTEST: the inverted pearl assertion passed (exit 1 is the counterfactual)"
fi
[ "$pearl_after" -eq 0 ] || fail "the pearl left $pearl_after residual lights"
[ "$dmg_after" -gt "$dmg_before" ] \
    || fail "the pearl removed lights without spending a charge ($dmg_before -> $dmg_after)"

# Differential: the same pearl used on a vanilla-lit cell with no residual light
# inside its cube. The fixture is left in box A; the player stands in box B, 160
# blocks away, next to a vanilla torch. Neither the count nor the wear may move.
ctl command "setblock -2 201 0 $LIGHT" >/dev/null
ctl command "setblock -3 201 0 $LIGHT" >/dev/null
ctl wait 5
standby=$(invisible_lights -8 200 -8 8 205 8)
# Again, build/reload box B in creative before returning to survival.
ctl command "gamemode creative @a" >/dev/null
build_room 160 0 200 minecraft:stone
ctl tp 160.5 201 0.5 >/dev/null
ctl wait 10
ctl command "setblock 161 201 0 minecraft:torch" >/dev/null
ctl command "gamemode survival @a" >/dev/null
ctl wait 3
[ "$(block_id 161 201 0)" = "minecraft:torch" ] || fail "the vanilla-lit differential cell was not built"
ctl use-item >/dev/null || fail "ctl use-item (differential) failed"
ctl wait 4
standby_after=$(invisible_lights -8 200 -8 8 205 8)
dmg_diff=$(pearl_damage)
echo "  differential on a vanilla-lit cell: box A lights $standby -> $standby_after; pearl damage $dmg_after -> $dmg_diff"
[ "$standby_after" -eq "$standby" ] \
    || fail "a pearl used where no residual light was in range changed the count ($standby -> $standby_after)"
[ "$dmg_diff" -eq "$dmg_after" ] \
    || fail "a pearl used where nothing was removed lost a charge ($dmg_after -> $dmg_diff)"
ctl command "setblock 161 201 0 minecraft:air" >/dev/null
ctl command "gamemode creative @a" >/dev/null

# ---- G. the feral lantern's light quietens a mob spawner -----------------------------
say "G. a lit feral lantern suppresses a zombie spawner through its light"
ctl command "gamemode creative @a" >/dev/null
ctl command "gamerule doMobSpawning true" >/dev/null || true
ctl command "gamerule spawn_monsters true" >/dev/null || true
ctl command "difficulty hard" >/dev/null
# /setblock in a chunk that is still loading answers "That position is not loaded":
# rebuild the arena and stand on the loaded floor before placing the spawner.
build_room 0 0 200 minecraft:stone
ctl tp 0.5 201 0.5 >/dev/null
ctl wait 20
[ "$(block_id 0 200 0)" = "minecraft:stone" ] || fail "arena A was not loaded for the spawner leg"
kill_mobs

say "G1. a zombie spawner in the sealed dark box spawns mobs"
ctl command "setblock 0 201 0 minecraft:spawner{SpawnData:{entity:{id:\"minecraft:zombie\"}}}" >/dev/null
ctl wait 5
spawner_block=$(block_id 0 201 0)
echo "  spawner block: $spawner_block"
[ "$spawner_block" = "minecraft:spawner" ] || fail "the zombie spawner did not place ($spawner_block)"
ctl wait 400
spawner_lit=$(zombie_count)
echo "  dark spawner: $spawner_lit zombies within 16 blocks"
[ "$spawner_lit" -gt 0 ] || fail "a spawner in a dark sealed box spawned no zombies"

say "G2. the lantern's light raises the spawner cell above vanilla's limit"
ctl setblock 2 201 0 "$FLARE" >/dev/null
ctl wait 60
echo "  lantern block light at the spawner: $(light_block 0 201 0 0 201 0) (hex)"
kill_mobs
ctl wait 400
spawner_dark=$(zombie_count)
echo "  spawner beside a lit lantern: $spawner_dark zombies within 16 blocks"
if [ "$SELFTEST" = spawner ]; then
    # Inverted on purpose: the lantern must quieten the spawner, so this run MUST
    # fail. Every earlier phase stays green, isolating this assertion.
    [ "$spawner_dark" -eq "$spawner_lit" ] \
        || fail "SELFTEST: expected the spawner count to stay $spawner_lit, saw $spawner_dark"
    fail "SELFTEST: the inverted spawner assertion passed (exit 1 is the counterfactual)"
fi
[ "$spawner_dark" -lt "$spawner_lit" ] \
    || fail "the lantern's light did not quieten the spawner ($spawner_lit -> $spawner_dark)"
ctl command "setblock 0 201 0 minecraft:air" >/dev/null
ctl command "setblock 2 201 0 minecraft:air" >/dev/null
ctl command "gamemode creative @a" >/dev/null

# ---- H. a Mega Torch vetoes phantom spawns near an insomniac player ---------------
say "H. the phantom veto: a Mega Torch in range stops phantoms under an open night sky"
ctl command "gamemode creative @a" >/dev/null
ctl command "gamerule doMobSpawning true" >/dev/null || true
ctl command "gamerule spawn_monsters true" >/dev/null || true
ctl command "gamerule spawn_phantoms true" >/dev/null || true
ctl command "difficulty hard" >/dev/null
time_reply=$(ctl time 18000)
echo "  ctl time 18000 -> $time_reply"
ctl wait 5 >/dev/null
# Vanilla's phantom spawner needs can_see_sky at the player, so this arena has a
# floor and NO roof (unlike the sealed monster boxes A/B). The player's insomnia
# was seeded from a stats file before the launch (see the seed phase at the top).
ctl tp 480.5 201 0.5 >/dev/null
ctl wait 60 >/dev/null
ctl command "fill 448 200 -32 512 200 32 minecraft:stone" >/dev/null
ctl wait 5 >/dev/null
ctl tp 480.5 201 0.5 >/dev/null
ctl wait 10
[ "$(block_id 480 200 0)" = "minecraft:stone" ] || fail "phantom arena floor was not built"
echo "  phantom conditions: state=$(ctl state | jq -c '{screen,day_time,dimension}'), light at player=$(ctl light 480 201 0 480 201 0 | jq -c .)"
echo "  time query: $(ctl command 'time query daytime')"
echo "  registry at H: $(readout '/torchmaster' | tr '\n' '|')"

say "H1. control: an insomniac player under an open night sky grows phantoms"
ctl command "kill @e[type=minecraft:phantom]" >/dev/null || true
ctl wait 3 >/dev/null
waited=$(wait_for_phantoms 5200 || true)
control_phantoms=$(phantom_count)
echo "  control: $control_phantoms phantoms within 48 blocks after $waited ticks"
[ "$control_phantoms" -gt 0 ] || fail "an insomniac player under an open night sky grew no phantoms"

say "H2. a Mega Torch within the artefact's 64-block radius vetoes the attempt"
ctl setblock 490 208 0 "$MEGA" >/dev/null
ctl wait 3
[ "$(block_id 490 208 0)" = "$MEGA" ] || fail "the phantom-veto Mega Torch did not place"
ctl command "kill @e[type=minecraft:phantom]" >/dev/null || true
ctl wait 3 >/dev/null
# H1 proved an attempt happens and the spawner's timer resets to 1200..2400
# ticks; two full windows make sure at least one vetoed attempt falls here even
# if the timer had just reset when the torch was placed.
ctl wait 5200 >/dev/null
torch_phantoms=$(phantom_count)
echo "  with the torch: $torch_phantoms phantoms after 5200 ticks (control $control_phantoms)"
if [ "$SELFTEST" = phantom ]; then
    # Inverted on purpose: the placed torch must have stopped the spawns, so
    # this run MUST fail. Every earlier phase stays green, isolating this assertion.
    [ "$torch_phantoms" -gt 0 ] || fail "SELFTEST: expected phantoms despite the torch, saw $torch_phantoms"
    fail "SELFTEST: the inverted phantom assertion passed (exit 1 is the counterfactual)"
fi
[ "$torch_phantoms" -lt "$control_phantoms" ] \
    || fail "the Mega Torch did not reduce phantom spawns ($control_phantoms -> $torch_phantoms)"
ctl command "setblock 490 208 0 minecraft:air" >/dev/null

echo "PASS: control=$control_hostile, torch=$torch_hostile, outside=$outside_hostile, after_break=$returned_hostile monsters; lamp_control=$control_bats, lamp=$lamp_bats, lamp_outside=$outside_bats bats, lamp_box_monsters=$lamp_hostile; drops torch=$torch_drop lamp=$lamp_drop control=$control_drop; flare=$flare_count lights, after break $after_flare, stone control $stone_lights; pearl $pearl_before -> $pearl_after lights for damage $dmg_before -> $dmg_after, vanilla cell $standby -> $standby_after for damage $dmg_diff; spawner $spawner_lit -> $spawner_dark zombies beside a lit lantern; phantom $control_phantoms -> $torch_phantoms near an insomniac player; /torchmaster readouts matched at every phase"
exit 0
