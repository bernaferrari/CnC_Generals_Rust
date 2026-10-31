// Interpret attack statuses without treating failures or pending work as success.
pub(crate) fn record_attack_success(saw_success: &mut bool, status: &str) {
    if status.starts_with("attack_ok") {
        *saw_success = true;
    }
}
