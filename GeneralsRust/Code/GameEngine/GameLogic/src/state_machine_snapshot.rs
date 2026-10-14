//! Shared ordered StateMachine.cpp803–815/863–866 control fields.
//! Concrete state payloads and transition algorithms remain with their owner.
use super::*;
type Error = Box<dyn std::error::Error + Send + Sync>;
pub(crate) fn xfer_prefix(
    xfer: &mut dyn crate::common::xfer::Xfer,
    sleep: &mut u32,
    default: &mut StateId,
    current: &mut StateId,
) -> Result<(), Error> {
    let mut version = 1;
    game_engine::system::Xfer::xfer_version(xfer, &mut version, 1)?;
    xfer.xfer_unsigned_int(sleep)?;
    xfer.xfer_unsigned_int(default)?;
    xfer.xfer_unsigned_int(current)?;
    Ok(())
}
pub(crate) fn xfer_tail(
    xfer: &mut dyn crate::common::xfer::Xfer,
    goal_object: &mut crate::common::ObjectID,
    goal_position: &mut Coord3D,
    locked: &mut bool,
    initialized: &mut bool,
) -> Result<(), Error> {
    xfer.xfer_object_id(goal_object)?;
    game_engine::system::Xfer::xfer_real(xfer, &mut goal_position.x)?;
    game_engine::system::Xfer::xfer_real(xfer, &mut goal_position.y)?;
    game_engine::system::Xfer::xfer_real(xfer, &mut goal_position.z)?;
    game_engine::system::Xfer::xfer_bool(xfer, locked)?;
    game_engine::system::Xfer::xfer_bool(xfer, initialized)?;
    Ok(())
}
