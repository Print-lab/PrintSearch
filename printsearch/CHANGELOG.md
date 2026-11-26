# Changelog

**English** · [Français](CHANGELOG.fr.md)

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **German interface** (Settings, console `lang en|de|fr`; the tray menu follows it).
- **Optional network access:** `PRINTSEARCH_BIND` (address to listen on) and `PRINTSEARCH_ALLOWED_HOSTS` (host names accepted in the `Host`/`Origin` headers). The default stays `127.0.0.1`. Clients that are not on the PrintSearch computer cannot set a custom browser path, run `logs export`, `logs clear` or `quit`, or start the Thingiverse OAuth flow.
- README sections *Headless Linux server (systemd)* and *Access from another machine*.

### Changed
- The README no longer says that server error messages are in French; the French source comments and messages were translated to English.

## [0.3.0] - 2026-11-22

### Added
- **Three new sites to search: Thangs, Cults and MyMiniFactory** (six sites in total), with their icons. They are searched without an account, by reading their results page with the background browser (title, picture and link). Their cards only have the *Open* link, and the Settings list a short *Search-only sites* card instead of connection fields.
- **Runs in the background with no terminal window.** On **Windows** the program no longer opens a console (it is a windowed program; run from a terminal it attaches to that terminal so commands still print). A **tray icon** (Windows taskbar, Linux notification area via StatusNotifierItem) opens the interface on left-click and offers *Open PrintSearch* and *Quit* on right-click. The menu language follows the interface language (applied at next start).
- **Started from a terminal, PrintSearch detaches by itself** (Linux): it relaunches in its own session without a terminal, gives you your prompt back and keeps running when you close the terminal. `--foreground` keeps it attached to the terminal.
- **Single instance:** launching PrintSearch while it is already running simply opens the existing one.
- **In-app console**, reached from **Settings → Advanced settings → Open the console** (a new *Advanced settings* page, which also holds the logs download), that accepts **the same commands as the terminal**, with history (↑/↓), Tab completion, `clear` and `lang en|fr`. Tokens typed in a command are masked on screen and in the log, and such commands are not kept in the history.
- **Application log** and the commands around it: `logs` (display, `-n`, `--level`), `logs export [path]` (whole log with a diagnostic header, to your Downloads folder by default), `logs path`, `logs clear`. Also **Settings → Advanced settings → Download logs**. The log records searches, errors, browser start-up, updates and crashes; it never contains tokens. It is limited to about 2 MB.
- New commands: `status` (version, browser, connections, log location) and `quit` (close the running PrintSearch).
- **Settings → Application** card with a *Quit PrintSearch* button.
- `serve --no-tray` to run in the terminal without a tray icon (Ctrl+C or SIGTERM stop it cleanly).
- Cargo feature `tray` (on by default). `--no-default-features` builds a terminal-only program.

### Changed
- **Results appear site by site**: each site is queried separately, so a quick site is displayed at once instead of waiting for the slowest one. Sites that fail for the same reason (for example no browser installed) share a single error message.
- Cards only show the **Like** and **Collection** buttons on the sites where they exist (Thingiverse and Printables for Like, Thingiverse for collections); the command line and the API give a clear message otherwise.
- At most **three pages are read at the same time** by the background browser, so that searching five browser-read sites at once does not overload it.
- `printsearch` (no argument) and `serve` now start the background application. The `.desktop` launcher uses `Terminal=false`.
- The background browser is no longer started at launch: it starts on the first search that needs it and is **closed after 10 minutes without use**, so a resident application does not keep a browser open for days.
- Shutdown is clean: the background browser is closed and the server stops within 6 seconds.
- The console is no longer a button on the main screen nor a tray menu entry: it moved to **Settings → Advanced settings**.
- Command-line help and messages are now in English (the web interface keeps its English / French switch).
- READMEs: tray icon, console, commands, logs, troubleshooting.

### Fixed
- A **browser process could be left running** after a command-line search or when the app was closed.
- A **corrupted `config.json`** prevented the app from starting; it is now set aside as `config.json.bad` and the app starts with defaults.
- The Thingiverse OAuth return page accepted any authorization code: it now only accepts one when the login was started from the application (within 10 minutes).
- Model and collection identifiers are validated before being placed in API addresses, and requests to the local API are limited to 64 KB.
- A blocked terminal could freeze logging in other threads.
- Closing the terminal of a PrintSearch started with `--foreground` or `--no-tray` now stops it cleanly (SIGHUP is handled) instead of crashing.

