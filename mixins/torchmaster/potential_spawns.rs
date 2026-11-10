// Runs at the head of `Game::native_potential_spawns` (API 388): natural
// spawning asks for the possible spawns at `pos`, and a mega torch or dread
// lamp in range vetoes the category by emptying the list.
fn on_potential_spawns(
    dimension: &str,
    category: &str,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    crate::game::torchmaster_state::on_potential_spawns(dimension, category, pos, canceled);
}
