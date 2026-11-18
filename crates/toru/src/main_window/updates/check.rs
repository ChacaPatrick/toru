use std::{
    fs, io,
    path::{Path, PathBuf},
};

use toru_core::update::{Manifest, Version};

use crate::platform;

/// Where each release puts its manifest. GitHub redirects it to the newest release's copy.
const MANIFEST_URL: &str =
    "https://github.com/ChacaPatrick/toru/releases/latest/download/toru-update.txt";

/// The version this build is.
pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("the package version is x.y.z")
}

/// Where the manifest is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Source {
    url: String,
    /// It came from `TORU_UPDATE_URL`, so the installer may be a local file.
    test: bool,
}

/// `named` is `TORU_UPDATE_URL`'s value. Without it only a release build checks.
pub(super) fn source(named: Option<String>, release: bool) -> Option<Source> {
    match named.filter(|url| !url.is_empty()) {
        Some(url) => Some(Source { url, test: true }),
        None => release.then(|| Source {
            url: MANIFEST_URL.to_string(),
            test: false,
        }),
    }
}

/// Whether an installer may be fetched from `url`: only over HTTPS, or from a file in a test.
fn installer_allowed(url: &str, test: bool) -> bool {
    url.starts_with("https://") || (test && url.starts_with("file://"))
}

fn installer_name(version: Version) -> String {
    format!("Toru-{version}-setup.exe")
}

/// An update whose installer is downloaded and waiting.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::main_window) struct Update {
    pub(super) version: Version,
    pub(super) notes: String,
    pub(super) installer: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Checked {
    UpToDate,
    Ready(Update),
}

/// Reads the manifest and, if it names a version newer than `current`, downloads its installer
/// into `folder` unless it is there already. The download gets its name only once it is whole.
/// Blocks, so it runs off the main thread.
pub(super) fn check(source: &Source, current: Version, folder: &Path) -> io::Result<Checked> {
    let manifest = Manifest::parse(&platform::fetch_text(&source.url)?)
        .ok_or_else(|| io::Error::other("not an update manifest"))?;
    if manifest.version <= current {
        keep_only(folder, None);
        return Ok(Checked::UpToDate);
    }
    if !installer_allowed(&manifest.installer, source.test) {
        return Err(io::Error::other("the installer isn't served over HTTPS"));
    }
    let installer = folder.join(installer_name(manifest.version));
    if !installer.is_file() {
        fs::create_dir_all(folder)?;
        let partial = installer.with_extension("part");
        platform::download(&manifest.installer, &partial)?;
        fs::rename(&partial, &installer)?;
    }
    keep_only(folder, Some(&installer));
    Ok(Checked::Ready(Update {
        version: manifest.version,
        notes: manifest.notes,
        installer,
    }))
}

