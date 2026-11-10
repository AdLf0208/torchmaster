// Runs at the head of `Game::native_world_lifecycle` (API 3): load the
// blocking lights when the world is ready and save them when it closes.
fn on_world_lifecycle(game: &mut crate::game::Game<'_>, phase: WorldPhase) {
    crate::game::torchmaster_state::on_world_lifecycle(game, phase);
}
