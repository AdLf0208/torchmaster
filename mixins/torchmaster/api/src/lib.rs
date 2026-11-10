//! The bundle's trait-mod install shim.
//!
//! The pin's `NaturalLevel::spawn_hooks` (`game/natural_spawning.rs:450-451`) only calls
//! `potential_spawns` when `mod_api::mods()` is non-empty, and the launcher only installs
//! bundles with an `[api]` package (`launcher/src/native_mods/api.rs`). This empty `Hooks`
//! object makes the bundle part of that list; the veto itself is the injected
//! `potential_spawns` HEAD helper in `potential_spawns.rs` / `support.rs`.
use horizon_client::mod_api::Hooks;

/// The launcher links this package and calls `hooks()` in dependency order.
pub fn hooks() -> Box<dyn Hooks> {
    Box::new(Torchmaster)
}

struct Torchmaster;

impl Hooks for Torchmaster {}
