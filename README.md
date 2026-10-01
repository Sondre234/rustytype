# rusttype

A tiny Monkeytype-inspired trainer for typing real Rust (lots of `::<>`),
with a GitHub-backed leaderboard. Play it locally, or over SSH with nothing
installed:

```sh
ssh <your-github-login>@HOST     # scores saved, key checked against github.com/<login>.keys
ssh guest@HOST                   # try it, nothing saved
```

Newlines and leading indentation are typed for you, stats stay hidden until the
snippet is done. `Backspace` fixes a mistake, `Ctrl-R` retries, `Esc`/`Ctrl-C`
quits, and after finishing `Enter`/`n`/`Space` is the next snippet and `l` opens the
leaderboard: `Tab` flips between the global board (each row shows which snippet that
player's best run was on) and the board for the snippet you just typed.

Other languages (C, C++, Java, Go) live on the `multi-language` branch.

## Local client

```sh
cargo run --release -p rusttype                  # play
cargo run --release -p rusttype -- login         # GitHub device flow, then scores auto-submit
cargo run --release -p rusttype -- logout
```

The server comes from `--server URL`, `$RUSTTYPE_SERVER`, or your saved login.
The login token lives in `~/.config/rusttype/credentials.json` (mode 0600).

## How it works

```
crates/core    snippets, typing state, renderer (any io::Write), replay verifier
crates/client  `rusttype`: crossterm TUI, `login` (GitHub device flow), submit
crates/server  `rusttype-server`: axum API + SSH server + SQLite
```

- **Scores are derived, not claimed.** Clients send the keystroke timeline;
  the server replays it against its own copy of the snippet and computes WPM and
  accuracy itself. It rejects runs over 250 wpm, bursts of keystrokes closer than
  8 ms, incomplete runs, and submissions faster than the run could have been
  typed. A determined cheater can still script human-looking timings; this
  stops casual forgery, not a dedicated bot.
- **Ranking:** each player's best run with at least 90% accuracy, by WPM. Two boards:
  global (best run across all snippets, tagged with its snippet) and per snippet.
  `GET /api/leaderboard[?snippet=<title>][&limit=N]` serves both; after each run the
  server reports your rank on both.
- **SSH login = GitHub login.** The SSH username is the GitHub login and the key
  you offer must be one of that account's public keys. No passwords, no OAuth
  needed over SSH. The GitHub numeric id is the account key, so renames are safe.
- **Local login** uses GitHub's device flow; the server checks the token was
  issued to *this* OAuth app (`POST /applications/{id}/token`) before trusting it.

## Running the server

1. Create a GitHub OAuth App (Settings -> Developer settings -> OAuth Apps),
   tick **Enable Device Flow**. The callback URL is unused; put anything.
2. Run it:

```sh
GITHUB_CLIENT_ID=... GITHUB_CLIENT_SECRET=... \
RUSTTYPE_PUBLIC_HOST=rusttype.example.com \
RUSTTYPE_DATA_DIR=/var/lib/rusttype \
cargo run --release -p rusttype-server
```

| variable | default | |
| --- | --- | --- |
| `GITHUB_CLIENT_ID` / `GITHUB_CLIENT_SECRET` | required | OAuth app credentials |
| `RUSTTYPE_DATA_DIR` | `.` | holds `rusttype.db` and the SSH host key (keep it, or every client sees a host-key change) |
| `RUSTTYPE_HTTP_ADDR` | `127.0.0.1:8080` | put Caddy/nginx in front for HTTPS |
| `RUSTTYPE_SSH_ADDR` | `0.0.0.0:2222` | to serve on port 22, move your admin sshd first or redirect 22 to 2222 |
| `RUSTTYPE_PUBLIC_HOST` | `localhost` | shown in the web page and connect hints |

Then set `DEFAULT_SERVER` in `crates/client/src/api.rs` to your URL before
publishing the client.
