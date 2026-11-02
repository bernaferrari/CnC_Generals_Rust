//! Cell visitation for `Pathfinder::iterateCellsAlongLine`.
//!
//! A diagonal step visits the intermediate orthogonal cell as well. The
//! callback decides when a blocked cell or the map boundary ends the walk.

pub fn visit_cell_line(
    start: (i32, i32),
    end: (i32, i32),
    mut callback: impl FnMut((i32, i32), (i32, i32)) -> bool,
) {
    let delta_x = (end.0 - start.0).abs();
    let delta_y = (end.1 - start.1).abs();
    let (mut x, mut y) = start;
    let (mut xinc1, mut xinc2) = if end.0 >= x { (1, 1) } else { (-1, -1) };
    let (mut yinc1, mut yinc2) = if end.1 >= y { (1, 1) } else { (-1, -1) };
    let (den, mut num, numadd, numpixels);
    if delta_x >= delta_y {
        xinc1 = 0;
        yinc2 = 0;
        (den, num, numadd, numpixels) = (delta_x, delta_x / 2, delta_y, delta_x);
    } else {
        xinc2 = 0;
        yinc1 = 0;
        (den, num, numadd, numpixels) = (delta_y, delta_y / 2, delta_x, delta_y);
    }
    let mut from = None;
    for _ in 0..=numpixels {
        let mut to = (x, y);
        if let Some(from) = from {
            if !callback(from, to) {
                return;
            }
        }
        num += numadd;
        if num >= den {
            num -= den;
            x += xinc1;
            y += yinc1;
            let next = (x, y);
            if !callback(to, next) {
                return;
            }
            to = next;
        }
        x += xinc2;
        y += yinc2;
        from = Some(to);
    }
}
