//! Everything the user can set, and how each setting is written as text for `Store`.
//!
//! Loading starts from the defaults and applies whatever is stored, so a missing key or a value
//! that doesn't parse leaves that one setting at its default and can't stop the app from starting.

use std::path::{Path, PathBuf};

use crate::capture::CaptureMode;
use crate::naming::{DEFAULT_PATTERN, validate_pattern};
use crate::store::{Result, mode_from_text, mode_text, path_text};

pub const RETENTION_PRESET_DAYS: [u32; 4] = [7, 14, 30, 90];
pub const DELAY_SECONDS: [u32; 3] = [3, 5, 10];
pub const BUILT_IN_PALETTES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFormat {
    Png,
    Jpg,
    WebP,
}

impl SaveFormat {
    fn as_stored(self) -> &'static str {
        match self {
            SaveFormat::Png => "png",
            SaveFormat::Jpg => "jpg",
            SaveFormat::WebP => "webp",
        }
    }

    fn from_stored(text: &str) -> Option<SaveFormat> {
        match text {
            "png" => Some(SaveFormat::Png),
            "jpg" => Some(SaveFormat::Jpg),
            "webp" => Some(SaveFormat::WebP),
            _ => None,
        }
    }
}

/// What happens to this session's unsaved captures when the app exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnClose {
    Ask,
    Keep,
    Discard,
}

impl OnClose {
    fn as_stored(self) -> &'static str {
        match self {
            OnClose::Ask => "ask",
            OnClose::Keep => "keep",
            OnClose::Discard => "discard",
        }
    }

    fn from_stored(text: &str) -> Option<OnClose> {
        match text {
            "ask" => Some(OnClose::Ask),
            "keep" => Some(OnClose::Keep),
            "discard" => Some(OnClose::Discard),
            _ => None,
        }
    }
}

/// How long unsaved captures are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retention {
    /// One of `RETENTION_PRESET_DAYS` or a custom count. Zero isn't valid and loads as the default.
    Days(u32),
    Never,
}

impl Retention {
    /// The count for `Store::delete_unsaved_older_than`, or `None` if nothing should be deleted.
    pub fn days(self) -> Option<u32> {
        match self {
            Retention::Days(days) => Some(days),
            Retention::Never => None,
        }
    }

    pub fn is_custom(self) -> bool {
        matches!(self, Retention::Days(days) if !RETENTION_PRESET_DAYS.contains(&days))
    }

    fn as_stored(self) -> String {
        match self {
            Retention::Days(days) => days.to_string(),
            Retention::Never => "never".to_string(),
        }
    }

    // Zero would delete all history before today, too costly a reading of a damaged value.
    fn from_stored(text: &str) -> Option<Retention> {
        if text == "never" {
            return Some(Retention::Never);
        }
        let days = text.parse::<u32>().ok().filter(|&days| days > 0)?;
        Some(Retention::Days(days))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Japandi,
    PixelArt,
    NeoBrutalism,
}

impl Look {
    const ALL: [Look; 3] = [Look::Japandi, Look::PixelArt, Look::NeoBrutalism];

    /// The keys for this look's palette, custom colours and custom background, in that order.
    fn appearance_keys(self) -> [&'static str; 3] {
        match self {
            Look::Japandi => [
                "japandi_palette",
                "japandi_custom_palette",
                "japandi_custom_background",
            ],
            Look::PixelArt => [
                "pixel_art_palette",
                "pixel_art_custom_palette",
                "pixel_art_custom_background",
            ],
            Look::NeoBrutalism => [
                "neo_brutalism_palette",
                "neo_brutalism_custom_palette",
                "neo_brutalism_custom_background",
            ],
        }
    }

    fn as_stored(self) -> &'static str {
        match self {
            Look::Japandi => "japandi",
            Look::PixelArt => "pixel_art",
            Look::NeoBrutalism => "neo_brutalism",
        }
    }

