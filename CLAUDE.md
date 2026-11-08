# Toru — Claude Code project notes

Toru is a Windows-first screenshot app built with GPUI. The design handoff calls it "Snapshot"; the name is Toru.

Read `design/DESIGN.md` first for behaviour. `design/mockups/*.html`, `design/screenshots/*.png` and
`design/tokens.json` are the visual source of truth. `docs/PLAN.md` is the build plan and tracks progress.

## Decisions that override the handoff
- Stack is GPUI (`gpui-pre`, pinned exactly), not Tauri. Ignore DESIGN.md §9.
- Looks shipped for now: Japandi (default), PixelArt, NeoBrutalism, each with its 3 palettes plus a custom fourth
  slot (own colours, optional background photo). The look and palette are chosen in settings.
- "This session" ends at midnight local time, on New session, and when the app exits. Hiding to the tray doesn't end it.
- The "Show previous screenshots" toggle stays and defaults to ON. ON: the gallery opens on today, and scrolling down
  loads earlier days one at a time. OFF: only "This session" shows.
- With the toggle ON, the history group right below "This session" is "Earlier today", holding today's captures from
  sessions that have ended. Then comes the previous day, and so on.
- History includes saved and unsaved captures.
- Unsaved captures are deleted automatically after 30 days by default. Other preset day counts, a custom count, or
  Never (never delete automatically) are options. Retention never touches files the user saved.
- On close (DESIGN.md §2) decides what happens to this session's unsaved captures when the app exits: "Keep" sends
  them to history under the retention setting, "Discard" deletes them, "Ask" prompts. Saved captures always stay in
  history, and earlier history is not affected.
- Four strings change. Settings hint: "Off = clean slate each session. On = earlier shots load below, a day at a
  time as you scroll." Session line: "… · nothing saved to your folder until you save". On close option: "Keep unsaved
  in history". Day group header: "5 captures · 2 unsaved · shown because “Show previous” is on".
- The `This session | History` tabs in the Select mockups are left over. There is one scrolling gallery; select mode
  keeps the Main mockup's header and only adds the select-mode controls.
- The settings column can be collapsed (not in the mockups). Day groups also have the mockups' Collapse button.
- The return pill is top-centre of the gallery.
- Delayed capture lets the user pick region, window or full screen as well as the seconds.
- File size appears in a tooltip and the `⋯` menu, not in the thumbnail caption.
- Screen reader support is deferred unless it works on Windows. Shared controls still set `.role()`/`.aria_label()`.
  Keyboard navigation, focus rings and contrast from DESIGN.md §7 are acceptance criteria.
- Merge in select mode is v2.

## Decisions kept from the handoff (do not re-litigate)
- Captures are temporary until saved; clipboard copy on capture is ON by default.
- Status feedback = toast + button-to-checkmark; cards never change shape.
- Select mode bulk bar: Copy as files, Copy paths (saved only), Save (skips saved), Delete.
- Editor = floating tool pill + left context panel; tools and shortcuts in DESIGN.md §5.

## Working rules
- Every completed section in `docs/PLAN.md` is one branch and one pull request, squash-merged to `main` as one commit.
- Before a merge: format, clippy and tests pass, then a fresh agent reviews the PR. Only bugs and missed requirements
  count as findings. Fix them and review again, for at most 3 rounds; anything still open after round 3 is listed for
  the owner and doesn't block the merge. If the same finding is disputed twice, ask the owner.
- Whoever's PR conflicts with `main` rebases it and resolves the conflict, then re-runs the checks.
- A PR that changes what the app looks like includes screenshots in its description, each with a caption saying what it
  shows. They are of the application only, taken with a window-specific capture such as `PrintWindow`, from a throwaway
  data folder with synthetic captures, and stored on the `pr-screenshots` branch under a folder named for the section.
- Keep code straightforward. Don't over-engineer or repeat yourself unless needed.
- Most coding: Opus 5.5, High, Fast. Graphics, design and UI work: Fable 5.1 only, Opus 5.5 if Fable is exhausted.
- Sections may run in parallel, each in its own git worktree. Conflicts are resolved at merge time.
- Don't send simulated mouse or keyboard input while the owner may be using the computer, unless they've said to.
- Windows first; keep Windows-specific code in one small module so macOS/Linux can slot in.
- Colours and sizes come from the theme; never hard-code them in components.
- UI sections are compared with the mockup PNGs before they count as done.
- Ask before adding dependencies beyond those listed in `docs/PLAN.md`.
