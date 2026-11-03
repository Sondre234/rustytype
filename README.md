# rusttype

A small Monkeytype-style trainer for typing real Rust, with plenty of `::<>`.
Race the clock on short snippets in your terminal.

## Play

```sh
cargo run --release
```

Closing brackets and quotes fill themselves in like an editor's autoclose:
type `(` and the `)` is already there, and the cursor steps over it when you
reach it. To type every character yourself:

```sh
cargo run --release -- --classic
```

## Controls

| key | |
| --- | --- |
| type | start the timer; newlines, indentation and closing brackets are filled in for you |
| `Backspace` | fix a mistake |
| `Ctrl-R` | restart the snippet |
| `Enter` / `n` / `Space` | next snippet (after finishing) |
| `Esc` / `Ctrl-C` | quit |

Stats stay hidden until you finish a snippet.

## Other languages

C, C++, Java and Go live on the `multi-language` branch.
