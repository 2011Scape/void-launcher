# Void Launcher

The desktop launcher for [Void](https://greghib.github.io/void/), the 2011 RuneScape experience recreated
from scratch, and for [2011Scape](https://2011.rs), its public world. It downloads the
[634 client](https://github.com/2011Scape/634-client), keeps it (and itself) up to date, makes sure a
suitable Java is available, and starts the game with the options you choose.

## Download

| Platform | Stable (latest release) | Development build |
|---|---|---|
| Windows 10/11 (64-bit) | [void-launcher-windows-x86_64.exe](https://github.com/2011Scape/void-launcher/releases/latest/download/void-launcher-windows-x86_64.exe) | [dev build](https://github.com/2011Scape/void-launcher/releases/download/dev/void-launcher-windows-x86_64.exe) |
| macOS 10.15+ (Intel and Apple Silicon) | [void-launcher-macos-universal.app.zip](https://github.com/2011Scape/void-launcher/releases/latest/download/void-launcher-macos-universal.app.zip) | [dev build](https://github.com/2011Scape/void-launcher/releases/download/dev/void-launcher-macos-universal.app.zip) |
| Linux (AppImage) | [void-launcher-linux-x86_64.AppImage](https://github.com/2011Scape/void-launcher/releases/latest/download/void-launcher-linux-x86_64.AppImage) | [dev build](https://github.com/2011Scape/void-launcher/releases/download/dev/void-launcher-linux-x86_64.AppImage) |
| Linux (plain binary) | [void-launcher-linux-x86_64](https://github.com/2011Scape/void-launcher/releases/latest/download/void-launcher-linux-x86_64) | [dev build](https://github.com/2011Scape/void-launcher/releases/download/dev/void-launcher-linux-x86_64) |

`SHA256SUMS` and `manifest.json` are published next to every build. All releases:
<https://github.com/2011Scape/void-launcher/releases>

- **Windows:** SmartScreen may say "Windows protected your PC" until the signed builds have earned a
  download reputation. Click "More info", then "Run anyway". (Releases are Authenticode-signed through
  [SignPath Foundation](https://signpath.org/about) once the project is onboarded; see `.github/workflows/release.yml`.)
- **macOS:** the app is not notarized. After unzipping, right-click `Void.app` and choose *Open*.
  On macOS 15, open it once, then allow it under *System Settings > Privacy & Security > Open Anyway*.
  Or run `xattr -dr com.apple.quarantine ~/Downloads/Void.app`.
- **Linux:** `chmod +x void-launcher-linux-x86_64.AppImage` and run it. Needs a Wayland or X11
  desktop with `libxkbcommon` and Vulkan or OpenGL drivers; `libfuse2` is optional (the AppImage
  extracts itself without it).

## What it does

- **Play** on the world you pick: 2011Scape (`play.2011.rs`), a Void server on this computer
  (`localhost`), or a custom host and port. A custom server can have its own keys: point the
  launcher at its `game.properties` and it is passed to the client with `--keys`.
- **Renderer:** the client's own choice, 634HD (OpenGL 3.3), OpenGL, or Safe mode.
- **Game, Graphics, Developer** settings cover the client's switches: tile markers, experience drops,
  NPC and ground item labels, shift-drop, wider zoom, render distance, world id; frame rate cap,
  interface scale, MSAA, SMAA, supersampling; the developer overlay (`--debug`) and a free-text field
  for anything else (`--lang`, `--dev-commands`, ...). Every default is the client's own, so an untouched
  profile passes no switches at all.
- **Java:** finds a Java 21 to 25 on your system (JAVA_HOME, PATH, the usual install folders, the
  Windows registry, `java_home` on macOS), or installs a private Eclipse Temurin 21 runtime into
  `~/.void/runtime` with one click. Nothing else on your computer is changed.
- **Updates:** checks the client's and its own GitHub releases on every start. The client is
  compared by SHA-256 from the release's `SHA256SUMS`, so nothing is re-downloaded needlessly.
  One *Channel* switch (Stable / Development) applies to both: Stable follows tagged releases,
  Development follows the `dev` pre-release that every push to `main` rebuilds.
- **Bug reports:** after a crash, or from *Report a problem*, the launcher puts a short report on
  your clipboard and opens a pre-filled issue on the client repository.

Everything the launcher owns lives in `~/.void`: `launcher.json` (settings), `client/`, `runtime/`,
`logs/`. The game's own files (`~/.jagex_cache_32`, `~/.634hd-settings.txt`, `~/.lwjgl/634-client`)
belong to the client and are never touched.

The client is started as
`java -Xms… -Xmx… -XX:+UseZGC -jar runetek5-634-<sha>.jar --address <host> --port <port> [--world n] [--keys file] [--toolkit hd|opengl|java] [gameplay] [graphics] [--debug] [extra client args]`.

## Building

```
cargo build --release -p void-launcher   # native build for your machine
just check                               # fmt, clippy, tests
just build linux-x86_64                  # what the Linux CI runner does (also windows-x86_64, macos-universal)
```

Releases are built by `.github/workflows/release.yml` on native GitHub runners: a `vX.Y.Z` tag
publishes a stable release, every push to `main` republishes the `dev` pre-release. `ci/lib.sh`
documents how the version and channel baked into the binary are derived. Set `VOID_LOG=debug` for
verbose logging; `ICED_BACKEND=tiny-skia` forces software rendering; `VOID_HOME` moves `~/.void`.

## Code signing policy

Free code signing on Windows is provided by [SignPath.io](https://signpath.io), certificate by the
[SignPath Foundation](https://signpath.org). Every Windows release is built by the public GitHub
Actions workflow in this repository and signed from that build; nothing is signed by hand.

- Committers and reviewers: the [2011Scape](https://github.com/2011Scape) organization members.
- Approvers (release signing): [@HarleyGilpin](https://github.com/HarleyGilpin).

**Privacy:** the launcher contacts only `github.com` (to read and download client and launcher releases
from 2011Scape/634-client and this repository) and `api.adoptium.net` (to download a Java runtime when
you ask for one). It sends no telemetry and transfers no information to other networked systems unless
you request it, for example by opening a bug report in your browser. The game client it starts has its
own network behaviour, documented in [634-client](https://github.com/2011Scape/634-client).

## License

AGPL-3.0. Fonts: Cinzel, Inter Tight and JetBrains Mono under the SIL Open Font License (see
`crates/launcher/assets/fonts/`).