/// Removes every file in the updates folder but `keep`: installers for other versions, the one
/// just installed among them, and downloads cut short. One still in use stays until next time.
fn keep_only(folder: &Path, keep: Option<&Path>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if Some(path.as_path()) != keep {
            fs::remove_file(path).ok();
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn this_build_has_a_version_to_compare() {
        assert_eq!(current_version().to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn only_release_builds_check_unless_a_manifest_is_named() {
        assert_eq!(source(None, false), None);
        assert_eq!(source(Some(String::new()), false), None);
        assert_eq!(
            source(None, true),
            Some(Source {
                url: MANIFEST_URL.to_string(),
                test: false
            })
        );
        for release in [false, true] {
            assert_eq!(
                source(Some("file:///C:/t/toru-update.txt".to_string()), release),
                Some(Source {
                    url: "file:///C:/t/toru-update.txt".to_string(),
                    test: true
                })
            );
        }
    }

    #[test]
    fn the_manifest_is_the_latest_releases() {
        assert_eq!(
            MANIFEST_URL,
            "https://github.com/ChacaPatrick/toru/releases/latest/download/toru-update.txt"
        );
    }

    #[test]
    fn an_installer_comes_over_https_or_from_a_file_in_a_test() {
        let release = "https://github.com/ChacaPatrick/toru/releases/download/v0.2.0/setup.exe";
        assert!(installer_allowed(release, false));
        assert!(installer_allowed(release, true));
        assert!(installer_allowed("file:///C:/t/setup.exe", true));
        for url in [
            "file:///C:/t/setup.exe",
            "http://example.com/setup.exe",
            "ftp://example.com/setup.exe",
            "-o C:/t/x",
            "",
        ] {
            assert!(!installer_allowed(url, false), "{url:?}");
        }
        assert!(!installer_allowed("http://example.com/setup.exe", true));
    }

    #[test]
    fn the_installer_is_named_for_its_version() {
        assert_eq!(installer_name(Version([0, 2, 0])), "Toru-0.2.0-setup.exe");
    }

    #[cfg(windows)]
    mod with_curl {
        use super::*;
        use crate::main_window::captures::tests::TestDir;
        use crate::main_window::updates::FOLDER;

        fn file_url(path: &Path) -> String {
            let path = path.display().to_string().replace('\\', "/");
            format!("file:///{}", path.replace(' ', "%20"))
        }

        /// A manifest for `version` in `dir`, whose installer is a file beside it.
        fn publish(dir: &Path, version: &str) -> Source {
            let installer = dir.join("published-setup.exe");
            fs::write(&installer, format!("setup {version}")).unwrap();
            let manifest = dir.join("toru-update.txt");
            let text = format!(
                "version: {version}\r\ninstaller: {}\r\n\r\n## New\r\n- A thing\r\n",
                file_url(&installer)
            );
            fs::write(&manifest, text).unwrap();
            Source {
                url: file_url(&manifest),
                test: true,
            }
        }

        fn names(folder: &Path) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(folder)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }

        #[test]
        fn a_newer_version_is_downloaded_and_older_installers_go() {
            let dir = TestDir::new("update-newer");
            let folder = dir.0.join(FOLDER);
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("Toru-0.1.5-setup.exe"), "old").unwrap();
            fs::write(folder.join("Toru-0.2.0-setup.part"), "cut short").unwrap();
            let source = publish(&dir.0, "0.2.0");

            let checked = check(&source, Version([0, 1, 9]), &folder).unwrap();

            let installer = folder.join("Toru-0.2.0-setup.exe");
            assert_eq!(
                checked,
                Checked::Ready(Update {
                    version: Version([0, 2, 0]),
                    notes: "## New\n- A thing".to_string(),
                    installer: installer.clone(),
                })
            );
            assert_eq!(fs::read_to_string(&installer).unwrap(), "setup 0.2.0");
            assert_eq!(names(&folder), ["Toru-0.2.0-setup.exe"]);

            // The next check finds it there and leaves it be.
            fs::write(&installer, "already here").unwrap();
            assert!(matches!(
                check(&source, Version([0, 1, 9]), &folder),
                Ok(Checked::Ready(_))
            ));
            assert_eq!(fs::read_to_string(&installer).unwrap(), "already here");
        }

        #[test]
        fn the_same_or_an_older_version_is_up_to_date_and_clears_the_folder() {
            let dir = TestDir::new("update-same");
            let folder = dir.0.join(FOLDER);
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("Toru-0.2.0-setup.exe"), "installed").unwrap();
            let source = publish(&dir.0, "0.2.0");

            for current in [Version([0, 2, 0]), Version([0, 10, 0])] {
                assert_eq!(check(&source, current, &folder).unwrap(), Checked::UpToDate);
            }
            assert_eq!(names(&folder), Vec::<String>::new());
        }

        #[test]
        fn a_check_that_cannot_run_is_an_error() {
            let dir = TestDir::new("update-failed");
            fs::create_dir_all(&dir.0).unwrap();
            let folder = dir.0.join(FOLDER);
            let missing = Source {
                url: file_url(&dir.0.join("no-such-manifest.txt")),
                test: true,
            };
            assert!(check(&missing, Version([0, 1, 0]), &folder).is_err());

            // Not a manifest at all, as a web page in its place would be.
            let page = dir.0.join("page.txt");
            fs::write(&page, "<html>Not Found</html>").unwrap();
            let not_one = Source {
                url: file_url(&page),
                test: true,
            };
            assert!(check(&not_one, Version([0, 1, 0]), &folder).is_err());

            // A real release's installer is never a local file.
            let mut source = publish(&dir.0, "0.2.0");
            source.test = false;
            assert!(check(&source, Version([0, 1, 0]), &folder).is_err());
            assert!(!folder.exists());
        }
    }
}
