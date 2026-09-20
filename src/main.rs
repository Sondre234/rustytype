use std::{
    io::{self, Write},
    panic,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{
        self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};

const RUST_SNIPPETS: &[(&str, &str)] = &[
    (
        "ownership",
        "fn longest<'a>(left: &'a str, right: &'a str) -> &'a str {\n    if left.len() >= right.len() { left } else { right }\n}",
    ),
    (
        "iterator",
        "let total: i32 = values\n    .iter()\n    .filter(|value| **value > 0)\n    .map(|value| value * 2)\n    .sum();",
    ),
    (
        "result",
        "fn parse_port(input: &str) -> Result<u16, String> {\n    input.parse().map_err(|error| format!(\"invalid port: {error}\"))\n}",
    ),
    (
        "structs",
        "#[derive(Debug, Clone)]\nstruct User {\n    name: String,\n    active: bool,\n}\n\nimpl User {\n    fn new(name: impl Into<String>) -> Self {\n        Self { name: name.into(), active: true }\n    }\n}",
    ),
    (
        "match",
        "let description = match status {\n    Status::Ready => \"ready\",\n    Status::Waiting(seconds) if seconds > 10 => \"delayed\",\n    Status::Waiting(_) => \"waiting\",\n};",
    ),
    (
        "async",
        "async fn fetch(client: &Client, url: &str) -> anyhow::Result<String> {\n    let response = client.get(url).send().await?;\n    Ok(response.text().await?)\n}",
    ),
    (
        "rusttype / screen",
        "#[derive(Clone, Copy, Debug, PartialEq)]\nenum Screen {\n    Typing,\n    Results,\n}",
    ),
    (
        "rusttype / reset",
        "fn reset(&mut self, next: bool) {\n    if next {\n        self.snippet = (self.snippet + 1) % SNIPPETS.len();\n    }\n    self.typed.clear();\n    self.started = None;\n    self.screen = Screen::Typing;\n}",
    ),
    (
        "rusttype / terminal",
        "impl Drop for TerminalGuard {\n    fn drop(&mut self) {\n        let _ = disable_raw_mode();\n        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen, ResetColor);\n    }\n}",
    ),
    (
        "rusttype / input",
        "match key.code {\n    KeyCode::Backspace => {\n        app.typed.pop();\n        app.automatic.pop();\n    }\n    KeyCode::Char(ch) => app.push(ch),\n    _ => {}\n}",
    ),
    (
        "rusttype / render",
        "queue!(\n    out,\n    MoveTo(left, title_y),\n    SetForegroundColor(Color::Cyan),\n    SetAttribute(Attribute::Bold),\n    Print(\"rusttype\"),\n    ResetColor,\n)?;",
    ),
];

const CPP_SNIPPETS: &[(&str, &str)] = &[
    (
        "references",
        "const std::string& longest(const std::string& left, const std::string& right) {\n    return left.size() >= right.size() ? left : right;\n}",
    ),
    (
        "algorithm",
        "std::vector<int> positive;\nstd::copy_if(values.begin(), values.end(),\n    std::back_inserter(positive), [](int value) { return value > 0; });",
    ),
    (
        "optional",
        "std::optional<int> parse_port(const std::string& input) {\n    try {\n        return std::stoi(input);\n    } catch (const std::exception&) {\n        return std::nullopt;\n    }\n}",
    ),
    (
        "class",
        "class User {\npublic:\n    explicit User(std::string name)\n        : name_(std::move(name)), active_(true) {}\n\nprivate:\n    std::string name_;\n    bool active_;\n};",
    ),
    (
        "range loop",
        "for (const auto& item : items) {\n    if (item.is_ready()) {\n        std::cout << item.name() << '\\n';\n    }\n}",
    ),
    (
        "smart pointer",
        "auto widget = std::make_unique<Widget>(42);\nif (widget) {\n    widget->render();\n}",
    ),
    (
        "template",
        "template <typename T>\nT clamp(T value, T low, T high) {\n    return std::min(std::max(value, low), high);\n}",
    ),
    (
        "lambda",
        "auto total = std::accumulate(values.begin(), values.end(), 0,\n    [](int sum, int value) { return sum + value; });",
    ),
];

const C_SNIPPETS: &[(&str, &str)] = &[
    (
        "pointers",
        "const char *longest(const char *left, const char *right) {\n    return strlen(left) >= strlen(right) ? left : right;\n}",
    ),
    (
        "array loop",
        "int total = 0;\nfor (size_t i = 0; i < length; i++) {\n    if (values[i] > 0) {\n        total += values[i] * 2;\n    }\n}",
    ),
    (
        "parse integer",
        "char *end;\nlong value = strtol(input, &end, 10);\nif (*end != '\\0') {\n    fprintf(stderr, \"invalid number: %s\\n\", input);\n}",
    ),
    (
        "struct",
        "struct user {\n    char name[64];\n    bool active;\n};\n\nstruct user user_create(const char *name) {\n    struct user result = { .active = true };\n    snprintf(result.name, sizeof result.name, \"%s\", name);\n    return result;\n}",
    ),
    (
        "enum and switch",
        "const char *status_name(enum status value) {\n    switch (value) {\n    case STATUS_READY:\n        return \"ready\";\n    case STATUS_WAITING:\n        return \"waiting\";\n    default:\n        return \"unknown\";\n    }\n}",
    ),
    (
        "allocation",
        "int *values = malloc(count * sizeof *values);\nif (values == NULL) {\n    return EXIT_FAILURE;\n}\n\nfree(values);",
    ),
    (
        "function pointer",
        "int apply(int value, int (*operation)(int)) {\n    return operation(value);\n}",
    ),
    (
        "linked list",
        "struct node {\n    int value;\n    struct node *next;\n};\n\nfor (struct node *node = head; node != NULL; node = node->next) {\n    printf(\"%d\\n\", node->value);\n}",
    ),
];

const JAVA_SNIPPETS: &[(&str, &str)] = &[
    (
        "streams",
        "int total = values.stream()\n    .filter(value -> value > 0)\n    .mapToInt(value -> value * 2)\n    .sum();",
    ),
    (
        "optional",
        "Optional<Integer> parsePort(String input) {\n    try {\n        return Optional.of(Integer.parseInt(input));\n    } catch (NumberFormatException error) {\n        return Optional.empty();\n    }\n}",
    ),
    (
        "record",
        "record User(String name, boolean active) {\n    User(String name) {\n        this(name, true);\n    }\n}",
    ),
    (
        "switch expression",
        "String description = switch (status) {\n    case READY -> \"ready\";\n    case WAITING -> \"waiting\";\n    default -> \"unknown\";\n};",
    ),
    (
        "enhanced loop",
        "for (var item : items) {\n    if (item.isReady()) {\n        System.out.println(item.name());\n    }\n}",
    ),
    (
        "generics",
        "static <T extends Comparable<T>> T max(T left, T right) {\n    return left.compareTo(right) >= 0 ? left : right;\n}",
    ),
    (
        "try with resources",
        "try (var reader = Files.newBufferedReader(path)) {\n    return reader.lines().toList();\n} catch (IOException error) {\n    throw new UncheckedIOException(error);\n}",
    ),
    (
        "lambda",
        "var names = users.stream()\n    .filter(User::active)\n    .map(User::name)\n    .sorted()\n    .toList();",
    ),
];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Language {
    Rust,
    C,
    Cpp,
    Java,
}

impl Language {
    fn from_args<'a>(args: impl IntoIterator<Item = &'a str>) -> Self {
        let args: Vec<&str> = args.into_iter().collect();
        if args.iter().any(|arg| matches!(*arg, "--java" | "-j")) {
            Self::Java
        } else if args.contains(&"--c") {
            Self::C
        } else if args.iter().any(|arg| matches!(*arg, "--cpp" | "-c")) {
            Self::Cpp
        } else {
            Self::Rust
        }
    }

    fn snippets(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Rust => RUST_SNIPPETS,
            Self::C => C_SNIPPETS,
            Self::Cpp => CPP_SNIPPETS,
            Self::Java => JAVA_SNIPPETS,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::C => "c",
            Self::Cpp => "c++",
            Self::Java => "java",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Screen {
    Typing,
    Results,
}

struct App {
    language: Language,
    snippet: usize,
    typed: Vec<char>,
    automatic: Vec<bool>,
    started: Option<Instant>,
    finished_in: Option<Duration>,
    screen: Screen,
}

impl App {
    fn new(language: Language) -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        Self {
            language,
            snippet: seed % language.snippets().len(),
            typed: Vec::new(),
            automatic: Vec::new(),
            started: None,
            finished_in: None,
            screen: Screen::Typing,
        }
    }

    fn target(&self) -> &'static str {
        self.language.snippets()[self.snippet].1
    }

    fn target_chars(&self) -> Vec<char> {
        self.target().chars().collect()
    }

    fn reset(&mut self, next: bool) {
        if next {
            self.snippet = (self.snippet + 1) % self.language.snippets().len();
        }
        self.typed.clear();
        self.automatic.clear();
        self.started = None;
        self.finished_in = None;
        self.screen = Screen::Typing;
    }

    fn push(&mut self, ch: char) {
        if self.screen != Screen::Typing || self.typed.len() >= self.target().chars().count() {
            return;
        }
        self.started.get_or_insert_with(Instant::now);
        self.typed.push(ch);
        self.automatic.push(false);
        self.advance_layout();
        self.finish_if_complete();
    }

    fn advance_layout(&mut self) {
        let target = self.target_chars();
        while self.typed.len() < target.len() {
            let index = self.typed.len();
            let at_line_start = index == 0 || target[index - 1] == '\n';
            if target[index] == '\n' {
                self.typed.push('\n');
                self.automatic.push(true);
            } else if at_line_start && target[index] == ' ' {
                while self.typed.len() < target.len() && target[self.typed.len()] == ' ' {
                    self.typed.push(' ');
                    self.automatic.push(true);
                }
            } else {
                break;
            }
        }
    }

    fn finish_if_complete(&mut self) {
        if self.typed.len() == self.target().chars().count() {
            self.finished_in = Some(self.started.unwrap().elapsed());
            self.screen = Screen::Results;
        }
    }

    fn elapsed(&self) -> Duration {
        self.finished_in
            .or_else(|| self.started.map(|start| start.elapsed()))
            .unwrap_or_default()
    }

    fn correct(&self) -> usize {
        self.typed
            .iter()
            .zip(self.target().chars())
            .enumerate()
            .filter(|(index, _)| !self.automatic[*index])
            .filter(|(_, (actual, expected))| actual == &expected)
            .count()
    }

    fn user_keystrokes(&self) -> usize {
        self.automatic
            .iter()
            .filter(|&&automatic| !automatic)
            .count()
    }

    fn accuracy(&self) -> f64 {
        let keystrokes = self.user_keystrokes();
        if keystrokes == 0 {
            100.0
        } else {
            self.correct() as f64 / keystrokes as f64 * 100.0
        }
    }

    fn wpm(&self) -> f64 {
        let minutes = self.elapsed().as_secs_f64() / 60.0;
        if minutes == 0.0 {
            0.0
        } else {
            self.correct() as f64 / 5.0 / minutes
        }
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen, ResetColor);
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let language = Language::from_args(args.iter().map(String::as_str));
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen, ResetColor);
        previous_hook(info);
    }));

    let _guard = TerminalGuard::enter()?;
    let mut app = App::new(language);

    loop {
        draw(&app)?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if handle_key(&mut app, key) {
            break;
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    if key.code == KeyCode::Esc
        || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
    {
        return true;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
        app.reset(false);
        return false;
    }
    if app.screen == Screen::Results {
        match key.code {
            KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char(' ') => app.reset(true),
            _ => {}
        }
        return false;
    }
    match key.code {
        KeyCode::Backspace => {
            while app.automatic.last() == Some(&true) {
                app.typed.pop();
                app.automatic.pop();
            }
            app.typed.pop();
            app.automatic.pop();
            if app.typed.is_empty() {
                app.started = None;
            }
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => app.push(ch),
        _ => {}
    }
    false
}

fn draw(app: &App) -> io::Result<()> {
    let mut out = io::stdout();
    let (width, height) = terminal::size()?;
    queue!(out, MoveTo(0, 0), Clear(ClearType::All))?;

    let content_width = width.saturating_sub(8).min(92);
    let left = width.saturating_sub(content_width) / 2;
    let title_y = 2;
    queue!(
        out,
        MoveTo(left, title_y),
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print("rusttype"),
        SetAttribute(Attribute::Reset),
        SetForegroundColor(Color::DarkGrey),
        Print(format!(
            "  / {} / {}",
            app.language.label(),
            app.language.snippets()[app.snippet].0
        )),
        ResetColor,
    )?;

    if app.screen == Screen::Results {
        queue!(
            out,
            MoveTo(left, title_y + 2),
            SetForegroundColor(Color::Green),
            Print(format!(
                "{:.0} wpm   {:.1}% accuracy   {:.1}s",
                app.wpm(),
                app.accuracy(),
                app.elapsed().as_secs_f64()
            )),
            ResetColor,
        )?;
    }

    draw_code(&mut out, app, left, title_y + 5, height.saturating_sub(10))?;

    let footer_y = height.saturating_sub(2);
    queue!(
        out,
        MoveTo(left, footer_y),
        SetForegroundColor(Color::DarkGrey)
    )?;
    if app.screen == Screen::Results {
        queue!(
            out,
            SetForegroundColor(Color::Green),
            SetAttribute(Attribute::Bold),
            Print("complete!"),
            SetAttribute(Attribute::Reset),
            SetForegroundColor(Color::DarkGrey),
            Print("  enter/n next   ctrl-r retry   esc quit")
        )?;
    } else {
        queue!(
            out,
            Print("newlines + indentation are automatic   ctrl-r restart   esc quit")
        )?;
    }
    queue!(out, ResetColor)?;
    out.flush()
}

fn draw_code(
    out: &mut io::Stdout,
    app: &App,
    left: u16,
    top: u16,
    max_rows: u16,
) -> io::Result<()> {
    let target: Vec<char> = app.target().chars().collect();
    let mut row = 0_u16;
    let mut col = 0_u16;
    let mut line_no = 1;
    queue!(
        out,
        MoveTo(left, top),
        SetForegroundColor(Color::DarkGrey),
        Print(format!("{line_no:>2}  "))
    )?;

    for (index, ch) in target.iter().copied().enumerate() {
        if row >= max_rows {
            break;
        }
        if ch == '\n' {
            row += 1;
            col = 0;
            line_no += 1;
            if row < max_rows {
                queue!(
                    out,
                    MoveTo(left, top + row),
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!("{line_no:>2}  "))
                )?;
            }
            continue;
        }
        queue!(out, MoveTo(left + 4 + col, top + row))?;
        if let Some(actual) = app.typed.get(index) {
            if *actual == ch {
                queue!(out, SetForegroundColor(Color::Green), Print(ch))?;
            } else {
                let shown = if *actual == '\n' { '↵' } else { *actual };
                queue!(
                    out,
                    SetForegroundColor(Color::Red),
                    SetAttribute(Attribute::Underlined),
                    Print(shown),
                    SetAttribute(Attribute::NoUnderline)
                )?;
            }
        } else if index == app.typed.len() && app.screen == Screen::Typing {
            queue!(
                out,
                SetForegroundColor(Color::Black),
                SetBackgroundColor(Color::Cyan),
                Print(ch),
                ResetColor
            )?;
        } else {
            queue!(
                out,
                SetForegroundColor(syntax_color(&target, index, app.language)),
                Print(ch)
            )?;
        }
        col += 1;
    }
    queue!(out, ResetColor)
}

use crossterm::style::SetBackgroundColor;

fn syntax_color(chars: &[char], index: usize, language: Language) -> Color {
    let ch = chars[index];
    if ch == '"'
        || chars[..index]
            .iter()
            .rev()
            .take_while(|&&c| c != '\n')
            .filter(|&&c| c == '"')
            .count()
            % 2
            == 1
    {
        return Color::Yellow;
    }
    if ch.is_ascii_digit() {
        return Color::Magenta;
    }
    let start = chars[..index]
        .iter()
        .rposition(|c| !c.is_alphanumeric() && *c != '_')
        .map_or(0, |position| position + 1);
    let end = chars[index..]
        .iter()
        .position(|c| !c.is_alphanumeric() && *c != '_')
        .map_or(chars.len(), |offset| index + offset);
    let word: String = chars[start..end].iter().collect();
    let keyword = match language {
        Language::Rust => matches!(
            word.as_str(),
            "fn" | "let"
                | "mut"
                | "struct"
                | "enum"
                | "impl"
                | "match"
                | "if"
                | "else"
                | "async"
                | "await"
                | "move"
                | "pub"
                | "use"
                | "return"
                | "Self"
                | "self"
        ),
        Language::C => matches!(
            word.as_str(),
            "auto"
                | "bool"
                | "break"
                | "case"
                | "char"
                | "const"
                | "continue"
                | "default"
                | "do"
                | "double"
                | "else"
                | "enum"
                | "extern"
                | "false"
                | "float"
                | "for"
                | "if"
                | "inline"
                | "int"
                | "long"
                | "register"
                | "restrict"
                | "return"
                | "short"
                | "signed"
                | "sizeof"
                | "static"
                | "struct"
                | "switch"
                | "true"
                | "typedef"
                | "union"
                | "unsigned"
                | "void"
                | "volatile"
                | "while"
        ),
        Language::Cpp => matches!(
            word.as_str(),
            "alignas"
                | "auto"
                | "bool"
                | "break"
                | "case"
                | "catch"
                | "class"
                | "const"
                | "constexpr"
                | "continue"
                | "default"
                | "delete"
                | "do"
                | "else"
                | "enum"
                | "explicit"
                | "false"
                | "for"
                | "friend"
                | "if"
                | "namespace"
                | "new"
                | "noexcept"
                | "nullptr"
                | "private"
                | "protected"
                | "public"
                | "return"
                | "sizeof"
                | "static"
                | "struct"
                | "switch"
                | "template"
                | "this"
                | "throw"
                | "true"
                | "try"
                | "typename"
                | "using"
                | "virtual"
                | "void"
                | "while"
        ),
        Language::Java => matches!(
            word.as_str(),
            "abstract"
                | "boolean"
                | "break"
                | "case"
                | "catch"
                | "class"
                | "continue"
                | "default"
                | "do"
                | "else"
                | "enum"
                | "extends"
                | "false"
                | "final"
                | "finally"
                | "for"
                | "if"
                | "implements"
                | "import"
                | "instanceof"
                | "interface"
                | "new"
                | "null"
                | "package"
                | "private"
                | "protected"
                | "public"
                | "record"
                | "return"
                | "static"
                | "super"
                | "switch"
                | "this"
                | "throw"
                | "throws"
                | "true"
                | "try"
                | "var"
                | "void"
                | "while"
        ),
    };
    if keyword {
        Color::Cyan
    } else if match language {
        Language::Rust => matches!(
            word.as_str(),
            "String"
                | "Result"
                | "Option"
                | "Some"
                | "None"
                | "Ok"
                | "Err"
                | "str"
                | "bool"
                | "i32"
                | "u16"
        ),
        Language::C => matches!(
            word.as_str(),
            "FILE" | "NULL" | "ptrdiff_t" | "size_t" | "stderr" | "stdin" | "stdout"
        ),
        Language::Cpp => matches!(
            word.as_str(),
            "char"
                | "double"
                | "float"
                | "int"
                | "long"
                | "short"
                | "signed"
                | "size_t"
                | "string"
                | "unsigned"
                | "vector"
        ),
        Language::Java => matches!(
            word.as_str(),
            "String"
                | "Integer"
                | "Long"
                | "Double"
                | "Optional"
                | "List"
                | "Map"
                | "Set"
                | "Object"
                | "System"
        ),
    } {
        Color::Blue
    } else {
        Color::Grey
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_count_only_correct_characters() {
        let mut app = App::new(Language::Rust);
        app.snippet = 0;
        app.typed = app.target().chars().take(4).collect();
        app.automatic = vec![false; 4];
        app.typed[2] = 'x';
        assert_eq!(app.correct(), 3);
        assert_eq!(app.accuracy(), 75.0);
    }

    #[test]
    fn completing_target_opens_results() {
        let mut app = App::new(Language::Rust);
        app.snippet = 0;
        let target: Vec<char> = app.target().chars().collect();
        while app.screen == Screen::Typing {
            let ch = target[app.typed.len()];
            app.push(ch);
        }
        assert_eq!(app.screen, Screen::Results);
        assert!(app.finished_in.is_some());
    }

    #[test]
    fn newlines_and_indentation_are_automatic() {
        let mut app = App::new(Language::Rust);
        app.snippet = 0;
        let newline = app.target().find('\n').unwrap();
        for ch in app.target().chars().take(newline).collect::<Vec<_>>() {
            app.push(ch);
        }
        assert!(app.typed.ends_with(&['\n', ' ', ' ', ' ', ' ']));
        assert!(
            app.automatic
                .iter()
                .rev()
                .take(5)
                .all(|automatic| *automatic)
        );
    }

    #[test]
    fn cpp_mode_uses_cpp_snippets_and_highlighting() {
        let mut app = App::new(Language::Cpp);
        app.snippet = 0;
        assert!(app.target().contains("std::string"));

        let chars: Vec<char> = "const int value".chars().collect();
        assert_eq!(syntax_color(&chars, 0, Language::Cpp), Color::Cyan);
        assert_eq!(syntax_color(&chars, 6, Language::Cpp), Color::Blue);
    }

    #[test]
    fn c_mode_uses_c_snippets_and_highlighting() {
        let mut app = App::new(Language::C);
        app.snippet = 0;
        assert!(app.target().contains("const char *longest"));

        let chars: Vec<char> = "const size_t length".chars().collect();
        assert_eq!(syntax_color(&chars, 0, Language::C), Color::Cyan);
        assert_eq!(syntax_color(&chars, 6, Language::C), Color::Blue);
    }

    #[test]
    fn java_mode_uses_java_snippets_and_highlighting() {
        let mut app = App::new(Language::Java);
        app.snippet = 0;
        assert!(app.target().contains("values.stream()"));

        let chars: Vec<char> = "public String value".chars().collect();
        assert_eq!(syntax_color(&chars, 0, Language::Java), Color::Cyan);
        assert_eq!(syntax_color(&chars, 7, Language::Java), Color::Blue);
    }

    #[test]
    fn startup_flags_select_the_language() {
        assert_eq!(Language::from_args([]), Language::Rust);
        assert_eq!(Language::from_args(["--c"]), Language::C);
        assert_eq!(Language::from_args(["--cpp"]), Language::Cpp);
        assert_eq!(Language::from_args(["-c"]), Language::Cpp);
        assert_eq!(Language::from_args(["--java"]), Language::Java);
        assert_eq!(Language::from_args(["-j"]), Language::Java);
    }
}
