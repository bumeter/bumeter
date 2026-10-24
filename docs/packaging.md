# Packaging for Homebrew and WinGet

Bumeter is installed from GitHub Releases. Two package managers make that one command instead of
"open the releases page and pick the right file": a Homebrew cask for macOS and a WinGet manifest for
Windows. Both are generated from the release assets, so the versions and hashes cannot drift apart.

```
scripts/packaging.py                       # latest release → packaging/
scripts/packaging.py --version v1.0.5      # a specific tag
scripts/packaging.py --check               # verify what is committed, write nothing
scripts/winget-pr.sh                       # the manifests → a winget-pkgs pull request
```

The script downloads each asset once (cached in the system temp directory), hashes it, and writes:

| File | Belongs in |
|---|---|
| `packaging/homebrew/bumeter.rb` | `Casks/bumeter.rb` in the `bumeter/homebrew-tap` repository |
| `packaging/winget/Bumeter.Bumeter*.yaml` | `manifests/b/Bumeter/Bumeter/<version>/` in a `microsoft/winget-pkgs` pull request |

`--check` exits non-zero when a committed manifest no longer matches the release, which is the sanity
check to run before tagging the next version. It needs no token; `GITHUB_TOKEN` in the environment
only raises the API rate limit, and `BUMETER_REPO` overrides the repository.

The three WinGet manifests carry the schema version the community repository asks for
(`MANIFEST_VERSION` in the script, 1.12 as of the current PR template — 1.6.0 is deprecated there)
and open with the `yaml-language-server` comment its checklist requires.

## Homebrew

```bash
brew install --cask bumeter/tap/bumeter
```

The cask lives in [bumeter/homebrew-tap](https://github.com/bumeter/homebrew-tap) and cannot live in
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

The cask's `binary` stanza links the `bum` command line tool, which ships inside the app as a Tauri
sidecar (`Bumeter.app/Contents/MacOS/bum`), into Homebrew's `bin` — the macOS counterpart of
`/usr/bin/bum` in the `.deb`. Homebrew removes the link on uninstall.

Because the app is quarantined by macOS on first launch, the cask carries a caveat saying to
right-click → Open (or to clear the quarantine flag, which also unblocks `bum` in a terminal). That is the honest version of "it installs
without the Gatekeeper dialog".

## WinGet

```powershell
winget install Bumeter.Bumeter
```

WinGet packages are manifests inside a monorepo, so a release is a branch and a pull request. After
the first version is in, `scripts/winget-pr.sh` does the whole round trip:

```bash
scripts/winget-pr.sh                       # version from packaging/, fork chieund
scripts/winget-pr.sh --version 1.0.6       # a specific version
scripts/winget-pr.sh --no-push             # stop after the commit and print the push command
WINGET_FORK=someone scripts/winget-pr.sh   # another fork
```

It clones the fork shallowly (`--depth 1`, blobless, sparse on `manifests/b/Bumeter`, so a few
hundred KB rather than the whole catalogue), copies the three manifests into
`manifests/b/Bumeter/Bumeter/<version>/`, and checks them against the official WinGet schemas before
committing — the first thing the winget-pkgs bot does, so a typo fails on your machine instead of in
CI. It picks the title the pull request template asks for (`New package: Bumeter.Bumeter version
1.0.5` the first time, `Update: Bumeter.Bumeter to 1.0.6` after that), pushes the branch, and prints
the URL to open the pull request; with `gh` installed it opens it directly with the body.

Forking is a web click the first time: <https://github.com/microsoft/winget-pkgs/fork>. The commit is
authored with your `git` identity, which the CLA bot matches to your GitHub account — a placeholder
`user.email` is the one thing that stops a pull request before a human looks at it.

On Windows, the checks the checklist asks for are still available:

```powershell
winget validate --manifest manifests/b/Bumeter/Bumeter/1.0.5
winget settings --enable LocalManifestFiles ; winget install --manifest manifests/b/Bumeter/Bumeter/1.0.5
```

`wingetcreate update Bumeter.Bumeter --version 1.0.6 --submit` does the same from Windows, if you
prefer it to the script. The manifest uses the NSIS installer (`.exe`), which installs into the user
profile without an administrator prompt; `scripts/packaging.py --installer msi --product-code '{GUID}'`
switches to the WiX package instead. The MSI ProductCode can be read with
`msiinfo export Bumeter_1.0.5_x64_en-US.msi Property` (package `msitools`).

The installer is not code-signed, so SmartScreen shows "Windows protected your PC" on the first run:
More info → Run anyway. `winget install` itself does not show that dialog.

`AppsAndFeaturesEntries` in the installer manifest repeats the name and publisher the installer
writes into "Apps & features". Tauri takes the publisher from `bundle.publisher`, and with that unset
falls back to the second element of the identifier (`dev.bumeter.app` → `bumeter`) — the released
`.msi` does have `Manufacturer: bumeter`. WinGet matches an already-installed app on those two
values, so `scripts/packaging.py` reads them out of `tauri.conf.json`; setting `bundle.publisher`
before the next release keeps the manifest and the installer in step.


`bum` on `PATH`: a nullsoft installer cannot get a command alias from its manifest, so the installer
does it itself. `app/src-tauri/windows/hooks.nsh` (wired in through `bundle.windows.nsis.installerHooks`)
adds the install directory to the user `PATH` after install and removes it after uninstall, through
PowerShell so a long `PATH` is not cut at NSIS's 1024-character limit and `%VARIABLES%` stay unexpanded.
The release workflow runs the real installer and uninstaller silently on the Windows runner and checks
both directions. The manifest lists `Commands: [bum]` for `winget search --command bum`; submit WinGet
from the first release that has the hook (1.0.7), since 1.0.6's installer does not touch `PATH`. The
`.msi` does not add `bum` to `PATH`.
## Release checklist

1. Bump `version` in `Cargo.toml` and `app/src-tauri/tauri.conf.json`, commit, tag `v1.0.6`, push the tag.
2. Wait for the release workflow to attach the assets.
3. `scripts/packaging.py` (or `--version v1.0.6`), then commit `packaging/`.
4. Copy the cask into the tap and push; `scripts/winget-pr.sh` for the WinGet pull request.
5. First time only: fork `microsoft/winget-pkgs` in the browser and sign the CLA when the bot asks.

The app updates itself from GitHub Releases (`src/update.rs`), so the package managers only have to
get the user to the first install; the later versions arrive on their own.
