# Spike results

## 1.1 Tray and global hotkeys

**Answer:** `tray-icon` 0.26.1 and `global-hotkey` 0.9.0 work alongside GPUI's event loop with no extra thread or
message pump. PrintScreen registers successfully, but what happens when the key is pressed is still unverified and
needs the owner's check below.

Tested 2026-11-08 on Windows 11 build 26200, Rust 1.98.1, `gpui-pre` 0.3.11. Code: `crates/toru/examples/spike_tray.rs`.

### What was tested

The example opens a GPUI window, adds a tray icon with an Open window / Quit menu, and registers the hotkeys. Every
received event is printed to stdout and added to the window's event list.

| Part | Result | How it was checked |
|---|---|---|
| Register PrintScreen, no modifiers | Succeeded | Stdout. `PrintScreenKeyForSnippingEnabled` was absent from `HKCU\Control Panel\Keyboard` |
| Register Ctrl+Shift+W, Ctrl+Shift+F | Succeeded | Stdout |
| Hotkey delivery | Works | Synthesized Ctrl+Shift+F12 (a spike-only hotkey), twice. `Pressed` and `Released` both reached GPUI |
| Tray event delivery | Works | Posted left-button down/up to the tray's hidden window. Two `Click` events reached GPUI |
| Tray icon known to the shell | Yes | Those events carry a real icon rectangle from `Shell_NotifyIconGetRect` |
| Menu event delivery | Works | Posted `WM_COMMAND` for each item to the tray's hidden window |
| Close window, app keeps running | Works | Posted `WM_CLOSE`. Process stayed alive, tray and hotkey windows stayed |
| Tray Open reopens the window | Works | New `Zed::Window` appeared with the event history intact, and hotkeys still worked |
| Tray Quit exits | Works | Exit code 0, stderr empty |
| GPUI view updates from events | Works | The view's `render` ran again after events arrived |

### The approach that worked

- **Quit mode:** `application().with_quit_mode(QuitMode::Explicit)`. The default ends the app when the last window
  closes. Quit calls `cx.quit()`.
- **Thread:** create `GlobalHotKeyManager` and `TrayIcon` on GPUI's main thread, inside the `run` callback. Each owns a
  hidden window (`global_hotkey_app`, `tray_icon_app`).
- **Message pump:** none needed. GPUI's `GetMessageW(&mut msg, None, 0, 0)` loop takes messages for every window on the
  thread, so it also dispatches `WM_HOTKEY` and the tray's messages.
- **Reaching GPUI:** use each crate's `set_event_handler` (`GlobalHotKeyEvent`, `TrayIconEvent`, `MenuEvent`) to send
  into one `std::sync::mpsc` channel. A `cx.spawn` task on the main thread loops on
  `cx.background_spawn(async move { let m = receiver.recv(); (receiver, m) }).await` and handles each message in
  `cx.update`. No polling, and all events were handled on the main thread (`ThreadId(1)`).
- **Handlers must not touch GPUI.** Most run inside a window procedure on the main thread, and hotkey `Released` runs on
  a short-lived thread `global-hotkey` spawns for each press (`ThreadId(4)` and `(6)` in the run).
- **Lifetime:** the `spawn` task owns the manager and tray icon. When it ends on Quit, dropping the tray icon removes it
  from the tray.
- **Reopening:** GPUI's `App::hide` does nothing on Windows, so closing the window is the way to get to the tray. Keep
  state in an entity outside the window and open a new window to show it again, as the example does.

### Limits

- Waiting on the channel holds one Windows thread-pool thread blocked for the life of the app.
- While registered, a hotkey is taken from every other app: Ctrl+Shift+W and Ctrl+Shift+F won't reach a focused browser
  or editor. Registration fails if another app already holds the combination.
- `global-hotkey` sets `MOD_NOREPEAT`, so holding a key sends one `Pressed`.
- Killing the process rather than using Quit can leave a stale icon in the tray until the pointer passes over it.

### Not verified

- Anything that happens when PrintScreen, Ctrl+Shift+W or Ctrl+Shift+F is pressed. They were only registered.
- Whether Snipping Tool also opens on PrintScreen, and whether registration differs with Windows' setting explicitly on
  or off. With the registry value absent, the system default applies, and it wasn't observed here.
- Real mouse clicks on the tray icon, the right-click menu appearing and choosing from it. The automated checks posted
  the messages those clicks produce.
- How the icon, the menu and the window's event list actually look. No screenshots were taken; only `render` running
  was observed.
- GPUI staying live while the popup menu's modal loop runs.
- Reopened and focused windows coming to the front, and the tray icon disappearing right after Quit.
- Recovery after Explorer restarts, which `tray-icon` handles through `TaskbarCreated`.

### Owner's manual checklist

Run from PowerShell, with `%USERPROFILE%\.cargo\bin` on `PATH`: `cargo run -p toru --example spike_tray`. The setting is
in Settings > Accessibility > Keyboard, "Use the Print screen key to open screen capture".

Tray and window:
- [ ] The orange square appears in the tray, with the tooltip "Toru tray spike".
- [ ] The window lists the four registration results and each event as it arrives.
- [ ] Left-click the icon: a `tray Click` line appears and no menu opens.
- [ ] Right-click the icon: the menu opens. Press a hotkey with it open; the line appears in the window.
- [ ] Press Esc or click elsewhere: the menu closes.
- [ ] With the window open but behind another app, choose Open window: it comes to the front.
- [ ] Close the window with its X: the app keeps running in the tray and the hotkeys still print to the console.
- [ ] Choose Open window: the window returns, in front, with the earlier lines.
- [ ] Choose Quit: the process ends and the icon leaves the tray straight away.

Hotkeys, in a window that doesn't use these shortcuts:
- [ ] Press Ctrl+Shift+W: `Pressed` then `Released` appear.
- [ ] Press Ctrl+Shift+F: `Pressed` then `Released` appear.

PrintScreen, restarting the example after each change of the setting:

| Setting | Registration line | Toru gets `Pressed` and `Released`? | Snipping Tool opens? |
|---|---|---|---|
| On | | | |
| Off | | | |
