// Runs at the head of `Game::native_level_tick_post` (API 23): prune lights
// whose block is gone and flush a changed registry to its world file.
fn on_level_tick_post(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::torchmaster_state::on_level_tick_post(game, dimension);
}
