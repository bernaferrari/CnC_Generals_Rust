pub fn binary_search<T: Ord>(slice: &[T], target: &T) -> Option<usize> {
    let mut pointer = 0usize;
    let mut stride = slice.len();

    while stride > 0 {
        let pivot = stride / 2;
        let index = pointer + pivot;
        let value = &slice[index];

        if target < value {
            stride = pivot;
        } else {
            if value == target {
                return Some(index);
            }
            pointer = index + 1;
            stride -= pivot + 1;
        }
    }
    None
}
