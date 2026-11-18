use std::{
    collections::HashMap,
    sync::mpsc::{SendError, Sender},
};

use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use super::{Binding, Planned, Problem, Shortcut};

/// Creating it opens the hidden window that receives the presses, on GPUI's main thread, whose
/// message loop serves it (`docs/spikes.md` §1.1). Each press is sent through `sender` as the
/// pressed shortcut's id, since the handler runs where GPUI can't be touched.
pub struct Hotkeys {
    manager: Option<GlobalHotKeyManager>,
    /// By the id a press carries.
    registered: HashMap<u32, (Shortcut, Binding)>,
    /// Held only while a shortcut is being recorded (`listen`).
    listening: HashMap<u32, Shortcut>,
}

impl Hotkeys {
    pub fn new<T: Send + 'static>(
        sender: Sender<T>,
        pressed: impl Fn(u32) -> T + Send + Sync + 'static,
    ) -> Hotkeys {
        let manager = match GlobalHotKeyManager::new() {
            Ok(manager) => Some(manager),
            Err(error) => {
                eprintln!("Toru couldn't set up global shortcuts: {error}");
                None
            }
        };
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state() == HotKeyState::Pressed {
                let _: Result<(), SendError<T>> = sender.send(pressed(event.id()));
            }
        }));
        Hotkeys {
            manager,
            registered: HashMap::new(),
            listening: HashMap::new(),
        }
    }

    /// Replaces whatever was registered before, and notes on each row what Windows refused.
    pub fn apply(&mut self, planned: &mut [Planned]) {
        self.suspend();
        let Some(manager) = &self.manager else {
            for row in planned.iter_mut().filter(|row| row.shortcut.is_some()) {
                row.problem
                    .get_or_insert(Problem::Failed("Shortcuts aren't available.".into()));
            }
            return;
        };
        for row in planned.iter_mut() {
            let Some(shortcut) = row.shortcut.filter(|_| row.problem.is_none()) else {
                continue;
            };
            let hotkey = shortcut.hotkey();
            // A display plugged in while recording may want keys held for the recording.
            if self.listening.remove(&hotkey.id()).is_some() {
                manager.unregister(hotkey).ok();
            }
            match manager.register(hotkey) {
                Ok(()) => {
                    self.registered
                        .insert(hotkey.id(), (shortcut, row.binding.clone()));
                }
                Err(error) => row.problem = Some(registration_problem(error)),
            }
        }
    }

    /// Lets every shortcut go, so the keys reach other apps. `apply` brings them back.
    pub fn suspend(&mut self) {
        if let Some(manager) = &self.manager {
            for (shortcut, _) in self.registered.values() {
                manager.unregister(shortcut.hotkey()).ok();
            }
        }
        self.registered.clear();
    }

    /// Holds `shortcuts` too, until `stop_listening`, so that while a shortcut is being recorded
    /// their presses reach `listening` rather than Windows. One a row already has, or another
    /// app holds, is left as it is.
    pub fn listen(&mut self, shortcuts: impl Iterator<Item = Shortcut>) {
        self.stop_listening();
        let Some(manager) = &self.manager else {
            return;
        };
        for shortcut in shortcuts {
            let hotkey = shortcut.hotkey();
            if !self.registered.contains_key(&hotkey.id()) && manager.register(hotkey).is_ok() {
                self.listening.insert(hotkey.id(), shortcut);
            }
        }
    }

    pub fn stop_listening(&mut self) {
        if let Some(manager) = &self.manager {
            for shortcut in self.listening.values() {
                manager.unregister(shortcut.hotkey()).ok();
            }
        }
        self.listening.clear();
    }

    pub fn binding(&self, id: u32) -> Option<&Binding> {
        self.registered.get(&id).map(|(_, binding)| binding)
    }

    /// The keys pressed, whether a row's or one held for recording (`listen`), so that while a
    /// shortcut is being recorded a press of any of them is recorded rather than acted on.
    pub fn pressed(&self, id: u32) -> Option<Shortcut> {
        self.listening
            .get(&id)
            .copied()
            .or_else(|| self.registered.get(&id).map(|(shortcut, _)| *shortcut))
    }
}

