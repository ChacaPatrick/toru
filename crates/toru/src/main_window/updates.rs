//! In-app updates (`docs/PLAN.md` 11.3): Toru looks for a newer release a little after it starts
//! and every few hours after that, downloads its installer, and only then says so, in a notice
//! under the toolbar: "Toru 0.2.0 is ready", with What's new, Later and Update. Nothing is asked
//! of the user before that, and an automatic check that fails says nothing.
//!
//! The release workflow (11.2) publishes a manifest (`toru_core::update`) naming the version, the
//! installer and the patch notes. `platform` fetches both with Windows' own curl, off the main
//! thread. The installer waits in `updates` under the app-data folder, where a later start finds
//! it again without downloading; installers for other versions are removed.
//!
//! Update quits Toru the way the tray's Quit does, so the On close setting has its say, and only
//! when the app really goes does `closing` start the installer, which waits for Toru to exit,
//! installs over it and starts it again. Cancel in the Ask dialog cancels the update too. The
//! new version's first start says "Toru updated to 0.2.0".
//!
//! Only release builds check, unless `TORU_UPDATE_URL` names another manifest, which is how this
//! is tested: with it set, any build checks, and the manifest and installer may be `file://` URLs.
//!
//! The mockups show none of this. The notice is a row in the toolbar's colours, so it covers
//! nothing and the gallery only starts lower; it is left out while the editor is open. What's
//! new is a dialog like the others, its notes drawn as the settings column draws its groups: a
//! label over its lines.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use gpui::{
    AnyElement, Bounds, Context, Div, Role, ScrollHandle, accesskit::Live, canvas, div, fill,
    point, prelude::*, px, size,
};
use gpui_kit::component::scroll::ScrollableElement;
use toru_core::update::{Manifest, NoteLine, Version, note_lines};

use super::{
    HINT_LEADING, MainWindow, act,
    settings::{group, hint, row, toggle},
};
use crate::platform;
use crate::theme::{ActiveTheme, Theme, ThemedText};
use crate::ui::{Button, Dialog, Readable, Toast};

/// Where each release puts its manifest. GitHub redirects it to the newest release's copy.
const MANIFEST_URL: &str =
    "https://github.com/ChacaPatrick/toru/releases/latest/download/toru-update.txt";
/// Names another manifest, for testing.
const URL_VAR: &str = "TORU_UPDATE_URL";
/// The folder under the app-data folder that holds the downloaded installer.
const FOLDER: &str = "updates";

/// Long enough after start-up that the window is up and history has loaded.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(10);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Longer than other toasts: it shows as the new version's window opens.
const UPDATED_TOAST_FOR: Duration = Duration::from_secs(6);

const CHECK_HINT: &str = "New versions download quietly. You choose when to install.";
const NOTICE_HINT: &str = "Updating restarts Toru.";
const UP_TO_DATE: &str = "Toru is up to date";
const CHECK_FAILED: &str = "Couldn't check for updates";
const NOT_CHECKED: &str = "This build doesn't check for updates";
const INSTALLER_GONE: &str = "The update's installer is gone. Check again.";

/// The notice row: the toolbar's padding at the sides, and a small button's height plus this.
const NOTICE_PADDING_X: f32 = 16.;
const NOTICE_PADDING_Y: f32 = 8.;
const NOTICE_GAP: f32 = 10.;
const NOTICE_BUTTON_GAP: f32 = 8.;

/// The notes scroll past this height.
const NOTES_MAX_HEIGHT: f32 = 360.;
const NOTE_GAP: f32 = 6.;
/// More air between the notes' parts than between a part's lines.
const PART_GAP: f32 = 10.;
const BULLET_SIZE: f32 = 5.;
const BULLET_GAP: f32 = 10.;
/// Keeps the notes' text clear of the scrollbar.
const SCROLLBAR_ROOM: f32 = 12.;
/// How far an arrow key scrolls the notes.
const LINE_SCROLL: f32 = 40.;

/// The version this build is.
pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("the package version is x.y.z")
}

/// Where the manifest is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    url: String,
    /// It came from `TORU_UPDATE_URL`, so the installer may be a local file.
    test: bool,
}

