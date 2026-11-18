const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;

#[link(name = "user32")]
unsafe extern "system" {
    fn SystemParametersInfoW(action: u32, param: u32, value: *mut i32, update: u32) -> i32;
}

/// Whether the user has asked for less motion: Settings > Accessibility > Visual effects >
/// Animation effects is off. If Windows can't say, motion stays.
pub fn reduce_motion() -> bool {
    let mut animations: i32 = 1;
    // SAFETY: `SPI_GETCLIENTAREAANIMATION` writes one `BOOL` through the pointer, which
    // outlives the call.
    let read = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &mut animations, 0) };
    read != 0 && animations == 0
}
