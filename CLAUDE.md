# Void Launcher

Rust workspace, AGPL-3.0. Native desktop launcher for the 634 client (2011Scape/634-client), styled after
https://greghib.github.io/void/ (iced GUI over a UI-free core crate).

## Layout

- `crates/launcher-core` — no UI. Settings (`~/.void/launcher.json`), GitHub release resolution
  (`github.rs`: asset URLs + `SHA256SUMS`, no REST API), client install by SHA-256 (`client.rs`), Java
  discovery/probing and the managed Temurin runtime (`java/`), launcher self-update (`self_update.rs`),
  detached client launch (`launch.rs`), crash pickup and bug-report text (`report.rs`). Long operations
  return `sipper::Straw` values that report `Progress`.
- `crates/launcher` — iced 0.14 GUI, package/bin `void-launcher`. `app.rs` is the state machine, `tasks.rs`
  the only file that knows both iced and core, `theme/` the Void tokens (`mod.rs`) and style functions,
  `view/` the screens.
- `ci/*.sh` + `.github/workflows/{check,release}.yml` — native runners (ubuntu-22.04, windows-latest,
  macos-14). `ci/lib.sh` derives VERSION/CHANNEL from `GITHUB_REF`; `ci/make-dist.sh` writes the
  `manifest.json` the updater reads.

## Contracts that must not drift

- Client command line: `java -Xms.. -Xmx.. [-XX:+UseZGC] <extra jvm args> -jar client/runetek5-634-<sha12>.jar
  --address <host> --port <port> [--world n] [--keys <game.properties>] [--toolkit hd|opengl|java]
  [gameplay: --tile-markers --xp-drops --npc-labels --ground-items --no-shift-drop --no-zoom]
  [graphics: --render-distance --fps --scale --msaa --render-scale --smaa] [--debug] <extra client args>`
  (see `launch.rs` tests). A switch is only emitted when the setting differs from the client's default
  (`settings.rs`: `Extra::default_on`, the `numeric_choice!` enums' `default`). Each install is its
  own jar so updates never replace a jar a running client holds; `prune_old_jars` removes the rest.
- Client releases: GitHub `2011Scape/634-client`. Stable = `releases/latest/download/<file>`, Development =
  `releases/download/dev/<file>`. The jar is `runetek5-634-<version>.jar` (ignore `-browser-` zips); version and
  SHA-256 come from the release's `SHA256SUMS`. Dev versions look like `dev-20261017-fd94175`.
- Launcher releases: GitHub `2011Scape/void-launcher`, same URL scheme, files `void-launcher-<target>[.ext]` plus
  `manifest.json` (schema 1) and `SHA256SUMS`. Self-update silently does nothing while the repo is private
  (GitHub answers 404 for private assets).
- Java 21–25 (`MIN_JAVA`/`MAX_TESTED_JAVA`); the managed runtime is Temurin 21.
- Never touch the client's own files: `~/.jagex_cache_32`, `~/.634hd-settings.txt`, `~/.lwjgl/634-client`.
- Crash pickup (`report.rs`): the JVM leaves `hs_err_pid*.log` in `~/.void` (the client's cwd); the launcher's
  panic hook writes `logs/launcher-crash-<stamp>.txt`. On boot the newest file newer than
  `settings.last_crash_seen` opens the bug-report dialog (copy → open pre-filled GitHub issue); with
  "keep open" the same dialog follows a non-zero client exit. 634-client writes no crash file of its own.

## Working on it

- `just check` (fmt, clippy -D warnings, tests). `just net-test` exercises the real GitHub/Adoptium APIs and scans
  local Java (`VOID_NET_TESTS=1`).
- UI checks: run with `HOME=<scratch dir>` for a fresh profile (`VOID_HOME` also works) and `VOID_LOG=debug` for
  verbose logs. Debug builds open a panel at boot with `VOID_START_PANEL=settings|java|update|report|welcome`.
  Under this machine's KDE Wayland session: unset `WAYLAND_DISPLAY` so the window runs on XWayland and
  `xdotool search`/`magick import -window` can see it; xdotool cannot inject input (XTEST is blocked), `ydotool key
  28:1 28:0` (Enter) works. Keep the default wgpu backend for screenshots: with `ICED_BACKEND=tiny-skia` the dialog
  layer renders dimmed under the scrim (software-renderer layering quirk, not a bug in the view). When pressing Play
  in a test, put `-Duser.home=<scratch dir>` in Extra JVM arguments first: Java ignores `$HOME`, so the client would
  otherwise write to the real `~/.jagex_cache_32`.
- Design tokens are copied from the site's `style/tokens/*.css`; keep `theme/mod.rs` in step with it. Fonts:
  Cinzel (display), Inter Tight (UI), JetBrains Mono (code), all OFL.
- Branch rules (GitHub rulesets): `main` needs a PR (squash/rebase only), the `check` status, linear history, no
  force-push; `v*` tags can only be created by repository admins. Admins bypass both, which is what lets
  `just release` push to `main` and tag directly; everyone else goes through a PR. The `dev` tag is deliberately
  outside the tag rule because the Release workflow moves it with the Actions token.
- Release: bump `version` in `Cargo.toml`, commit, tag `vX.Y.Z` (`just release X.Y.Z`). The `check` job fails a tag
  that does not match `Cargo.toml`. Every push to `main` republishes the `dev` pre-release.
- Windows code signing: `release.yml`'s `sign-windows` job sends the exe to SignPath (SignPath Foundation, free for
  OSS) and `dist` packs the signed copy, so `manifest.json`/`SHA256SUMS` always describe what is published. It
  activates when repository variables `SIGNPATH_ORGANIZATION_ID` (+ optional `SIGNPATH_PROJECT_SLUG`, default
  `void-launcher`) and secret `SIGNPATH_API_TOKEN` exist; policies are `test-signing` (main) and `release-signing`
  (tags, needs a maintainer's approval in the SignPath portal, job waits up to 2 h). Not run for pull requests.
  macOS still needs an Apple Developer ID + notarization (paid); Linux needs nothing.
- Windows Defender once flagged an unsigned predecessor build as `Trojan:Win32/Wacatac.B!ml` (a cloud ML verdict on
  low-prevalence unsigned binaries). Mitigations kept: full version-info block (`build.rs`), `strip = "debuginfo"` so the
  symbol table stays, no registry reads for the OS version. Code-signing is the real fix.
- Rust 1.93 toolchain pinned in `rust-toolchain.toml` and the workflows; `sysinfo` is held at 0.37 because 0.39 needs
  rustc 1.95.