/// `AlreadyRegistered` is the one failure the user can do something about.
fn registration_problem(error: global_hotkey::Error) -> Problem {
    match error {
        global_hotkey::Error::AlreadyRegistered(_) => Problem::TakenByAnotherApp,
        other => Problem::Failed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::shortcuts::bindings::tests::entry;
    use crate::shortcuts::shortcut::tests::shortcut;
    use crate::shortcuts::{Binding, Problem, plan};

    /// Ctrl+Alt+Shift+Win+F24 is a combination nobody uses, held for the moment the test takes.
    #[test]
    #[cfg(windows)]
    fn a_combination_another_registry_holds_is_taken_by_another_app() {
        let unusual = shortcut("Ctrl+Alt+Shift+Win+F24");
        let (sender, _receiver) = mpsc::channel::<u32>();
        let mut first = Hotkeys::new(sender.clone(), |id| id);
        let mut second = Hotkeys::new(sender, |id| id);
        assert!(first.manager.is_some() && second.manager.is_some());
        let mut rows = plan(vec![entry(Binding::Delayed, "Ctrl+Alt+Shift+Win+F24")]);

        first.apply(&mut rows);
        assert_eq!(rows[0].problem, None);
        assert_eq!(
            first.binding(unusual.hotkey().id()),
            Some(&Binding::Delayed)
        );

        let mut again = rows.clone();
        second.apply(&mut again);
        assert_eq!(again[0].problem, Some(Problem::TakenByAnotherApp));

        first.suspend();
        assert_eq!(first.binding(unusual.hotkey().id()), None);
        let mut after = rows.clone();
        second.apply(&mut after);
        assert_eq!(after[0].problem, None);
        second.suspend();
    }

    /// Unusual combinations stand in for PrtScn so the owner's key isn't taken while the test
    /// runs.
    #[test]
    #[cfg(windows)]
    fn listened_keys_are_recorded_not_acted_on_and_let_go_afterwards() {
        let own = shortcut("Ctrl+Alt+Shift+Win+F22");
        let extra = shortcut("Ctrl+Alt+Shift+Win+F23");
        let (sender, _receiver) = mpsc::channel::<u32>();
        let mut hotkeys = Hotkeys::new(sender.clone(), |id| id);
        let mut other = Hotkeys::new(sender, |id| id);
        let mut rows = plan(vec![entry(Binding::Delayed, "Ctrl+Alt+Shift+Win+F22")]);
        hotkeys.apply(&mut rows);
        assert_eq!(rows[0].problem, None);

        hotkeys.listen([own, extra].into_iter());
        assert_eq!(hotkeys.pressed(own.hotkey().id()), Some(own));
        assert_eq!(hotkeys.pressed(extra.hotkey().id()), Some(extra));
        assert_eq!(hotkeys.binding(extra.hotkey().id()), None);
        assert_eq!(
            hotkeys.listening.len(),
            1,
            "the row's own keys aren't held twice"
        );
        let mut taken = plan(vec![entry(Binding::Region, "Ctrl+Alt+Shift+Win+F23")]);
        other.apply(&mut taken);
        assert_eq!(taken[0].problem, Some(Problem::TakenByAnotherApp));

        hotkeys.stop_listening();
        assert_eq!(hotkeys.pressed(extra.hotkey().id()), None);
        assert_eq!(hotkeys.pressed(own.hotkey().id()), Some(own));
        let mut free = plan(vec![entry(Binding::Region, "Ctrl+Alt+Shift+Win+F23")]);
        other.apply(&mut free);
        assert_eq!(free[0].problem, None);
        other.suspend();
        hotkeys.suspend();
    }
}
