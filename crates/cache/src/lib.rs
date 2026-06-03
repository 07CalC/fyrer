pub fn is_cacheable(is_persistent: bool, is_utility: bool) -> bool {
    !(is_persistent || is_utility)
}
