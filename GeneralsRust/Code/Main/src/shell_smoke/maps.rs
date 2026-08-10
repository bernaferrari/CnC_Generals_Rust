//! Host map resolution for shell smoke (wraps production resolve_first_map).

use super::helpers::HOST_MAP_CANDIDATES;
use crate::map_frame_scenario::resolve_first_map;
use std::path::PathBuf;

/// Resolve the first existing host map from [`HOST_MAP_CANDIDATES`].
pub(super) fn resolve_host_map() -> Option<(String, PathBuf)> {
    resolve_first_map(HOST_MAP_CANDIDATES)
}
