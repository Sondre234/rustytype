# rusttype

A small Monkeytype-style trainer for typing real Rust, with plenty of `::<>`.
Race the clock on short snippets and climb a leaderboard tied to your GitHub
account.

## Play

Over SSH, nothing to install:

```sh
ssh <your-github-login>@HOST    # scores are saved to your GitHub account
ssh guest@HOST                  # just try it, nothing is saved
```

Or run it locally:

```sh
cargo run --release -p rusttype
```

Locally, closing brackets and quotes fill themselves in like an editor's
autoclose: type `(` and the `)` is already there, and the cursor steps over it
when you reach it. To type every character yourself, as over SSH:

```sh
cargo run --release -p rusttype -- --classic
```

## Controls

| key | |
| --- | --- |
| type | start the timer; newlines, indentation and (locally) closing brackets are filled in for you |
| `Backspace` | fix a mistake |
| `Ctrl-R` | restart the snippet |
| `Enter` / `n` / `Space` | next snippet (after finishing) |
| `l` | leaderboard (after finishing) |
| `Tab` | on the leaderboard, switch between global and this snippet |
| `Esc` / `Ctrl-C` | quit |

Stats stay hidden until you finish a snippet.

## Leaderboard

There are two boards: **global**, showing each player's best run and which
snippet it was on, and one **per snippet**. Only runs with at least 90%
accuracy are ranked, and your score is measured from your keystrokes, so it
can't be submitted by hand. Autoclose runs are ranked separately from classic
ones, and the leaderboard screen shows the boards for the mode you're playing.

To save scores from the local client, sign in once with GitHub:

```sh
cargo run --release -p rusttype -- login
cargo run --release -p rusttype -- logout    # forget the saved login
```

Over SSH there is nothing to do: your SSH key is checked against the public
keys on your GitHub account.

## Other languages

C, C++, Java and Go live on the `multi-language` branch.
