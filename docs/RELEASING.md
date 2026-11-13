# Releasing Toru

Releases are made on demand, not on every merge: merge a batch of PRs, then cut a release. The
[Release workflow](../.github/workflows/release.yml) does the rest on a GitHub Windows runner.

## Cut a release

Either open **Actions → Release → Run workflow** on `main`, or:

```powershell
gh workflow run release.yml -f bump=minor
```

Inputs:

- `bump`: `patch` (default), `minor` or `major`, applied to the `version` in `Cargo.toml`'s `[workspace.package]`.
- `version`: an exact version such as `0.1.0`, used instead of `bump`. Giving the current version publishes it as it
  is, without a version commit (that is how the first release, 0.1.0, is made:
  `gh workflow run release.yml -f version=0.1.0`).
- `summary`: optional text placed above the generated notes, e.g.
  `gh workflow run release.yml -f bump=patch -f summary="Fixes a crash when saving to a full disk."`

Watch it with `gh run watch` (it takes a while: a full release build plus the licence notices).

## What the workflow does

1. Refuses to run from any branch but `main`, or if the tag `v<version>` already exists.
2. Sets the new version in `Cargo.toml` and runs `cargo update --workspace` so `Cargo.lock` matches.
3. Builds the installer with `installer\build.ps1` (Inno Setup 6 is on the runner image; cargo-about is installed and
   cached).
4. Writes the patch notes: the `summary`, then one bullet per commit title on `main` since the previous `v*` tag, with
   the `(#N)` PR numbers and `Release x.y.z` commits left out. The first release says "First public release."
5. Commits `Release <version>` to `main` as `github-actions[bot]` (only if the version changed), tags `v<version>`
   and pushes both.
6. Publishes the release `Toru <version>`, marked latest, with two assets:
   - `Toru-<version>-setup.exe`
   - `toru-update.txt`, which installed copies of Toru read from
     `https://github.com/ChacaPatrick/toru/releases/latest/download/toru-update.txt` to find updates:

     ```
     version: 0.2.0
     installer: https://github.com/ChacaPatrick/toru/releases/download/v0.2.0/Toru-0.2.0-setup.exe

     <patch notes>
     ```

     UTF-8, LF line endings. Header lines are `key: value` up to the first blank line; the rest is the notes, plain
     text where `## ` lines are headings and `- ` lines are bullets. The release body is the same notes.

## Edit the notes afterwards

The commit titles are a starting point. To reword them, edit the release body:

```powershell
gh release edit v0.2.0 --notes-file notes.md
```

Toru's "What's new" reads `toru-update.txt`, not the release body, so replace that asset too. Write the file with LF
line endings (the header is the two lines shown above, then a blank line, then the notes):

```powershell
gh release upload v0.2.0 toru-update.txt --clobber
```

## If a run fails

- Before the push step nothing has changed on GitHub: fix the cause and run it again.
- If the push fails because `main` moved while the build ran, run it again.
- If publishing fails after the tag was pushed, delete any draft it left (`gh release delete v<version>`) and the tag
  (`git push origin :refs/tags/v<version>`), then run the workflow again with `-f version=<version>`. `Cargo.toml`
  already has that version, so it rebuilds and publishes without another version commit.
