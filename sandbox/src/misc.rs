use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::os::unix::io::RawFd;

use nix::fcntl::{FcntlArg, FdFlag};

/// Check if the given string slice is a valid C-style string.
///
/// Formally, this function checks whether the byte sequence of the string slice contains any
/// b'\x00'. If so, this function returns `false`.
///
/// ```ignore
/// assert!(is_valid_c_string("abc哈哈哈"));
/// assert!(!is_valid_c_string("abc\x00哈哈哈"));
/// ```
///
pub fn is_valid_c_string(s: &str) -> bool {
    !s.as_bytes().contains(&b'\x00')
}

/// Get number of clocks in one second.
fn clocks_per_sec() -> i64 {
    // The `CLOCKS_PER_SEC` constant corresponds to the macro with the same name used in C. Posix
    // standard requires this constant set to one million on every platform, regardless of the
    // actual clock speed. We use this constant as a fallback when `sysconf` fails.
    const CLOCKS_PER_SEC: i64 = 1000000;

    let ret = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if ret == -1 {
        log::warn!("Failed to get system clock speed through sysconf. Use CLOCKS_PER_SEC instead.");
        CLOCKS_PER_SEC
    } else {
        ret
    }
}

/// Create a `Duration` instance from clocks number.
pub fn duration_from_clocks(clocks: libc::clock_t) -> Duration {
    Duration::from_secs_f64(clocks as f64 / clocks_per_sec() as f64)
}

/// This function calls `dup2(old_fd, new_fd)` and set the `O_CLOEXEC` flag on the old file
/// descriptor. This function is useful when duplicating file descriptors for standard streams
/// that can effectively prevent the original file descriptors from leaking.
pub fn dup_and_cloexec(old_fd: RawFd, new_fd: RawFd) -> nix::Result<()> {
    nix::unistd::dup2(old_fd, new_fd)?;
    nix::fcntl::fcntl(old_fd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))?;

    Ok(())
}

/// Expand the `PATH` environment variable before the given path and returns the one that exists.
pub fn expand_path<'a, P>(path: &'a P) -> Option<Cow<'a, Path>>
    where P: ?Sized + AsRef<Path> {
    let original = path.as_ref();
    if original.exists() {
        return Some(Cow::Borrowed(original));
    }

    if original.is_absolute() {
        return None;
    }

    let path_env = match std::env::var_os("PATH") {
        Some(p) => p.to_string_lossy().into_owned().to_owned(),
        None => return None
    };
    for dir_path in path_env.split(':') {
        let mut path = PathBuf::from(dir_path);
        path.push(original);

        if path.exists() {
            return Some(Cow::Owned(path));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::is_valid_c_string;

    #[test]
    fn test_is_valid_c_string() {
        assert!(is_valid_c_string("abc哈哈哈"));
        assert!(!is_valid_c_string("abc\x00哈哈哈"));
    }
}
