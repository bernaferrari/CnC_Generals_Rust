//! 2D line helpers (ported from `Line2D.cpp`).

use crate::message_stream::game_message::{ICoord2D, IRegion2D};

const CLIP_LEFT: i32 = 0x01;
const CLIP_RIGHT: i32 = 0x02;
const CLIP_BOTTOM: i32 = 0x04;
const CLIP_TOP: i32 = 0x08;

/// Clip a line to the provided region.
/// Returns `true` if the line is visible; `false` if fully outside.
pub fn clip_line_2d(
    p1: &ICoord2D,
    p2: &ICoord2D,
    clip_region: &IRegion2D,
) -> Option<(ICoord2D, ICoord2D)> {
    let clip_left = clip_region.x;
    let clip_top = clip_region.y;
    let clip_right = clip_region.x + clip_region.width;
    let clip_bottom = clip_region.y + clip_region.height;

    let mut x1 = p1.x;
    let mut y1 = p1.y;
    let mut x2 = p2.x;
    let mut y2 = p2.y;

    let mut clip_code1 = 0;
    if x1 < clip_left {
        clip_code1 = CLIP_LEFT;
    } else if x1 > clip_right {
        clip_code1 = CLIP_RIGHT;
    }
    if y1 < clip_top {
        clip_code1 |= CLIP_TOP;
    } else if y1 > clip_bottom {
        clip_code1 |= CLIP_BOTTOM;
    }

    let mut clip_code2 = 0;
    if x2 < clip_left {
        clip_code2 = CLIP_LEFT;
    } else if x2 > clip_right {
        clip_code2 = CLIP_RIGHT;
    }
    if y2 < clip_top {
        clip_code2 |= CLIP_TOP;
    } else if y2 > clip_bottom {
        clip_code2 |= CLIP_BOTTOM;
    }

    if (clip_code1 | clip_code2) == 0 {
        return Some((p1.clone(), p2.clone()));
    }

    if (clip_code1 & clip_code2) != 0 {
        return None;
    }

    if clip_code1 != 0 {
        if (clip_code1 & CLIP_TOP) != 0 {
            let diff = y2 - y1;
            if diff == 0 {
                return None;
            }
            x1 += (x2 - x1) * (clip_top - y1) / diff;
            y1 = clip_top;
        } else if (clip_code1 & CLIP_BOTTOM) != 0 {
            let diff = y2 - y1;
            if diff == 0 {
                return None;
            }
            x1 += (x2 - x1) * (clip_bottom - y1) / diff;
            y1 = clip_bottom;
        }

        if x1 > clip_right {
            let diff = x2 - x1;
            if diff == 0 {
                return None;
            }
            y1 += (y2 - y1) * (clip_right - x1) / diff;
            x1 = clip_right;
        } else if x1 < clip_left {
            let diff = x2 - x1;
            if diff == 0 {
                return None;
            }
            y1 += (y2 - y1) * (clip_left - x1) / diff;
            x1 = clip_left;
        }
    }

    if clip_code2 != 0 {
        if (clip_code2 & CLIP_TOP) != 0 {
            let diff = y2 - y1;
            if diff == 0 {
                return None;
            }
            x2 += (x2 - x1) * (clip_top - y2) / diff;
            y2 = clip_top;
        } else if (clip_code2 & CLIP_BOTTOM) != 0 {
            let diff = y2 - y1;
            if diff == 0 {
                return None;
            }
            x2 += (x2 - x1) * (clip_bottom - y2) / diff;
            y2 = clip_bottom;
        }

        if x2 > clip_right {
            let diff = x2 - x1;
            if diff == 0 {
                return None;
            }
            y2 += (y2 - y1) * (clip_right - x2) / diff;
            x2 = clip_right;
        } else if x2 < clip_left {
            let diff = x2 - x1;
            if diff == 0 {
                return None;
            }
            y2 += (y2 - y1) * (clip_left - x2) / diff;
            x2 = clip_left;
        }
    }

    if x1 < clip_left
        || x1 > clip_right
        || y1 < clip_top
        || y1 > clip_bottom
        || x2 < clip_left
        || x2 > clip_right
        || y2 < clip_top
        || y2 > clip_bottom
    {
        return None;
    }

    Some((ICoord2D { x: x1, y: y1 }, ICoord2D { x: x2, y: y2 }))
}
