//! The bundled fonts. Each is a static instance subset from the variable fonts in google/fonts,
//! because the display face only ever shows Latin headings (see `assets/fonts/README.md`).

use std::borrow::Cow;

use gpui::App;

/// Name fields use the system monospace face, as the mockups' `ui-monospace` does. On Windows
/// that is Consolas; other platforms pick their own when they arrive.
pub const MONO: &str = "Consolas";

const FONTS: [&[u8]; 5] = [
    include_bytes!("../../../../assets/fonts/NotoSerifJP-Regular.ttf"),
    include_bytes!("../../../../assets/fonts/NotoSerifJP-SemiBold.ttf"),
    include_bytes!("../../../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../../../assets/fonts/Inter-SemiBold.ttf"),
];

/// Registers the bundled fonts with GPUI. Call once, before the first window opens.
pub fn install_fonts(cx: &App) {
    let fonts = FONTS.iter().map(|data| Cow::Borrowed(*data)).collect();
    cx.text_system()
        .add_fonts(fonts)
        .expect("the bundled fonts are valid TrueType files");
}
