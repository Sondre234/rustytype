# rusttype

A tiny Monkeytype-inspired terminal trainer for practicing Rust syntax on a new
keyboard.

## Run

```sh
cargo run --release
```

To practice C++ syntax instead, pass `--cpp` (or `-c`) after Cargo's argument
separator:

```sh
cargo run --release -- --cpp
```

For C syntax, pass `--c` (`-c` remains the C++ shortcut):

```sh
cargo run --release -- --c
```

For Java syntax, pass `--java` (or `-j`):

```sh
cargo run --release -- --java
```

Start typing to begin the timer. Newlines and leading indentation are inserted
automatically, and performance stats stay hidden until the exercise is complete.
Use `Backspace` to fix a mistake, `Ctrl-R` to retry, and `Esc` or `Ctrl-C` to
quit. After completing a snippet, press `Enter`, `Space`, or `n` for the next one.
