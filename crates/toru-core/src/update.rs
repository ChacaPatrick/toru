//! What a release says about itself to a running Toru (`docs/PLAN.md` 11.2 and 11.3): its
//! version, where its installer is, and its patch notes. The release workflow publishes this as
//! `toru-update.txt`, UTF-8 text:
//!
//! ```text
//! version: 0.2.0
//! installer: https://github.com/ChacaPatrick/toru/releases/download/v0.2.0/Toru-0.2.0-setup.exe
//!
//! ## Fixed
//! - A line of the notes
//! ```
//!
//! The header is `key: value` lines up to the first blank line, and everything after it is the
//! notes. Keys this version doesn't know are skipped, so a later release can add some.

use std::fmt;

/// A version as releases number them: `x.y.z`, compared number by number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub [u32; 3]);

impl Version {
    /// Three numbers of digits only, split by dots. Anything else isn't a version.
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.split('.');
        let mut numbers = [0; 3];
        for number in &mut numbers {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            *number = part.parse().ok()?;
        }
        parts.next().is_none().then_some(Version(numbers))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [major, minor, patch] = self.0;
        write!(f, "{major}.{minor}.{patch}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub version: Version,
    /// The installer's URL.
    pub installer: String,
    /// The patch notes, without the blank lines around them. May be empty.
    pub notes: String,
}

impl Manifest {
    /// `None` when the version or the installer is missing or the version isn't one.
    pub fn parse(text: &str) -> Option<Manifest> {
        // Some editors put a byte order mark first, which would hide the first key.
        let mut lines = text.trim_start_matches('\u{feff}').lines();
        let (mut version, mut installer) = (None, None);
        for line in lines.by_ref() {
            if line.trim().is_empty() {
                break;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            match key.trim() {
                "version" => version = Version::parse(value.trim()),
                "installer" => installer = Some(value.trim().to_string()),
                _ => {}
            }
        }
        let notes: Vec<&str> = lines.map(str::trim_end).collect();
        Some(Manifest {
            version: version?,
            installer: installer.filter(|url| !url.is_empty())?,
            notes: notes.join("\n").trim_matches('\n').to_string(),
        })
    }
}

/// A line of the patch notes, as it is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteLine<'a> {
    Heading(&'a str),
    Bullet(&'a str),
    Text(&'a str),
}

/// The notes line by line, without the blank ones: `## ` starts a heading and `- ` a bullet.
pub fn note_lines(notes: &str) -> Vec<NoteLine<'_>> {
    notes
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            if let Some(heading) = line.strip_prefix("## ") {
                NoteLine::Heading(heading.trim_start())
            } else if let Some(bullet) = line.strip_prefix("- ") {
                NoteLine::Bullet(bullet.trim_start())
            } else {
                NoteLine::Text(line)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSTALLER: &str =
        "https://github.com/ChacaPatrick/toru/releases/download/v0.2.0/Toru-0.2.0-setup.exe";

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn a_version_is_three_numbers() {
        assert_eq!(Version::parse("0.2.0"), Some(Version([0, 2, 0])));
        assert_eq!(Version::parse("12.0.345"), Some(Version([12, 0, 345])));
        for text in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "1.2.x",
            "v1.2.3",
            "1.2.3-beta",
            "1..3",
            "+1.2.3",
            "1.2.-3",
            " 1.2.3",
            "1.2.99999999999",
        ] {
            assert_eq!(Version::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_version_reads_as_it_was_written() {
        assert_eq!(version("0.2.0").to_string(), "0.2.0");
        assert_eq!(version("01.002.3").to_string(), "1.2.3");
    }

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert!(version("0.2.0") > version("0.1.0"));
        assert!(version("0.10.0") > version("0.9.9"));
        assert!(version("1.0.0") > version("0.99.99"));
        assert!(version("0.1.10") > version("0.1.9"));
        assert!(version("0.1.0") < version("0.1.1"));
        assert_eq!(version("0.1.0"), version("0.01.0"));
    }

    #[test]
    fn a_manifest_gives_its_version_installer_and_notes() {
        let text =
            format!("version: 0.2.0\ninstaller: {INSTALLER}\n\n## Fixed\n- One thing\n- Another\n");
        assert_eq!(
            Manifest::parse(&text),
            Some(Manifest {
                version: Version([0, 2, 0]),
                installer: INSTALLER.to_string(),
                notes: "## Fixed\n- One thing\n- Another".to_string(),
            })
        );
    }

    #[test]
    fn a_manifest_with_windows_line_endings_reads_the_same() {
        let text = format!(
            "\u{feff}version: 0.2.0\r\ninstaller: {INSTALLER}\r\n\r\n## Fixed\r\n- One thing\r\n"
        );
        let manifest = Manifest::parse(&text).unwrap();
        assert_eq!(manifest.version, Version([0, 2, 0]));
        assert_eq!(manifest.installer, INSTALLER);
        assert_eq!(manifest.notes, "## Fixed\n- One thing");
    }

    #[test]
    fn unknown_keys_and_odd_spacing_are_fine() {
        let text =
            format!("sha256: abc\nversion:0.2.0\nnot a key\n  installer :  {INSTALLER}  \nsize: 1");
        let manifest = Manifest::parse(&text).unwrap();
        assert_eq!(manifest.version, Version([0, 2, 0]));
        assert_eq!(manifest.installer, INSTALLER);
        assert_eq!(manifest.notes, "");
    }

    #[test]
    fn a_manifest_missing_a_key_is_no_manifest() {
        for text in [
            "",
            "version: 0.2.0",
            "installer: https://example.com/setup.exe",
            "version: 0.2.0\ninstaller:",
            "version: soon\ninstaller: https://example.com/setup.exe",
            // Past the blank line it is the notes, not the header.
            "version: 0.2.0\n\ninstaller: https://example.com/setup.exe",
            "<html>Not Found</html>",
        ] {
            assert_eq!(Manifest::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn the_notes_keep_their_inner_blank_lines_and_colons() {
        let text = "version: 0.2.0\ninstaller: x\n\n\n## New\n- Shortcut: Ctrl+U\n\nThanks!\n\n";
        assert_eq!(
            Manifest::parse(text).unwrap().notes,
            "## New\n- Shortcut: Ctrl+U\n\nThanks!"
        );
    }

    #[test]
    fn notes_are_headings_bullets_and_plain_lines() {
        assert_eq!(
            note_lines("## New\n- First\n\n  - Indented\nJust text\n##Not a heading\n-5 degrees"),
            vec![
                NoteLine::Heading("New"),
                NoteLine::Bullet("First"),
                NoteLine::Bullet("Indented"),
                NoteLine::Text("Just text"),
                NoteLine::Text("##Not a heading"),
                NoteLine::Text("-5 degrees"),
            ]
        );
        assert_eq!(note_lines(""), Vec::new());
    }
}
