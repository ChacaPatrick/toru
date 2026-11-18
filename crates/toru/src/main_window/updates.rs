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

mod check;
mod notice;
mod whats_new;

use std::{io, time::Duration};

use gpui::{Context, Div, ScrollHandle, div, prelude::*, px};
use toru_core::update::Version;

use super::{
    MainWindow, TOAST_FOR, act,
    settings::{group, hint, row, toggle},
};
use crate::platform;
use crate::theme::{ActiveTheme, ThemedText};
use crate::ui::{Button, Readable, Toast};
use check::{Checked, Source, Update, check, current_version, source};
pub(super) use notice::notice;

/// Names another manifest, for testing.
const URL_VAR: &str = "TORU_UPDATE_URL";
/// The folder under the app-data folder that holds the downloaded installer.
const FOLDER: &str = "updates";

/// Long enough after start-up that the window is up and history has loaded.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(10);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Longer than other toasts: it shows as the new version's window opens.
const UPDATED_TOAST_FOR: Duration = Duration::from_secs(6);
/// How long after start-up it shows, so that start-up's own toasts, which are put up as the
/// window opens, are there to be waited for.
const UPDATED_AFTER: Duration = Duration::from_secs(1);

const CHECK_HINT: &str = "New versions download quietly. You choose when to install.";
const NOTICE_HINT: &str = "Updating restarts Toru.";
const UP_TO_DATE: &str = "Toru is up to date";
const CHECK_FAILED: &str = "Couldn't check for updates";
const NOT_CHECKED: &str = "This build doesn't check for updates";
const INSTALLER_GONE: &str = "The update's installer is gone. Check again.";

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
    /// At start-up: notes the version now running, or has an update say so first, and starts the
    /// checks where this build makes them. The setting is read at each check, so turning it off or on needs no restart.
    pub(super) fn start_updates(&mut self, cx: &mut Context<Self>) {
        let current = current_version();
        match self.settings.last_version {
            Some(last) if last == current => {}
            // An update. The version is noted when its toast shows, so a start nobody looked
            // at, in the tray from sign-in to shutdown, doesn't use the toast up.
            Some(last) if last < current => {
                self.updates.updated_to = Some(current);
                self.show_updated_after(UPDATED_AFTER, cx);
            }
            _ => self.change_settings(cx, |settings| settings.last_version = Some(current)),
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

    /// "Toru updated to 0.2.0", once, when the window is there to show it in: a window that
    /// isn't active comes back here when it is. There is one toast at a time, so while another
    /// is up, as start-up's own may be (a shortcut that isn't working, no tray icon), this one
    /// tries again when that one has had its time rather than replace it.
    pub(super) fn show_updated(&mut self, cx: &mut Context<Self>) {
        if self.updates.updated_to.is_none() || !self.window_active {
            return;
        }
        if self.toast.get().is_some() {
            self.show_updated_after(TOAST_FOR, cx);
            return;
        }
        if let Some(version) = self.updates.updated_to.take() {
            let toast = Toast::new(format!("Toru updated to {version}"));
            self.toast
                .show(toast, UPDATED_TOAST_FOR, cx, |this| &mut this.toast);
            self.change_settings(cx, |settings| settings.last_version = Some(version));
        }
    }

    fn show_updated_after(&self, wait: Duration, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update(cx, |this, cx| this.show_updated(cx)).ok();
        })
        .detach();
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
}
