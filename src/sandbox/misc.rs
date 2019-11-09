use std::time::Duration;

/// Check if the given string slice is a valid C-style string.
///
/// Formally, this function checks whether the byte sequence of the string slice
/// contains any b'\x00'. If so, this function returns `false`.
///
/// ```
/// assert!(is_valid_c_string("abc哈哈哈"));
/// assert!(!is_valid_c_string("abc\x00哈哈哈"));
/// ```
///
pub fn is_valid_c_string(s: &str) -> bool {
    !s.as_bytes().contains(&b'\x00')
}

/// Create a `Duration` instance from clocks number.
pub fn duration_from_clocks(clocks: libc::clock_t)
    -> std::io::Result<Duration> {
    // TODO: Implement duration_from_clocks(libc::clock_t).
    unimplemented!()
}

/// Provide a RAII wrapper that can be used to implement the `defer` function.
pub struct DeferWrapper<T>
    where T: FnOnce() {
    action: Option<T>
}

impl<T> DeferWrapper<T>
    where T: FnOnce() {
    /// Create a new `DeferWrapper` instance.
    pub fn new(action: T) -> DeferWrapper<T> {
        DeferWrapper {
            action: Some(action)
        }
    }
}

impl<T> Drop for DeferWrapper<T>
    where T: FnOnce() {
    fn drop(&mut self) {
        (self.action.take().unwrap())()
    }
}

/// Defer the execution of the given function to the end of the enclosing block.
pub fn defer<T>(action: T) -> DeferWrapper<T>
    where T: FnOnce() {
    DeferWrapper::new(action)
}
