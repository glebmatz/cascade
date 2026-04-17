# Releasing Cascade

This is the maintainer's checklist for cutting a new release. Contributors
don't need to read this.

## One-time infrastructure setup

You only need to do this before the **first** release.

### 1. Create the repositories

On GitHub, create these two public repos under the `glebmatz` user:

1. **`glebmatz/cascade`** — the main project. Push this working tree into it.
2. **`glebmatz/homebrew-cascade`** — a separate repo that acts as the
   Homebrew tap. It needs a single top-level directory called `Formula/`.
   Initialize it with just a README pointing back to the main project.

   ```sh
   mkdir homebrew-cascade && cd homebrew-cascade
   mkdir Formula
   cat > README.md <<'EOF'
   # homebrew-cascade

   Homebrew tap for [Cascade](https://github.com/glebmatz/cascade).

       brew install glebmatz/cascade/cascade
   EOF
   git init -b main
   git remote add origin https://github.com/glebmatz/homebrew-cascade
   git add . && git commit -m "init"
   git push -u origin main
   ```

### 2. Create the `HOMEBREW_TAP_TOKEN` secret

The release workflow needs write access to the tap repo.

1. Go to <https://github.com/settings/personal-access-tokens/new>.
2. Create a **fine-grained** PAT:
   - **Resource owner**: `glebmatz`
   - **Repository access**: Only select repositories → `homebrew-cascade`
   - **Permissions**: Repository → Contents → **Read and write**
3. Copy the token.
4. In the main `cascade` repo: **Settings → Secrets and variables → Actions
   → New repository secret**.
5. Name: `HOMEBREW_TAP_TOKEN`. Value: paste the token.

### 3. (Optional) Create the `CARGO_REGISTRY_TOKEN` secret

Only if you want the release workflow to auto-publish to crates.io.

1. Log in to <https://crates.io/>.
2. Go to **Account Settings → API Tokens → New Token**.
3. Scope: `publish-update`. Crate: `cascade-rhythm`.
4. Save as secret `CARGO_REGISTRY_TOKEN` in the main repo.

### 4. Verify the crate name is free

Run:

```sh
cargo search cascade-rhythm
```

If the name is taken, edit `Cargo.toml` and pick another, then `cargo
publish --dry-run` until it works. The binary name stays `cascade`.

## Cutting a release

1. **Make sure `main` is green**: CI passes, no unfixed regressions.

2. **Update `CHANGELOG.md`**:
   - Move items from `[Unreleased]` into a new `[x.y.z] — YYYY-MM-DD` section.
   - Update the version comparison links at the bottom.

3. **Bump the version**:
   ```sh
   # Edit Cargo.toml: version = "x.y.z"
   cargo check            # refreshes Cargo.lock with new version
   git add Cargo.toml Cargo.lock CHANGELOG.md
   git commit -m "chore: release x.y.z"
   ```

4. **Tag and push**:
   ```sh
   git tag -a vx.y.z -m "Release vx.y.z"
   git push origin main
   git push origin vx.y.z
   ```

   The tag push triggers `.github/workflows/release.yml`, which:
   - Builds binaries for all targets in parallel
   - Creates a GitHub Release with assets + auto-generated notes
   - Commits an updated formula to `glebmatz/homebrew-cascade`
   - Publishes to crates.io (if the token is set)

5. **Verify** (5 – 10 minutes later):
   - Release appears at <https://github.com/glebmatz/cascade/releases/latest>
   - All 5 platform tarballs are attached plus `cascade-installer.sh` /
     `cascade-installer.ps1`
   - Tap repo has a new commit updating `Formula/cascade.rb`
   - `brew install glebmatz/cascade/cascade` (on a clean machine) works
   - `curl ... /cascade-installer.sh | sh` works

6. **Announce**: Discussions post / README update if needed.

## If something goes wrong

- **Build fails for one platform**: fix the issue, push a new commit, then
  re-tag with the same version:
  ```sh
  git tag -d vx.y.z
  git push origin :vx.y.z
  git tag -a vx.y.z && git push origin vx.y.z
  ```
  The workflow will re-run.

- **Tap commit failed but GitHub Release went through**: the tap job is
  independent, re-run it from the Actions tab.

- **You accidentally published a broken version to crates.io**: `cargo
  yank --version x.y.z` stops new users from pulling it. Then cut
  x.y.z+1 with the fix.

## Version bumping rules

Cascade follows [Semantic Versioning](https://semver.org/):

- **Patch** (0.1.0 → 0.1.1): bug fixes, small tweaks, no behavioral changes.
- **Minor** (0.1.0 → 0.2.0): new features, non-breaking config / CLI additions.
- **Major** (0.x → 1.0): breaking changes to config format, CLI flags, or
  beatmap format. Pre-1.0 we have a bit more latitude but try to avoid
  breaking changes in minor bumps.

## Pre-release versions

For beta testing:

```sh
git tag -a v0.2.0-beta.1
git push origin v0.2.0-beta.1
```

The Homebrew tap and crates.io publish jobs are gated on the version NOT
containing a `-` character, so prereleases only produce GitHub Releases.
