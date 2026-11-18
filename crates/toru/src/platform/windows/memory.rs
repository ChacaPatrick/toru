#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> isize;
    fn SetProcessWorkingSetSize(process: isize, minimum: usize, maximum: usize) -> i32;
}

/// Windows does this itself for a minimized window but not for a hidden one, so a Toru waiting
/// in the tray would otherwise hold on to everything its window last drew with.
pub fn trim_working_set() {
    // SAFETY: the pseudo-handle of this process needs no closing, and the two `-1`s are the
    // documented request to empty the working set.
    unsafe { SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX) };
}
