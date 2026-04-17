# Contributing to Cascade

Thank you for wanting to help! Cascade is a young project and almost
everything is on the table. This guide covers the **what** and **how** of
contributing.

## TL;DR

1. Fork the repo on GitHub.
2. Create a branch: `git checkout -b fix/some-thing` or `feat/add-thing`.
3. Make your change. Add tests if behavior changed.
4. Run `cargo fmt && cargo clippy -- -D warnings && cargo test`.
5. Commit with a [Conventional Commits](https://www.conventionalcommits.org/)
   message: `fix: clamp scroll speed to 0.5 – 2.0`.
6. Push and open a pull request against `main`.

We try to respond to PRs within a week.

## Getting started

### Prerequisites

- **Rust** 1.85 or newer (`rustup update stable`)
- On **Linux**: `libasound2-dev` (or your distro's equivalent)
  ```sh
  # Debian / Ubuntu
  sudo apt install libasound2-dev pkg-config

  # Fedora
  sudo dnf install alsa-lib-devel pkgconf-pkg-config

  # Arch
  sudo pacman -S alsa-lib pkgconf
  ```
- macOS and Windows need no extra system libs.

### Clone and build

```sh
git clone https://github.com/glebmatz/cascade
cd cascade
cargo build --release
./target/release/cascade
```

### Running the test suite

```sh
cargo test
```

Tests cover the hit-judge windows, score state machine, and config I/O. The
beatmap generator is intentionally **not** unit-tested — it's validated by
ear on real audio; feel free to contribute synthetic test vectors if you
want to change that.

### Formatting and lints

All code must pass both before we merge:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

CI blocks on these — fix them locally first to avoid the round-trip.

## What's worth a PR?

**Always welcome**

- 🐛 Bug fixes with a clear repro
- 📖 Documentation improvements (README, CONTRIBUTING, inline docs)
- 🎨 Visual polish (new particle effects, animations, menu improvements)
- 🖥 Terminal compatibility fixes and entries in the compatibility table
- 🎼 Beatmap generator improvements — if you're bringing an algorithmic
  change, please explain *why* it's better (e.g. "better onset recall on
  tracks with dense hi-hat")
- 🧪 Tests for things that aren't tested

**Please open an issue first**

- 🚧 Large refactors (> 300 LOC)
- ➕ New external dependencies
- 🗜 Changes to the beatmap on-disk format — these break existing saves
- 🌐 Online features (song sharing, leaderboards, multiplayer)

## Code style

- **Rust**: we follow the defaults of `cargo fmt`. No custom rustfmt config.
- **Comments**: write them for *why*, not *what*. If a comment would
  restate the code, delete it.
- **Function size**: if a function is longer than one screen, consider
  splitting it. `pick_peaks` in `generator.rs` is at the upper limit.
- **Panics**: panicking is fine for CLI misuse and programmer errors.
  Never panic on user-provided audio or terminal state — return
  `anyhow::Result` instead.
- **No `unwrap()` on user input**: use `?` propagation or `.unwrap_or_default()`.
- **Public API**: everything in `src/lib.rs` is technically an API, but we
  don't promise stability pre-1.0. Still, don't break it casually.

## Commit messages

We use [Conventional Commits](https://www.conventionalcommits.org/).

```
<type>(<optional scope>): <short imperative summary>

<optional longer body — wrap at 72 chars>

<optional footer with BREAKING CHANGE: / Closes #123 / etc>
```

Common types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`, `chore`.

Examples:

```
feat(generator): detect chords from secondary band flux
fix(audio): use sink.get_pos() for real playback position
perf(render): precompute Hann window in compute_novelty
```

## Pull request process

1. **One PR, one concern.** Small, focused PRs are reviewed quickly.
   Refactoring + a feature + a bug fix in one PR → expect it to bounce.
2. **Describe the user-visible change.** What would I say about this in a
   release note?
3. **Link the issue.** `Closes #42` in the description.
4. **Keep the diff minimal.** Don't include unrelated `cargo fmt` drift.
5. **Run the checks locally.** See [Formatting and lints](#formatting-and-lints).
6. **Be patient.** Maintainers may ask for changes. It's nothing personal.

When a PR is ready:

- We'll do a review pass. Please respond to comments even if it's "ok, fixed."
- Merges are usually **squash merges** so commit history stays clean.
- After merge, the change will appear in the next release's CHANGELOG.

## Reporting bugs

Use the [bug report issue template][bug]. Please include:

- Cascade version (`cascade --version` — coming soon) or commit hash
- Terminal emulator and version
- OS and architecture
- Minimal reproduction steps
- Expected vs actual behavior
- For audio issues: song format, length, sample rate if you know it

## Proposing features

Use the [feature request issue template][feat]. The shorter and sharper,
the better. Include:

- What problem you're solving
- Why it belongs in Cascade and not a fork or plugin
- Rough sketch of the UI/CLI surface if relevant

## Security

Please **do not** open a public issue for security problems. Email
security reports to Gleb Matsko (the maintainer; see the commit author
field for the email address) and give us a chance to fix it before
disclosure.

## Be excellent to each other

Be kind, be helpful, assume good faith. Harassment, personal attacks, and
discrimination have no place here and will get you banned from the
project. That's the whole policy.

---

Thanks again! 🎵

[bug]: https://github.com/glebmatz/cascade/issues/new?template=bug_report.md
[feat]: https://github.com/glebmatz/cascade/issues/new?template=feature_request.md