/// `named` is `TORU_UPDATE_URL`'s value. Without it only a release build checks.
fn source(named: Option<String>, release: bool) -> Option<Source> {
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
pub(super) struct Update {
    version: Version,
    notes: String,
    installer: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
enum Checked {
    UpToDate,
    Ready(Update),
}

/// Reads the manifest and, if it names a version newer than `current`, downloads its installer
/// into `folder` unless it is there already. The download gets its name only once it is whole.
/// Blocks, so it runs off the main thread.
fn check(source: &Source, current: Version, folder: &Path) -> io::Result<Checked> {
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

/// How far a press of Update has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Install {
    No,
    /// Update asked Toru to quit, and the On close setting may yet ask the user, who may cancel.
    Asked,
    /// Toru is quitting for it, so the installer starts as it goes.
    OnExit,
}

/// What the main window keeps about updates.
pub(super) struct Updates {
    /// `None` in a build that doesn't check.
    source: Option<Source>,
    ready: Option<Update>,
    /// Later hides the notice until a newer version turns up, a Check now, or the next start.
    dismissed: bool,
    /// The What's new dialog is open.
    whats_new: bool,
    notes_scroll: ScrollHandle,
    checking: bool,
    /// Check now is waiting for the check under way, so its result gets a toast.
    asked: bool,
    install: Install,
    /// This start is the first of a newer version, which a toast says once the window is seen.
    updated_to: Option<Version>,
}

impl Updates {
    pub fn new() -> Updates {
        Updates {
            source: source(std::env::var(URL_VAR).ok(), !cfg!(debug_assertions)),
            ready: None,
            dismissed: false,
            whats_new: false,
            notes_scroll: ScrollHandle::new(),
            checking: false,
            asked: false,
            install: Install::No,
            updated_to: None,
        }
    }

    pub fn whats_new_open(&self) -> bool {
        self.whats_new
    }

    /// The dialog goes when another takes its place, as the quit question does.
    pub fn close_whats_new(&mut self) {
        self.whats_new = false;
    }

    /// Toru is quitting for good. If Update asked for it, the installer starts on the way out.
    pub fn leaving(&mut self) {
        if self.install == Install::Asked {
            self.install = Install::OnExit;
        }
    }

    /// The quit was cancelled, so nothing is installed and the notice stays.
    pub fn quit_cancelled(&mut self) {
        self.install = Install::No;
    }

    /// As the app exits: starts the installer if this exit is for the update. It waits for this
    /// process to end before it touches anything. If it can't start, Toru just quits, and the
    /// next start offers the update again.
    pub fn install_on_exit(&self) {
        if self.install == Install::OnExit
            && let Some(update) = &self.ready
        {
            platform::start_installer(&update.installer).ok();
        }
    }
}

impl MainWindow {
    /// At start-up: notes the version now running, and starts the checks where this build makes
    /// them. The setting is read at each check, so turning it off or on needs no restart.
    pub(super) fn start_updates(&mut self, cx: &mut Context<Self>) {
        let current = current_version();
        let last = self.settings.last_version;
        if last != Some(current) {
            self.change_settings(cx, |settings| settings.last_version = Some(current));
            self.updates.updated_to = last.filter(|last| current > *last).map(|_| current);
            self.show_updated(cx);
        }
        if self.updates.source.is_none() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let mut wait = FIRST_CHECK_AFTER;
            loop {
                cx.background_executor().timer(wait).await;
                wait = CHECK_EVERY;
                let checked = this.update(cx, |this, cx| {
                    if this.settings.check_for_updates {
                        this.check_for_updates(false, cx);
                    }
                });
                if checked.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// "Toru updated to 0.2.0", once, when the window is there to show it in.
    pub(super) fn show_updated(&mut self, cx: &mut Context<Self>) {
        if !self.window_active {
            return;
        }
        if let Some(version) = self.updates.updated_to.take() {
            let toast = Toast::new(format!("Toru updated to {version}"));
            self.toast
                .show(toast, UPDATED_TOAST_FOR, cx, |this| &mut this.toast);
        }
    }

    /// Looks for a newer version and downloads it. `asked` is Check now, which gets a toast
    /// whatever the result; pressed while a check is already under way, it gets that one's.
    pub(super) fn check_for_updates(&mut self, asked: bool, cx: &mut Context<Self>) {
        let Some(source) = self.updates.source.clone() else {
            if asked {
                self.show_toast(Toast::failed(NOT_CHECKED), cx);
            }
            return;
        };
        self.updates.asked |= asked;
        cx.notify();
        if self.updates.checking {
            return;
        }
        self.updates.checking = true;
        let folder = platform::app_data_dir().join(FOLDER);
        cx.spawn(async move |this, cx| {
            let checked = cx
                .background_spawn(async move { check(&source, current_version(), &folder) })
                .await;
            this.update(cx, |this, cx| this.checked(checked, cx)).ok();
        })
        .detach();
    }

    fn checked(&mut self, checked: io::Result<Checked>, cx: &mut Context<Self>) {
        self.updates.checking = false;
        let asked = std::mem::take(&mut self.updates.asked);
        match checked {
            Ok(Checked::Ready(update)) => {
                let ready = self.updates.ready.as_ref();
                if asked || ready.is_none_or(|ready| update.version > ready.version) {
                    self.updates.dismissed = false;
                }
                if asked {
                    self.show_toast(Toast::new(format!("Toru {} is ready", update.version)), cx);
                }
                self.updates.ready = Some(update);
            }
            Ok(Checked::UpToDate) if asked => self.show_toast(Toast::new(UP_TO_DATE), cx),
            Err(_) if asked => self.report_failure(CHECK_FAILED, cx),
            Ok(Checked::UpToDate) | Err(_) => {}
        }
        cx.notify();
    }

    /// Later, on the notice or in the dialog.
    fn dismiss_update(&mut self, cx: &mut Context<Self>) {
        self.updates.dismissed = true;
        self.updates.whats_new = false;
        cx.notify();
    }

    fn open_whats_new(&mut self, cx: &mut Context<Self>) {
        self.updates.whats_new = true;
        self.updates.notes_scroll.set_offset(point(px(0.), px(0.)));
        self.focus_dialog(cx);
        cx.notify();
    }

    fn close_whats_new(&mut self, cx: &mut Context<Self>) {
        self.updates.whats_new = false;
        cx.notify();
    }

    /// Update: quits as Quit does, and `closing` starts the installer if the app really exits.
    fn install_update(&mut self, cx: &mut Context<Self>) {
        self.updates.whats_new = false;
        cx.notify();
        if !self
            .updates
            .ready
            .as_ref()
            .is_some_and(|update| update.installer.is_file())
        {
            // Removed behind Toru's back: nothing to offer until the next check finds it again.
            self.updates.ready = None;
            self.show_toast(Toast::failed(INSTALLER_GONE), cx);
            return;
        }
        self.updates.install = Install::Asked;
        self.request_quit(cx);
    }

    /// The What's new dialog: the version, its notes, and Update, which holds focus, or Later.
    /// Esc only closes it. The arrow and page keys scroll the notes wherever focus is, since
    /// the buttons have no use for them.
    pub(super) fn whats_new_dialog(&self, cx: &Context<Self>) -> Option<Dialog> {
        let update = self
            .updates
            .ready
            .as_ref()
            .filter(|_| self.updates.whats_new)?;
        let lines = note_lines(&update.notes);
        let dialog = Dialog::new(
            "whats-new",
            format!("Toru {}", update.version),
            if lines.is_empty() {
                NOTICE_HINT.to_string()
            } else {
                format!("Here's what's new. {NOTICE_HINT}")
            },
            &self.dialog_focus,
            act(cx, |this, cx| this.close_whats_new(cx)),
        )
        .primary_action("Update", act(cx, |this, cx| this.install_update(cx)))
        .action("Later", act(cx, |this, cx| this.dismiss_update(cx)));
        if lines.is_empty() {
            return Some(dialog);
        }
        let scroll = self.updates.notes_scroll.clone();
        Some(
            dialog
                .content(notes(cx.theme(), &lines, &scroll), Vec::new())
                .on_key(move |event, window, _| {
                    let Some(to) = scroll_target(
                        &event.keystroke.key,
                        scroll.offset().y.into(),
                        scroll.max_offset().y.into(),
                        scroll.bounds().size.height.into(),
                    ) else {
                        return false;
                    };
                    scroll.set_offset(point(px(0.), px(to)));
                    window.refresh();
                    true
                }),
        )
    }
}

/// Where a key scrolls the notes to: the new offset, zero at the top and `-max` at the bottom,
/// for a view `page` tall. `None` for a key that doesn't scroll.
fn scroll_target(key: &str, offset: f32, max: f32, page: f32) -> Option<f32> {
    let by = match key {
        "up" => LINE_SCROLL,
        "down" => -LINE_SCROLL,
        "pageup" => page - LINE_SCROLL,
        "pagedown" => LINE_SCROLL - page,
        "home" => f32::INFINITY,
        "end" => f32::NEG_INFINITY,
        _ => return None,
    };
    Some((offset + by).clamp(-max.max(0.), 0.))
}

/// The patch notes: each heading as a settings group's label, each bullet behind the look's dot.
fn notes(theme: &Theme, lines: &[NoteLine], scroll: &ScrollHandle) -> impl IntoElement {
    let c = &theme.colors;
    let body = theme.type_scale.body;
    let label = theme.type_scale.label;
    let leading = body.size * HINT_LEADING;
    let rows = lines.iter().enumerate().map(|(index, line)| -> AnyElement {
        // A new part of the notes: a heading, or plain text once a list has ended.
        let parts = match line {
            NoteLine::Heading(_) => index > 0,
            NoteLine::Text(_) => index > 0 && matches!(lines[index - 1], NoteLine::Bullet(_)),
            NoteLine::Bullet(_) => false,
        };
        let row = div()
            .id(("note", index))
            .when(parts, |row| row.mt(px(PART_GAP)));
        match *line {
            NoteLine::Heading(text) => row
                .read_as_heading(text)
                .type_style(theme, label)
                .text_color(c.label)
                .child(label.case.apply(text))
                .into_any_element(),
            NoteLine::Bullet(text) => row
                .read_as_text(text)
                .flex()
                .gap(px(BULLET_GAP))
                // As tall as a line, so the dot sits beside the first one.
                .child(
                    div().flex_none().h(leading).flex().items_center().child(
                        div()
                            .size(px(BULLET_SIZE))
                            .rounded(theme.radii.dot)
                            .bg(c.label),
                    ),
                )
                .child(div().flex_1().min_w_0().child(text.to_string()))
                .into_any_element(),
            NoteLine::Text(text) => row
                .read_as_text(text)
                .child(text.to_string())
                .into_any_element(),
        }
    });
    // A rule across the top or bottom edge while there are notes past it, since the scrollbar
    // only shows while it is used. Painted after the list is laid out, so it is right from the
    // first frame.
    let hair = theme.borders.hair;
    let more = canvas(|_, _, _| (), {
        let scroll = scroll.clone();
        move |bounds, (), window, _| {
            let rule = size(bounds.size.width, hair.width);
            // Less than a pixel is the rounding of lines that fit, not notes out of view.
            let (offset, hidden) = (scroll.offset().y, px(1.));
            if offset < -hidden {
                window.paint_quad(fill(Bounds::new(bounds.origin, rule), hair.color));
            }
            if offset > hidden - scroll.max_offset().y {
                let origin = point(bounds.left(), bounds.bottom() - hair.width);
                window.paint_quad(fill(Bounds::new(origin, rule), hair.color));
            }
        }
    })
    .absolute()
    .inset_0();
    // In a short window the notes give way and scroll, and the actions stay in view.
    div()
        .relative()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            div()
                .id("whats-new-notes")
                .min_h_0()
                .max_h(px(NOTES_MAX_HEIGHT))
                .overflow_y_scroll()
                .track_scroll(scroll)
                .flex()
                .flex_col()
                .gap(px(NOTE_GAP))
                .pr(px(SCROLLBAR_ROOM))
                .type_style(theme, body)
                .line_height(leading)
                .children(rows),
        )
        .child(more)
        .vertical_scrollbar(scroll)
}

/// The notice, while an update is downloaded and not put off: a row under the toolbar, in its
/// colours, with the look's dot in the accent. Announced like a toast when it appears.
pub(super) fn notice(this: &MainWindow, cx: &Context<MainWindow>) -> Option<Div> {
    let update = this
        .updates
        .ready
        .as_ref()
        .filter(|_| !this.updates.dismissed)?;
    let theme = cx.theme();
    let c = &theme.colors;
    let hair = theme.borders.hair;
    let message = format!("Toru {} is ready", update.version);
    Some(
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(NOTICE_GAP))
            .px(px(NOTICE_PADDING_X))
            .py(px(NOTICE_PADDING_Y))
            .bg(c.surface_2)
            .border_b(hair.width)
            .border_color(hair.color)
            .child(
                div()
                    .flex_none()
                    .size(theme.icons.dot)
                    .rounded(theme.radii.dot)
                    .bg(c.accent),
            )
            .child(
                div()
                    .id("update-ready")
                    .role(Role::Status)
                    .aria_label(format!("{message}. {NOTICE_HINT}"))
                    .a11y_synthetic_children(|builder| {
                        builder.parent_node().set_live(Live::Polite);
                    })
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_baseline()
                    .gap(px(NOTICE_GAP))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .type_style(theme, theme.type_scale.setting)
                            .child(message),
                    )
                    .child(
                        hint(theme, NOTICE_HINT)
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis(),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap(px(NOTICE_BUTTON_GAP))
                    .child(
                        Button::new("whats-new", "What's new")
                            .small()
                            .on_click(act(cx, |this, cx| this.open_whats_new(cx))),
                    )
                    .child(
                        Button::new("update-later", "Later")
                            .named("Update later")
                            .small()
                            .on_click(act(cx, |this, cx| this.dismiss_update(cx))),
                    )
                    .child(
                        Button::new("update-now", "Update")
                            .named(format!("Update to Toru {}", update.version))
                            .small()
                            .primary()
                            .on_click(act(cx, |this, cx| this.install_update(cx))),
                    ),
            ),
    )
}

/// The settings column's Updates group: the setting, then this version with Check now.
pub(super) fn settings_group(this: &MainWindow, cx: &Context<MainWindow>) -> Div {
    let theme = cx.theme();
    let version = format!("Version {}", current_version());
    group(theme, "Updates")
        .child(toggle(
            cx,
            "check-for-updates",
            "Check for updates",
            this.settings.check_for_updates,
            Some(CHECK_HINT),
            |settings, on| settings.check_for_updates = on,
        ))
        .child(hint(theme, CHECK_HINT).mt(px(-4.)))
        .child(
            row(theme)
                .child(
                    div()
                        .id("version")
                        .read_as_text(&version)
                        .type_style(theme, theme.type_scale.setting)
                        .child(version),
                )
                .child(
                    Button::new(
                        "check-now",
                        if this.updates.asked {
                            "Checking…"
                        } else {
                            "Check now"
                        },
                    )
                    .named("Check for updates now")
                    .small()
                    .disabled(this.updates.asked)
                    .on_click(act(cx, |this, cx| this.check_for_updates(true, cx))),
                ),
        )
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

    #[test]
    fn update_only_installs_once_the_quit_is_final() {
        let mut updates = Updates::new();
        // A quit nobody pressed Update for installs nothing.
        updates.leaving();
        assert_eq!(updates.install, Install::No);

        updates.install = Install::Asked;
        updates.quit_cancelled();
        updates.leaving();
        assert_eq!(updates.install, Install::No);

        updates.install = Install::Asked;
        updates.leaving();
        assert_eq!(updates.install, Install::OnExit);
    }

    #[test]
    fn keys_scroll_the_notes_within_their_ends() {
        let (max, page) = (300., 280.);
        assert_eq!(scroll_target("down", 0., max, page), Some(-40.));
        assert_eq!(scroll_target("up", 0., max, page), Some(0.));
        assert_eq!(scroll_target("pagedown", -40., max, page), Some(-280.));
        assert_eq!(scroll_target("pagedown", -280., max, page), Some(-300.));
        assert_eq!(scroll_target("pageup", -300., max, page), Some(-60.));
        assert_eq!(scroll_target("end", 0., max, page), Some(-300.));
        assert_eq!(scroll_target("home", -300., max, page), Some(0.));
        // Notes that fit don't move.
        assert_eq!(scroll_target("down", 0., 0., page), Some(0.));
        assert_eq!(scroll_target("enter", 0., max, page), None);
        assert_eq!(scroll_target("tab", 0., max, page), None);
    }

    #[cfg(windows)]
    mod with_curl {
        use super::super::super::captures::tests::TestDir;
        use super::*;

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