    fn from_stored(text: &str) -> Option<Look> {
        match text {
            "japandi" => Some(Look::Japandi),
            "pixel_art" => Some(Look::PixelArt),
            "neo_brutalism" => Some(Look::NeoBrutalism),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteChoice {
    /// An index below `BUILT_IN_PALETTES`.
    BuiltIn(usize),
    Custom,
}

impl PaletteChoice {
    fn as_stored(self) -> String {
        match self {
            PaletteChoice::BuiltIn(index) => index.to_string(),
            PaletteChoice::Custom => "custom".to_string(),
        }
    }

    fn from_stored(text: &str) -> Option<PaletteChoice> {
        if text == "custom" {
            return Some(PaletteChoice::Custom);
        }
        let index = text
            .parse::<usize>()
            .ok()
            .filter(|&index| index < BUILT_IN_PALETTES)?;
        Some(PaletteChoice::BuiltIn(index))
    }
}

/// The palette choices one look remembers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookAppearance {
    pub palette: PaletteChoice,
    /// The custom slot's colours. Opaque until Phase 9 defines what's inside.
    pub custom_palette: String,
    pub custom_background: Option<PathBuf>,
}

impl Default for LookAppearance {
    fn default() -> Self {
        LookAppearance {
            palette: PaletteChoice::BuiltIn(0),
            custom_palette: String::new(),
            custom_background: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub show_previous: bool,
    pub copy_on_capture: bool,
    pub auto_save: bool,
    pub keep_in_tray: bool,
    pub open_editor_after_capture: bool,
    // `None` means no shortcut is bound. Phase 6.2 parses the text.
    pub region_shortcut: Option<String>,
    pub window_shortcut: Option<String>,
    pub full_screen_shortcut: Option<String>,
    pub delayed_shortcut: Option<String>,
    /// `None` until the app supplies the user's Pictures folder, which core can't know.
    pub save_folder: Option<PathBuf>,
    pub name_pattern: String,
    pub format: SaveFormat,
    pub on_close: OnClose,
    pub retention: Retention,
    /// One of `DELAY_SECONDS`.
    pub delay_seconds: u32,
    pub delay_mode: CaptureMode,
    pub look: Look,
    pub japandi: LookAppearance,
    pub pixel_art: LookAppearance,
    pub neo_brutalism: LookAppearance,
    pub settings_collapsed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_previous: true,
            copy_on_capture: true,
            auto_save: false,
            keep_in_tray: true,
            open_editor_after_capture: false,
            region_shortcut: Some("PrtScn".to_string()),
            window_shortcut: Some("Ctrl+Shift+W".to_string()),
            full_screen_shortcut: Some("Ctrl+Shift+F".to_string()),
            delayed_shortcut: None,
            save_folder: None,
            name_pattern: DEFAULT_PATTERN.to_string(),
            format: SaveFormat::Png,
            on_close: OnClose::Ask,
            retention: Retention::Days(30),
            delay_seconds: 5,
            delay_mode: CaptureMode::Region,
            look: Look::Japandi,
            japandi: LookAppearance::default(),
            pixel_art: LookAppearance::default(),
            neo_brutalism: LookAppearance::default(),
            settings_collapsed: false,
        }
    }
}

impl Settings {
    pub fn appearance(&self, look: Look) -> &LookAppearance {
        match look {
            Look::Japandi => &self.japandi,
            Look::PixelArt => &self.pixel_art,
            Look::NeoBrutalism => &self.neo_brutalism,
        }
    }

    pub fn appearance_mut(&mut self, look: Look) -> &mut LookAppearance {
        match look {
            Look::Japandi => &mut self.japandi,
            Look::PixelArt => &mut self.pixel_art,
            Look::NeoBrutalism => &mut self.neo_brutalism,
        }
    }

    // `None` is stored as an empty value rather than left out, because a missing key means the
    // default and a shortcut the user cleared must not come back as `PrtScn`.
    pub(crate) fn stored_values(&self) -> Result<Vec<(&'static str, String)>> {
        let mut values = vec![
            ("show_previous", self.show_previous.to_string()),
            ("copy_on_capture", self.copy_on_capture.to_string()),
            ("auto_save", self.auto_save.to_string()),
            ("keep_in_tray", self.keep_in_tray.to_string()),
            (
                "open_editor_after_capture",
                self.open_editor_after_capture.to_string(),
            ),
            (
                "region_shortcut",
                self.region_shortcut.clone().unwrap_or_default(),
            ),
            (
                "window_shortcut",
                self.window_shortcut.clone().unwrap_or_default(),
            ),
            (
                "full_screen_shortcut",
                self.full_screen_shortcut.clone().unwrap_or_default(),
            ),
            (
                "delayed_shortcut",
                self.delayed_shortcut.clone().unwrap_or_default(),
            ),
            ("save_folder", path_value(self.save_folder.as_deref())?),
            ("name_pattern", self.name_pattern.clone()),
            ("format", self.format.as_stored().to_string()),
            ("on_close", self.on_close.as_stored().to_string()),
            ("retention", self.retention.as_stored()),
            ("delay_seconds", self.delay_seconds.to_string()),
            ("delay_mode", mode_text(self.delay_mode).to_string()),
            ("look", self.look.as_stored().to_string()),
            ("settings_collapsed", self.settings_collapsed.to_string()),
        ];
        for look in Look::ALL {
            let [palette, colours, background] = look.appearance_keys();
            let appearance = self.appearance(look);
            values.push((palette, appearance.palette.as_stored()));
            values.push((colours, appearance.custom_palette.clone()));
            values.push((
                background,
                path_value(appearance.custom_background.as_deref())?,
            ));
        }
        Ok(values)
    }

    /// Overwrites the setting `key` with `value` if it parses. Unknown keys are ignored.
    pub(crate) fn apply_stored(&mut self, key: &str, value: &str) {
        match key {
            "show_previous" => set(&mut self.show_previous, value.parse().ok()),
            "copy_on_capture" => set(&mut self.copy_on_capture, value.parse().ok()),
            "auto_save" => set(&mut self.auto_save, value.parse().ok()),
            "keep_in_tray" => set(&mut self.keep_in_tray, value.parse().ok()),
            "open_editor_after_capture" => {
                set(&mut self.open_editor_after_capture, value.parse().ok());
            }
            "region_shortcut" => self.region_shortcut = non_empty(value),
            "window_shortcut" => self.window_shortcut = non_empty(value),
            "full_screen_shortcut" => self.full_screen_shortcut = non_empty(value),
            "delayed_shortcut" => self.delayed_shortcut = non_empty(value),
            "save_folder" => self.save_folder = non_empty(value).map(PathBuf::from),
            "name_pattern" => set(
                &mut self.name_pattern,
                validate_pattern(value).is_ok().then(|| value.to_string()),
            ),
            "format" => set(&mut self.format, SaveFormat::from_stored(value)),
            "on_close" => set(&mut self.on_close, OnClose::from_stored(value)),
            "retention" => set(&mut self.retention, Retention::from_stored(value)),
            "delay_seconds" => set(
                &mut self.delay_seconds,
                value
                    .parse::<u32>()
                    .ok()
                    .filter(|seconds| DELAY_SECONDS.contains(seconds)),
            ),
            "delay_mode" => set(&mut self.delay_mode, mode_from_text(value)),
            "look" => set(&mut self.look, Look::from_stored(value)),
            "settings_collapsed" => set(&mut self.settings_collapsed, value.parse().ok()),
            _ => self.apply_stored_appearance(key, value),
        }
    }

    fn apply_stored_appearance(&mut self, key: &str, value: &str) {
        for look in Look::ALL {
            let [palette, colours, background] = look.appearance_keys();
            let appearance = self.appearance_mut(look);
            if key == palette {
                set(&mut appearance.palette, PaletteChoice::from_stored(value));
            } else if key == colours {
                appearance.custom_palette = value.to_string();
            } else if key == background {
                appearance.custom_background = non_empty(value).map(PathBuf::from);
            }
        }
    }
}

fn set<T>(field: &mut T, parsed: Option<T>) {
    if let Some(value) = parsed {
        *field = value;
    }
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn path_value(path: Option<&Path>) -> Result<String> {
    Ok(path.map(path_text).transpose()?.unwrap_or("").to_string())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Differs from the default in every field, so a field that isn't saved fails the round trip.
    pub(crate) fn every_field_changed() -> Settings {
        Settings {
            show_previous: false,
            copy_on_capture: false,
            auto_save: true,
            keep_in_tray: false,
            open_editor_after_capture: true,
            region_shortcut: Some("Ctrl+Alt+R".to_string()),
            window_shortcut: Some("Ctrl+Alt+W".to_string()),
            full_screen_shortcut: Some("Ctrl+Alt+F".to_string()),
            delayed_shortcut: Some("Ctrl+Alt+D".to_string()),
            save_folder: Some(PathBuf::from(r"D:\写真\shots")),
            name_pattern: "{yyyy}-{mm}-{dd} shot".to_string(),
            format: SaveFormat::WebP,
            on_close: OnClose::Discard,
            retention: Retention::Days(45),
            delay_seconds: 10,
            delay_mode: CaptureMode::FullScreen,
            look: Look::NeoBrutalism,
            japandi: LookAppearance {
                palette: PaletteChoice::BuiltIn(1),
                custom_palette: "opaque {data} = ✓\nsecond line".to_string(),
                custom_background: Some(PathBuf::from(r"C:\Users\me\back.jpg")),
            },
            pixel_art: LookAppearance {
                palette: PaletteChoice::BuiltIn(2),
                custom_palette: "pixel colours".to_string(),
                custom_background: Some(PathBuf::from(r"D:\写真\pixel.png")),
            },
            neo_brutalism: LookAppearance {
                palette: PaletteChoice::Custom,
                custom_palette: "neo colours".to_string(),
                custom_background: Some(PathBuf::from(r"E:\neo.webp")),
            },
            settings_collapsed: true,
        }
    }

    fn reloaded(settings: &Settings) -> Settings {
        let mut loaded = Settings::default();
        for (key, value) in settings.stored_values().unwrap() {
            loaded.apply_stored(key, &value);
        }
        loaded
    }

    fn with_stored(key: &str, value: &str) -> Settings {
        let mut settings = Settings::default();
        settings.apply_stored(key, value);
        settings
    }

    #[test]
    fn defaults_match_the_design() {
        let appearance = LookAppearance {
            palette: PaletteChoice::BuiltIn(0),
            custom_palette: String::new(),
            custom_background: None,
        };
        assert_eq!(
            Settings::default(),
            Settings {
                show_previous: true,
                copy_on_capture: true,
                auto_save: false,
                keep_in_tray: true,
                open_editor_after_capture: false,
                region_shortcut: Some("PrtScn".to_string()),
                window_shortcut: Some("Ctrl+Shift+W".to_string()),
                full_screen_shortcut: Some("Ctrl+Shift+F".to_string()),
                delayed_shortcut: None,
                save_folder: None,
                name_pattern: "screenshot_{mm}_{dd}_{yyyy}".to_string(),
                format: SaveFormat::Png,
                on_close: OnClose::Ask,
                retention: Retention::Days(30),
                delay_seconds: 5,
                delay_mode: CaptureMode::Region,
                look: Look::Japandi,
                japandi: appearance.clone(),
                pixel_art: appearance.clone(),
                neo_brutalism: appearance,
                settings_collapsed: false,
            }
        );
    }

    #[test]
    fn the_test_settings_differ_from_the_defaults_everywhere() {
        let changed = every_field_changed().stored_values().unwrap();
        let defaults = Settings::default().stored_values().unwrap();
        assert_eq!(changed.len(), defaults.len());
        for ((key, changed), (_, default)) in changed.iter().zip(&defaults) {
            assert_ne!(changed, default, "{key}");
        }
    }

    #[test]
    fn every_field_round_trips() {
        let settings = every_field_changed();
        assert_eq!(reloaded(&settings), settings);
    }

    #[test]
    fn the_defaults_round_trip() {
        assert_eq!(reloaded(&Settings::default()), Settings::default());
    }

    #[test]
    fn every_key_is_unique() {
        let values = Settings::default().stored_values().unwrap();
        let mut keys: Vec<&str> = values.iter().map(|(key, _)| *key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), values.len());
    }

    #[test]
    fn every_choice_round_trips() {
        for format in [SaveFormat::Png, SaveFormat::Jpg, SaveFormat::WebP] {
            assert_eq!(SaveFormat::from_stored(format.as_stored()), Some(format));
        }
        for on_close in [OnClose::Ask, OnClose::Keep, OnClose::Discard] {
            assert_eq!(OnClose::from_stored(on_close.as_stored()), Some(on_close));
        }
        for look in [Look::Japandi, Look::PixelArt, Look::NeoBrutalism] {
            assert_eq!(Look::from_stored(look.as_stored()), Some(look));
        }
        for palette in [
            PaletteChoice::BuiltIn(0),
            PaletteChoice::BuiltIn(1),
            PaletteChoice::BuiltIn(2),
            PaletteChoice::Custom,
        ] {
            assert_eq!(
                PaletteChoice::from_stored(&palette.as_stored()),
                Some(palette)
            );
        }
        for delay_mode in [
            CaptureMode::Region,
            CaptureMode::Window,
            CaptureMode::FullScreen,
        ] {
            let settings = Settings {
                delay_mode,
                ..Settings::default()
            };
            assert_eq!(reloaded(&settings), settings);
        }
        for delay_seconds in DELAY_SECONDS {
            let settings = Settings {
                delay_seconds,
                ..Settings::default()
            };
            assert_eq!(reloaded(&settings), settings);
        }
    }

    #[test]
    fn a_cleared_shortcut_stays_cleared() {
        let settings = Settings {
            region_shortcut: None,
            window_shortcut: None,
            full_screen_shortcut: None,
            ..Settings::default()
        };
        assert_eq!(reloaded(&settings), settings);
    }

    #[test]
    fn a_cleared_folder_and_background_stay_cleared() {
        let mut settings = Settings {
            save_folder: None,
            ..every_field_changed()
        };
        for look in Look::ALL {
            settings.appearance_mut(look).custom_background = None;
        }
        assert_eq!(reloaded(&settings), settings);
    }

    #[test]
    fn each_look_keeps_its_own_appearance_through_a_round_trip() {
        let appearance_of = |index: usize| LookAppearance {
            palette: PaletteChoice::BuiltIn(index),
            custom_palette: format!("colours {index}"),
            custom_background: Some(PathBuf::from(format!(r"C:\back{index}.jpg"))),
        };
        let mut settings = Settings::default();
        for (index, look) in Look::ALL.into_iter().enumerate() {
            *settings.appearance_mut(look) = appearance_of(index);
        }

        let loaded = reloaded(&settings);

        for (index, look) in Look::ALL.into_iter().enumerate() {
            assert_eq!(*loaded.appearance(look), appearance_of(index), "{look:?}");
        }
    }

    #[test]
    fn a_malformed_palette_affects_only_its_own_look() {
        for look in Look::ALL {
            let [palette_key, ..] = look.appearance_keys();
            let mut loaded = Settings::default();
            for (key, value) in every_field_changed().stored_values().unwrap() {
                let value = if key == palette_key {
                    "huh"
                } else {
                    value.as_str()
                };
                loaded.apply_stored(key, value);
            }

            let mut expected = every_field_changed();
            expected.appearance_mut(look).palette = PaletteChoice::BuiltIn(0);
            assert_eq!(loaded, expected, "{look:?}");
        }
    }

    #[test]
    fn missing_keys_keep_their_defaults() {
        let mut settings = Settings::default();
        settings.apply_stored("auto_save", "true");
        assert_eq!(
            settings,
            Settings {
                auto_save: true,
                ..Settings::default()
            }
        );
    }

    #[test]
    fn malformed_values_keep_the_default() {
        for (key, value) in [
            ("show_previous", "yes"),
            ("copy_on_capture", ""),
            ("auto_save", "1"),
            ("keep_in_tray", "TRUE"),
            ("open_editor_after_capture", "2"),
            ("name_pattern", ""),
            ("name_pattern", "shot_{yy}"),
            ("name_pattern", "a:b"),
            ("name_pattern", "con"),
            ("format", "bmp"),
            ("format", "PNG"),
            ("on_close", "maybe"),
            ("retention", ""),
            ("retention", "soon"),
            ("retention", "0"),
            ("retention", "-1"),
            ("retention", "1.5"),
            ("retention", "4294967296"),
            ("delay_seconds", ""),
            ("delay_seconds", "0"),
            ("delay_seconds", "4"),
            ("delay_seconds", "x"),
            ("delay_mode", "hologram"),
            ("look", "vaporwave"),
            ("japandi_palette", ""),
            ("japandi_palette", "3"),
            ("pixel_art_palette", "-1"),
            ("neo_brutalism_palette", "Custom"),
            ("settings_collapsed", "no"),
        ] {
            assert_eq!(
                with_stored(key, value),
                Settings::default(),
                "{key} = {value:?}"
            );
        }
    }

    #[test]
    fn unknown_keys_are_ignored() {
        assert_eq!(with_stored("from_the_future", "1"), Settings::default());
    }

    #[test]
    fn preset_retention_gives_its_day_count() {
        for days in RETENTION_PRESET_DAYS {
            let retention = Retention::Days(days);
            assert_eq!(retention.days(), Some(days));
            assert!(!retention.is_custom(), "{days}");
            assert_eq!(
                Retention::from_stored(&retention.as_stored()),
                Some(retention)
            );
        }
    }

    #[test]
    fn custom_retention_gives_its_day_count() {
        for days in [1, 8, 45, 365, u32::MAX] {
            let retention = Retention::Days(days);
            assert_eq!(retention.days(), Some(days));
            assert!(retention.is_custom(), "{days}");
            assert_eq!(
                Retention::from_stored(&retention.as_stored()),
                Some(retention)
            );
        }
    }

    #[test]
    fn never_has_no_day_count_and_is_not_custom() {
        assert_eq!(Retention::Never.days(), None);
        assert!(!Retention::Never.is_custom());
        assert_eq!(Retention::from_stored("never"), Some(Retention::Never));
    }
}
