pub fn is_log_left_as_update_as_right(
    left_term: i32,
    left_index: i32,
    right_term: i32,
    right_index: i32,
) -> bool {
    if left_term > right_term {
        return true;
    } else if left_term < right_term {
        return false;
    } else {
        if left_index >= right_index {
            return true;
        } else {
            return false;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElectionState {
    Finished,
    Running,
    Won,
    Lost,
}