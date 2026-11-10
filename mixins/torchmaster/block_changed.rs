// Runs at the head of `Game::native_block_changed` (API 7): a placed,
// replaced or removed block refreshes the blocking-light registry.
fn on_block_changed(
    level: &crate::game::GameLevel<'_>,
    dimension: LevelDimension,
    pos: BlockPos,
    old: BlockStateId,
    new: BlockStateId,
) {
    crate::game::torchmaster_state::on_block_changed(level, dimension, pos, old, new);
}