### Notes
- **macOS** has no tray icon in this version (it runs in the terminal).
- On **GNOME**, the tray icon needs the *AppIndicator and KStatusNotifierItem Support* extension. Without a tray, the app still runs; use the web interface or `printsearch quit`.
- On Windows, the tray icon may be hidden in the overflow area (`^`); drag it to the taskbar.

## [0.2.0] - 2026-11-20

### Added
- **Choice of the browser used for searches.** MakerWorld (and Thingiverse without a token) are read with a Chromium-based browser running in the background. You can now pick it in **⚙ Settings → Browser used for searches**: *Automatic*, a detected browser (**Chrome, Edge, Brave, Vivaldi, Chromium**) or a **custom path**. Chrome is no longer required: **Edge, preinstalled on Windows, is enough**.
- `printsearch browser` command to list detected browsers and select one (`auto`, `chrome`, `edge`, `brave`, `vivaldi`, `chromium` or a path).
- **Application icon embedded in the Windows `.exe`** (`build.rs`, `packaging/printsearch.ico`). If the icon cannot be embedded (for example `windres` missing when cross-compiling), the build still succeeds and prints a warning.
- **Progressive results:** each site is queried separately, so fast sites (e.g. Printables) appear immediately instead of waiting for the slowest one. A progress message shows how many sites have answered, and an error on one site no longer hides the others.
- **Update check** against the GitHub repository: *Settings → Updates → Check for updates*, the `printsearch update` command, and an optional check at startup (on by default, can be turned off) that shows a dismissible notice when a newer version exists. It reads the latest published release (falling back to the public releases page if the API rate limit is reached) and never downloads or installs anything.
- `printsearch --version`.
- This changelog, in English and French.

### Changed
- Results are merged client-side (sites interleaved, then sorted by likes or downloads when requested). Result cards already on screen are reused: no flicker, and their entrance animation is no longer replayed. The same model appearing twice is shown once.
- A single HTTP client is shared by the whole program (connection reuse) and now has timeouts: 8 s to connect, 25 s per request.
- The page cache of the browser reader is capped (40 entries) and expired entries are purged.
- HTTPS connections now also trust the operating system's certificate store (not only the bundled one), so the app works behind company proxies and antivirus programs that inspect HTTPS.
- The configuration file is written atomically (temporary file then rename).
- The `.deb` package now recommends Edge and Brave as well as Chromium and Chrome.
- The maintainer and author of the package are now **Print-lab**.
- READMEs updated (browser choice, Windows icon, cross-compilation from Linux, `rustfmt` requirement for building).

### Security
- The local server now rejects any request whose `Host` is not `localhost:8787` / `127.0.0.1:8787`, and any request with a foreign `Origin`. This protects against malicious web pages opened in your browser (CSRF and DNS rebinding), which could otherwise have called the local API, for example to change the browser executable.

### Fixed
- **Hang when the selected browser is not a real browser** (wrong path, non-Chromium program): the launch could block forever and freeze later searches. Launch now fails after 35 s with a clear message, a failed launch is remembered for 20 s instead of being retried in a loop, each browser call has a 10 s timeout, and a page read is capped at 75 s.
- A search started with no site selected could still display the results of the previous search.
- A panic in one thread can no longer permanently disable the browser reader (poisoned locks are recovered).
- Starting a second instance now reports a clear message when port 8787 is already in use.

### Notes
- Firefox and Safari cannot be used for searches (the reader needs a Chromium-based browser).
- Building requires `rustfmt` (included in a normal `rustup` install) because of the `headless_chrome` dependency.
- Unchanged limitations: the Printables and MakerWorld endpoints are unofficial, and Like / collections remain experimental (see the README).

## [0.1.0]

Initial release.

### Added
- Search across **Thingiverse, Printables and MakerWorld** from a local web interface and a command-line mode.
- Filters (sites, price, sort, minimum likes and downloads, results per site) with automatic re-search, pagination ("Load more") and animations.
- Like and add-to-collection actions (experimental), connection to sites from a Settings page (Thingiverse App Token or OAuth, Printables Bearer token).
- English and French interface (English by default) and a search history that can be turned off.
- Packaging: `.deb` metadata, launcher and icon; Windows build. Released under the GPL v3.

[0.3.0]: https://github.com/Print-lab/PrintSearch/releases/tag/v0.3.0
[0.2.0]: https://github.com/Print-lab/PrintSearch/releases/tag/v0.2.0
[0.1.0]: https://github.com/Print-lab/PrintSearch/releases/tag/v0.1.0
