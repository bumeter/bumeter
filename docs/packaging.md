# Packaging for Homebrew and WinGet

Bumeter is installed from GitHub Releases. Two package managers make that one command instead of
"open the releases page and pick the right file": a Homebrew cask for macOS and a WinGet manifest for
Windows. Both are generated from the release assets, so the versions and hashes cannot drift apart.

```
scripts/packaging.py                       # latest release → packaging/
scripts/packaging.py --version v1.0.5      # a specific tag
scripts/packaging.py --check               # verify what is committed, write nothing
```

The script downloads each asset once (cached in the system temp directory), hashes it, and writes:

| File | Belongs in |
|---|---|
| `packaging/homebrew/bumeter.rb` | `Casks/bumeter.rb` in the `chieund/homebrew-tap` repository |
| `packaging/winget/Bumeter.Bumeter*.yaml` | `manifests/b/Bumeter/Bumeter/<version>/` in a `microsoft/winget-pkgs` pull request |

`--check` exits non-zero when a committed manifest no longer matches the release, which is the sanity
check to run before tagging the next version. It needs no token; `GITHUB_TOKEN` in the environment
only raises the API rate limit, and `BUMETER_REPO` overrides the repository.

## Homebrew

```bash
brew install --cask chieund/tap/bumeter
```

The cask lives in [chieund/homebrew-tap](https://github.com/chieund/homebrew-tap) and cannot live in
`homebrew/cask`: Homebrew only accepts applications there that are signed and notarized with an Apple
Developer ID, and CI builds Bumeter's `.dmg` without certificates. A personal tap has no such
requirement.

After a release has attached its assets:

```bash
./scripts/packaging.py --version v1.0.6
cp packaging/homebrew/bumeter.rb ../homebrew-tap/Casks/bumeter.rb
cd ../homebrew-tap && git commit -am "bumeter 1.0.6" && git push
```

`brew upgrade --cask bumeter` picks it up from there. There is no `livecheck` block on purpose: the
version and the SHA256 come from the release, so the command above is the bump.

Because the app is quarantined by macOS on first launch, the cask carries a caveat saying to
right-click → Open (or to clear the quarantine flag). That is the honest version of "it installs
without the Gatekeeper dialog".

## WinGet

```powershell
winget install Bumeter.Bumeter
```

WinGet packages are manifests in a monorepo, so a release is a pull request:

```bash
gh repo fork microsoft/winget-pkgs --clone --remote
cd winget-pkgs
mkdir -p manifests/b/Bumeter/Bumeter/1.0.5
cp <bumeter>/packaging/winget/Bumeter.Bumeter*.yaml manifests/b/Bumeter/Bumeter/1.0.5/
winget validate --manifest manifests/b/Bumeter/Bumeter/1.0.5   # on Windows
git checkout -b bumeter-1.0.5 && git commit -am "New package: Bumeter.Bumeter 1.0.5" && gh pr create
```

`wingetcreate update Bumeter.Bumeter --version 1.0.5 --submit` does the same in one command once the
first version is merged. The manifest uses the NSIS installer (`.exe`), which installs into the user
profile without an administrator prompt; `scripts/packaging.py --installer msi --product-code '{GUID}'`
switches to the WiX package instead. The MSI ProductCode can be read with
`msiinfo export Bumeter_1.0.5_x64_en-US.msi Property` (package `msitools`).

The installer is not code-signed, so SmartScreen shows "Windows protected your PC" on the first run:
More info → Run anyway. `winget install` itself does not show that dialog.

## Release checklist

1. Bump `version` in `Cargo.toml` and `app/src-tauri/tauri.conf.json`, commit, tag `v1.0.6`, push the tag.
2. Wait for the release workflow to attach the assets.
3. `scripts/packaging.py` (or `--version v1.0.6`), then commit `packaging/`.
4. Copy the cask into the tap and push; open the WinGet pull request.

The app updates itself from GitHub Releases (`src/update.rs`), so the package managers only have to
get the user to the first install; the later versions arrive on their own.
