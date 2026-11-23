# PrintSearch

**English** · [Français](README.fr.md)

One search box for 3D-printing models on **Thingiverse**, **Printables** and **MakerWorld**.
A small Rust application with a local web interface and a command-line mode: search the three sites at once, filter and sort the results, like a model or add it to a collection, and keep your recent searches. This was coded by Claude to ensure complete transparency; I could never have done this on my own—it would have taken me too long, and I don't have enough knowledge.

![PrintSearch screenshot 1](printsearch.png)

> **Not affiliated with Thingiverse, Printables or MakerWorld.** Names and logos belong to their respective owners and are used only to identify each source. See [Limitations & legal notes](#limitations--legal-notes).

<!-- Add a screenshot here: ![PrintSearch](docs/screenshot.png) -->

## Features

- Search **Thingiverse, Printables and MakerWorld** in parallel; results are interleaved by site.
- Filters: **sites, price (free / paid), sort (relevance / likes / downloads), minimum likes, minimum downloads, results per site**. Changing a filter re-runs the search automatically.
- **Load more results** pagination, with a short-lived cache so repeated searches are fast.
- **Like** and **add to collection** from the result cards (see [feature support](#feature-support)).
- **Search history** under the search bar (can be turned off) and a **language setting** (English by default; French and German available).
- Local by default: the server listens on `127.0.0.1` and refuses requests that do not come from `localhost:8787` (protection against malicious web pages); your tokens never leave your computer. See [Access from another machine](#access-from-another-machine-optional) to change this.
- Also usable from the **command line**.

## Requirements

| What | Why |
|---|---|
| [Rust](https://rustup.rs) (stable) | to build from source |
| A **Chromium-based browser**: Chrome, **Edge**, Brave, Vivaldi or Chromium | MakerWorld has no usable public API, so it is read with a headless browser. Thingiverse uses it as a fallback when you have no token. **Windows already ships with Edge**, so there is usually nothing to install. Firefox and Safari are not supported. You choose which one in ⚙ Settings. |
| `rustfmt` (included in a normal `rustup` install) | needed to build the `headless_chrome` dependency; if you installed Rust with the *minimal* profile, run `rustup component add rustfmt` |
| Windows only: *Visual Studio Build Tools* ("Desktop development with C++") | required by Rust on Windows |

No OpenSSL needed (the project uses `rustls`).

## Installation

### From source (Linux, macOS, Windows)

```bash
git clone https://github.com/Print-lab/PrintSearch.git
cd PrintSearch
cargo build --release
```

The binary is `target/release/printsearch` (`target\release\printsearch.exe` on Windows). The web interface and the icons are embedded in it: it is a single file.

### Debian / Ubuntu package (`.deb`)

```bash
cargo install cargo-deb
cargo deb
sudo apt install ./target/debian/printsearch_*_amd64.deb
```

A Chromium-based browser (Chromium, Chrome, Edge or Brave) is *recommended* by the package, so `apt` will offer to install one. After installation, "PrintSearch" appears in your application menu and `printsearch` is available in the terminal. Before distributing the package, fill in `maintainer` in `Cargo.toml`.

### Headless Linux server (systemd)

PrintSearch runs fine on a server without a screen. Packages needed (Debian / Ubuntu):

```
sudo apt install -y git curl build-essential chromium   # Ubuntu: chromium-browser
```

- `git`, `curl`, `build-essential` (C compiler and linker): to fetch and build the sources.
- A Chromium-based browser, used headless in the background. The package is called `chromium` on Debian and `chromium-browser` on Ubuntu (which may install a snap); Google Chrome, Brave or Edge from their own `.deb` packages also work. If none is detected, pick one with `printsearch browser <name or /path/to/browser>`.
- Rust (stable) through [rustup](https://rustup.rs/) (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`, then `source "$HOME/.cargo/env"`). `rustfmt` is part of the default profile.

Build, then run `printsearch serve` (not plain `printsearch`, which tries to open a browser window):

```
git clone https://github.com/Print-lab/PrintSearch.git
cd PrintSearch/printsearch
cargo build --release
./target/release/printsearch serve
```

To keep it running after you log out and start it at boot, create `/etc/systemd/system/printsearch.service` (adjust `User` and the paths):

```
[Unit]
Description=PrintSearch
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=youruser
WorkingDirectory=/opt/PrintSearch/printsearch
ExecStart=/opt/PrintSearch/printsearch/target/release/printsearch serve
# Optional, to reach it from your network (see "Access from another machine"):
# Environment=PRINTSEARCH_BIND=192.168.1.10
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```
sudo systemctl daemon-reload
sudo systemctl enable --now printsearch
journalctl -u printsearch -f          # logs
sudo ss -tlnp | grep 8787             # check that the port is listening
```

Run the service as the user whose home holds the configuration (`~/.config/printsearch/`), so saved tokens are found. After rebuilding, run `sudo systemctl restart printsearch`.

### Windows executable

The `.exe` embeds the application icon. 
Build on Windows with `cargo build --release`, then copy `target\release\printsearch.exe` anywhere. A console window stays open while the app runs: closing it stops the server. Windows SmartScreen or the firewall may show a prompt on first launch (the server only listens locally, you can allow it).

### Cross-compiling the Windows `.exe` from Linux

```bash
sudo apt install mingw-w64
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

The icon is embedded with `windres` (included in `mingw-w64`); if it is missing, the build still succeeds, just without the icon (a warning is printed).

## Usage

### Web interface

![PrintSearch screenshot 2](printsearch-2.png)

Run `printsearch` with no argument: it starts the local server and opens your browser on **http://localhost:8787**.

```bash
printsearch            # starts the server and opens the browser
printsearch serve      # starts the server only (add --open to open the browser)
```

Type a query, press Enter. Use the filter row under the search bar; the ⚙ button (top right) opens the **Settings** page.

### Command line

```bash
printsearch search "benchy"
printsearch search "headphone stand" --sites thingiverse,printables -l 5
printsearch search "vase" --price free --sort likes --min-likes 100 --page 2

printsearch login thingiverse                      # OAuth (see below)
printsearch login printables --token <TOKEN>
printsearch like thingiverse 763622
printsearch collect thingiverse <COLLECTION_ID> 763622
```

`-l/--limit` is the number of results *per site* and *per page*; `--page` starts at 1.

### Choosing the browser used for searches

⚙ **Settings → Browser used for searches**: *Automatic* (first one found, in the order Chrome, Edge, Brave, Vivaldi, Chromium), a detected browser, or a **custom path** to any Chromium-based executable. From the command line:

```bash
printsearch browser                 # lists detected browsers and the current choice
printsearch browser edge            # chrome | edge | brave | vivaldi | chromium | auto | /path/to/browser
```

The browser runs in the background (no window), with images disabled for speed; your own browsing is untouched.

### Access from another machine (optional)

By default the server only listens on `127.0.0.1`. To reach it from another machine (for example a headless Linux server), set the address to listen on **before** starting it:

```
PRINTSEARCH_BIND=192.168.1.10 printsearch serve
```

Requests are only accepted when their `Host` is `localhost`, `127.0.0.1` or the bind address. If you use a host name, or bind to `0.0.0.0`, list the names you will type in the browser:

```
PRINTSEARCH_BIND=0.0.0.0 PRINTSEARCH_ALLOWED_HOSTS=myserver,192.168.1.10 printsearch serve
```

Notes: entries of `PRINTSEARCH_ALLOWED_HOSTS` are host names or IP addresses without a port (lower-case); the port is always 8787 and only plain `http://` is accepted (no TLS reverse proxy). With a wildcard bind (`0.0.0.0`) the server still prints `http://localhost:8787`: use the server's name or IP address in your browser. While `PRINTSEARCH_BIND` is a non-loopback address (for example `0.0.0.0` or a LAN address), a custom browser path cannot be saved from the web interface (use `printsearch browser <path>` on the server).

⚠ There is no authentication. Anyone who can reach that address can use PrintSearch and change the saved tokens, and what you type (tokens included) travels unencrypted because the connection is plain HTTP. Only do this on a trusted network, restrict the port with a firewall, or use an SSH tunnel instead (`ssh -L 8787:localhost:8787 user@server`).

## Connecting your accounts

**Searching works without any account** on Printables and MakerWorld. Connecting is only needed for actions on your account (like, collections) and, on Thingiverse, for faster and richer search.

Everything is done in **⚙ Settings → Site connections**: each site has a status badge and a field to paste a token. Click **Save token**; use **Disconnect** to remove it.

### Thingiverse

Without a token, search reads the website with a browser running in the background (slower; no author, likes or downloads). With a token it uses the official API: fast, with author, likes and downloads. A token is **required for Like and collections**.

**Option A, App Token (simplest).**
1. Sign in at <https://www.thingiverse.com/developers> and create an app.
2. Copy its **App Token**.
3. Paste it in Settings → Thingiverse → *Save token*.

**Option B, OAuth login.**
1. Create an app at <https://www.thingiverse.com/developers> and set the redirect URI to `http://localhost:8787/callback`.
2. Set your credentials **before** starting the app:

   ```bash
   # Linux / macOS
   export THINGIVERSE_CLIENT_ID="..."
   export THINGIVERSE_CLIENT_SECRET="..."
   printsearch serve
   ```
   ```powershell
   # Windows PowerShell
   $env:THINGIVERSE_CLIENT_ID="..."; $env:THINGIVERSE_CLIENT_SECRET="..."
   .\printsearch.exe serve
   ```
3. In Settings, a **Connect with Thingiverse** button appears. After authorizing, you come back to the Settings page.

The OAuth button does not appear when the app is launched from a menu or by double-click (no environment variables): use Option A there. The CLI equivalent is `printsearch login thingiverse`.

You can also set `THINGIVERSE_APP_TOKEN` as an environment variable instead of saving a token.

### Printables

Search works without a connection. To use **Like**, paste a Bearer token from your browser session:

1. Sign in on <https://www.printables.com>.
2. Open the developer tools (<kbd>F12</kbd>) → **Network** tab, then reload the page.
3. Click a request to `api.printables.com`; in **Request Headers**, copy the value after `Authorization: Bearer `.
4. Paste it in Settings → Printables → *Save token*.

These tokens **expire**: if Like starts failing, repeat the steps. Treat a token like a password.

### MakerWorld

Search works without a connection (needs a Chromium-based browser). Like and collections are **not implemented** for MakerWorld yet; the field exists in Settings for the future and currently has no effect.

### Where tokens are stored

In a `config.json` inside the application's configuration folder (typically `~/.config/printsearch/` on Linux, `%APPDATA%\printsearch\printsearch\config\` on Windows, `~/Library/Application Support/` on macOS), with owner-only permissions on Unix. Uninstalling does not delete it. **Language and search history** are stored separately, in your browser (`localStorage` for `localhost:8787`).

## Feature support

| | Thingiverse | Printables | MakerWorld |
|---|---|---|---|
| Search without login | ✅ via a browser (slower, limited data) | ✅ | ✅ via a browser (limited data) |
| Search with token | ✅ official API (fast, full data) | n/a | n/a |
| Author / likes / downloads | ✅ with token | ✅ | ❌ |
| Price filter | ✅ always free | ⚠️ best effort | ❌ unknown |
| Like | ⚠️ with token, experimental | ⚠️ with token, experimental | ❌ |
| Add to collection | ⚠️ with token, experimental | ❌ | ❌ |

⚠️ *Experimental*: Printables and MakerWorld endpoints are **not official**, and the Thingiverse like/collection routes were written from the public API description without being verified against a live account. They may need adjusting. Pull requests welcome.

When a value is unknown (for example likes on MakerWorld), the "min likes / min downloads" filters do not exclude the model, and an unknown price is treated as free by the *Free* filter.

## Troubleshooting

- **"No Chromium-based browser found"**: install Chrome, Edge, Brave or Chromium, or pick one (or type its path) in ⚙ Settings → *Browser used for searches*. `printsearch browser` lists what was detected.
- **"The browser did not start within 35 s"**: the browser you selected is probably not Chromium-based (or the path is wrong). Pick another one in Settings.
- **MakerWorld or Thingiverse returns nothing / "no result read"**: the site probably shows an anti-bot check (e.g. a page titled "Just a moment…"), which an automated browser cannot always pass. For Thingiverse, connect with an App Token to use the API instead. The error banner shows the title of the page that was read, which helps to diagnose.
- **Thingiverse is slow without a token**: that is the browser fallback; use an App Token.
- **Printables returns a GraphQL error or nothing**: the unofficial API may have changed; open an issue with the message.
- **Like fails**: your token may have expired or the route may have changed (see *Feature support*).
- **"Address already in use"**: port **8787** is fixed (the Thingiverse OAuth redirect depends on it). Close the other instance.

## Limitations & legal notes

- MakerWorld and Printables are accessed through **unofficial** means (a headless browser and an undocumented GraphQL endpoint). These can change or be blocked at any time, and their terms of service may restrict automated access. Use PrintSearch for **personal use, at a reasonable request rate**, and check each site's terms. You are responsible for how you use it.
- No affiliation with, or endorsement by, Thingiverse (MakerBot/UltiMaker), Printables (Prusa Research) or MakerWorld (Bambu Lab). Trademarks and logos are the property of their owners.
- Results, thumbnails and models belong to their authors; always respect each model's license.

## Project layout

```
src/main.rs            CLI (clap) and entry point
src/server.rs          local web server (axum) + JSON API
src/search.rs          multi-site search, merging, filters, sorting
src/browser.rs         headless browser reader (detection, single shared instance, cache)
src/providers/         one file per site (Thingiverse, Printables, MakerWorld)
src/config.rs          token storage
web/index.html         the whole front-end (embedded in the binary)
web/icons/             site icons (replace to change them, then rebuild)
packaging/             .desktop launcher and icon for the .deb
```

Adding a site means adding a file implementing the `Provider` trait in `src/providers/`.

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## Contributing

[Issues](https://github.com/Print-lab/PrintSearch/issues) and pull requests are welcome, especially for verifying or fixing the experimental endpoints, adding sites, and translations (the `I18N` object in `web/index.html`).

## License

PrintSearch is free software, released under the **GNU General Public License v3.0** (GPL-3.0). See the [LICENSE](LICENSE) file.
