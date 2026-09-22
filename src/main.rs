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
        "core / mem::swap",
        "pub const fn swap<T>(x: &mut T, y: &mut T) {\n    unsafe {\n        let mut tmp = MaybeUninit::<T>::uninit();\n        ptr::copy_nonoverlapping(x, tmp.as_mut_ptr(), 1);\n        ptr::copy(y, x, 1);\n        ptr::copy_nonoverlapping(tmp.as_ptr(), y, 1);\n    }\n}",
    ),
    (
        "core / Option::take",
        "pub const fn take(&mut self) -> Option<T> {\n    mem::replace(self, None)\n}",
    ),
    (
        "alloc / Vec::push",
        "pub fn push(&mut self, value: T) {\n    if self.len == self.buf.capacity() {\n        self.buf.grow_one();\n    }\n    unsafe {\n        let end = self.as_mut_ptr().add(self.len);\n        ptr::write(end, value);\n        self.len += 1;\n    }\n}",
    ),
    (
        "alloc / Arc::clone",
        "fn clone(&self) -> Arc<T> {\n    let old_size = self.inner().strong.fetch_add(1, Relaxed);\n    if old_size > MAX_REFCOUNT {\n        abort();\n    }\n    unsafe { Arc::from_inner_in(self.ptr, self.alloc.clone()) }\n}",
    ),
    (
        "core / slice::rotate_left",
        "pub fn rotate_left(&mut self, mid: usize) {\n    assert!(mid <= self.len());\n    let k = self.len() - mid;\n    let p = self.as_mut_ptr();\n    unsafe {\n        ptr::swap_nonoverlapping(p, p.add(mid), cmp::min(mid, k));\n        rotate::ptr_rotate(mid, p.add(mid), k);\n    }\n}",
    ),
    (
        "core / slice::binary_search",
        "pub fn binary_search_by<F>(&self, mut cmp: F) -> Result<usize, usize>\nwhere F: FnMut(&T) -> Ordering {\n    if self.is_empty() { return Err(0); }\n    let mut size = self.len();\n    let mut base = 0;\n    while size > 1 {\n        let half = size / 2;\n        let mid = base + half;\n        base = if cmp(&self[mid]) == Greater { base } else { mid };\n        size -= half;\n    }\n    let result = cmp(&self[base]);\n    if result == Equal { Ok(base) } else { Err(base + (result == Less) as usize) }\n}",
    ),
    (
        "std / thread::spawn",
        "pub fn spawn<F, T>(f: F) -> JoinHandle<T>\nwhere\n    F: FnOnce() -> T + Send + 'static,\n    T: Send + 'static,\n{\n    Builder::new().spawn(f).expect(\"failed to spawn thread\")\n}",
    ),
    (
        "core / Iterator::find",
        "fn find<P>(&mut self, predicate: P) -> Option<Self::Item>\nwhere\n    Self: Sized,\n    P: FnMut(&Self::Item) -> bool,\n{\n    self.try_fold(predicate, check).break_value()\n}",
    ),
];

const CPP_SNIPPETS: &[(&str, &str)] = &[
    (
        "utility / std::move",
        "template <class T>\nconstexpr remove_reference_t<T>&& move(T&& value) noexcept {\n    return static_cast<remove_reference_t<T>&&>(value);\n}",
    ),
    (
        "utility / std::exchange",
        "template <class T, class U = T>\nconstexpr T exchange(T& object, U&& value) {\n    T old = std::move(object);\n    object = std::forward<U>(value);\n    return old;\n}",
    ),
    (
        "memory / make_unique",
        "template <class T, class... Args>\nunique_ptr<T> make_unique(Args&&... args) {\n    return unique_ptr<T>(new T(std::forward<Args>(args)...));\n}",
    ),
    (
        "memory / unique_ptr::reset",
        "void reset(pointer next = pointer()) noexcept {\n    pointer old = ptr_;\n    ptr_ = next;\n    if (old) {\n        get_deleter()(old);\n    }\n}",
    ),
    (
        "algorithm / lower_bound",
        "while (count > 0) {\n    auto step = count / 2;\n    auto middle = first;\n    std::advance(middle, step);\n    if (*middle < value) {\n        first = ++middle;\n        count -= step + 1;\n    } else {\n        count = step;\n    }\n}\nreturn first;",
    ),
    (
        "algorithm / clamp",
        "template <class T, class Compare>\nconstexpr const T& clamp(const T& value, const T& low,\n                         const T& high, Compare comp) {\n    return comp(value, low) ? low : comp(high, value) ? high : value;\n}",
    ),
    (
        "optional / value_or",
        "template <class U>\nconstexpr T value_or(U&& fallback) const& {\n    static_assert(is_copy_constructible_v<T>);\n    static_assert(is_convertible_v<U&&, T>);\n    return has_value() ? **this : static_cast<T>(std::forward<U>(fallback));\n}",
    ),
    (
        "vector / emplace_back",
        "template <class... Args>\nreference emplace_back(Args&&... args) {\n    if (end_ != cap_) {\n        construct_at(end_, std::forward<Args>(args)...);\n        ++end_;\n    } else {\n        grow_and_emplace(std::forward<Args>(args)...);\n    }\n    return back();\n}",
    ),
];

const C_SNIPPETS: &[(&str, &str)] = &[
    (
        "stdlib / malloc",
        "void *malloc(size_t size) {\n    size = align_up(size, alignof(max_align_t));\n    for (struct block *b = free_list; b != NULL; b = b->next) {\n        if (b->free && b->size >= size) {\n            split_block(b, size);\n            b->free = false;\n            return b + 1;\n        }\n    }\n    return grow_heap(size);\n}",
    ),
    (
        "stdlib / free",
        "void free(void *ptr) {\n    if (ptr == NULL) {\n        return;\n    }\n    struct block *block = (struct block *)ptr - 1;\n    block->free = true;\n    coalesce(block);\n}",
    ),
    (
        "string / strlen",
        "size_t strlen(const char *text) {\n    const char *end = text;\n    while (*end != '\\0') {\n        end++;\n    }\n    return (size_t)(end - text);\n}",
    ),
    (
        "string / memcpy",
        "void *memcpy(void *restrict dst, const void *restrict src, size_t count) {\n    unsigned char *out = dst;\n    const unsigned char *in = src;\n    while (count-- > 0) {\n        *out++ = *in++;\n    }\n    return dst;\n}",
    ),
    (
        "string / memmove",
        "void *memmove(void *dst, const void *src, size_t count) {\n    unsigned char *out = dst;\n    const unsigned char *in = src;\n    if (out < in) {\n        while (count--) *out++ = *in++;\n    } else {\n        while (count--) out[count] = in[count];\n    }\n    return dst;\n}",
    ),
    (
        "string / strcmp",
        "int strcmp(const char *left, const char *right) {\n    while (*left && *left == *right) {\n        left++;\n        right++;\n    }\n    return *(const unsigned char *)left\n         - *(const unsigned char *)right;\n}",
    ),
    (
        "stdlib / bsearch",
        "while (count != 0) {\n    size_t middle = count / 2;\n    const void *entry = base + middle * size;\n    int order = compare(key, entry);\n    if (order == 0) return (void *)entry;\n    if (order > 0) {\n        base = entry + size;\n        count -= middle + 1;\n    } else {\n        count = middle;\n    }\n}\nreturn NULL;",
    ),
    (
        "stdlib / calloc",
        "void *calloc(size_t count, size_t size) {\n    if (size != 0 && count > SIZE_MAX / size) {\n        return NULL;\n    }\n    size_t bytes = count * size;\n    void *ptr = malloc(bytes);\n    if (ptr != NULL) {\n        memset(ptr, 0, bytes);\n    }\n    return ptr;\n}",
    ),
];

const JAVA_SNIPPETS: &[(&str, &str)] = &[
    (
        "Objects.requireNonNull",
        "public static <T> T requireNonNull(T object, String message) {\n    if (object == null) {\n        throw new NullPointerException(message);\n    }\n    return object;\n}",
    ),
    (
        "ArrayList.add",
        "public boolean add(E element) {\n    modCount++;\n    if (size == elementData.length) {\n        elementData = grow();\n    }\n    elementData[size++] = element;\n    return true;\n}",
    ),
    (
        "ArrayList.grow",
        "private Object[] grow(int minCapacity) {\n    int oldCapacity = elementData.length;\n    int preferredGrowth = oldCapacity >> 1;\n    int newCapacity = ArraysSupport.newLength(\n        oldCapacity, minCapacity - oldCapacity, preferredGrowth);\n    return elementData = Arrays.copyOf(elementData, newCapacity);\n}",
    ),
    (
        "HashMap.hash",
        "static final int hash(Object key) {\n    int hash;\n    return key == null ? 0 : (hash = key.hashCode()) ^ (hash >>> 16);\n}",
    ),
    (
        "Arrays.binarySearch",
        "while (low <= high) {\n    int middle = (low + high) >>> 1;\n    int value = array[middle];\n    if (value < key)\n        low = middle + 1;\n    else if (value > key)\n        high = middle - 1;\n    else\n        return middle;\n}\nreturn -(low + 1);",
    ),
    (
        "Optional.map",
        "public <U> Optional<U> map(Function<? super T, ? extends U> mapper) {\n    Objects.requireNonNull(mapper);\n    if (isEmpty()) {\n        return empty();\n    }\n    return Optional.ofNullable(mapper.apply(value));\n}",
    ),
    (
        "Collections.swap",
        "public static void swap(List<?> list, int first, int second) {\n    final List values = list;\n    values.set(first, values.set(second, values.get(first)));\n}",
    ),
    (
        "ConcurrentHashMap.spread",
        "static final int spread(int hash) {\n    return (hash ^ (hash >>> 16)) & HASH_BITS;\n}",
    ),
];

const GO_SNIPPETS: &[(&str, &str)] = &[
    (
        "slices / BinarySearch",
        "func BinarySearch[S ~[]E, E cmp.Ordered](x S, target E) (int, bool) {\n    n := len(x)\n    i, j := 0, n\n    for i < j {\n        h := int(uint(i+j) >> 1)\n        if x[h] < target {\n            i = h + 1\n        } else {\n            j = h\n        }\n    }\n    return i, i < n && x[i] == target\n}",
    ),
    (
        "slices / Clone",
        "func Clone[S ~[]E, E any](s S) S {\n    if s == nil {\n        return nil\n    }\n    return append(S([]E{}), s...)\n}",
    ),
    (
        "sync / Once.Do",
        "func (o *Once) Do(f func()) {\n    if o.done.Load() == 0 {\n        o.doSlow(f)\n    }\n}",
    ),
    (
        "sort / Search",
        "func Search(n int, f func(int) bool) int {\n    i, j := 0, n\n    for i < j {\n        h := int(uint(i+j) >> 1)\n        if !f(h) {\n            i = h + 1\n        } else {\n            j = h\n        }\n    }\n    return i\n}",
    ),
    (
        "bytes / Clone",
        "func Clone(b []byte) []byte {\n    if b == nil {\n        return nil\n    }\n    return append([]byte{}, b...)\n}",
    ),
    (
        "maps / Equal",
        "func Equal[M1, M2 ~map[K]V, K comparable, V comparable](m1 M1, m2 M2) bool {\n    if len(m1) != len(m2) {\n        return false\n    }\n    for k, v1 := range m1 {\n        if v2, ok := m2[k]; !ok || v1 != v2 {\n            return false\n        }\n    }\n    return true\n}",
    ),
    (
        "strings / Join",
        "func Join(elems []string, sep string) string {\n    switch len(elems) {\n    case 0:\n        return \"\"\n    case 1:\n        return elems[0]\n    }\n    var builder Builder\n    builder.Grow(len(sep) * (len(elems) - 1))\n    builder.WriteString(elems[0])\n    for _, elem := range elems[1:] {\n        builder.WriteString(sep)\n        builder.WriteString(elem)\n    }\n    return builder.String()\n}",
    ),
    (
        "context / WithCancel",
        "func WithCancel(parent Context) (Context, CancelFunc) {\n    c := withCancel(parent)\n    return c, func() { c.cancel(true, Canceled, nil) }\n}",
    ),
];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Language {
    Rust,
    C,
    Cpp,
    Java,
    Go,
}

impl Language {
    fn from_args<'a>(args: impl IntoIterator<Item = &'a str>) -> Self {
        let args: Vec<&str> = args.into_iter().collect();
        if args.iter().any(|arg| matches!(*arg, "--go" | "-g")) {
            Self::Go
        } else if args.iter().any(|arg| matches!(*arg, "--java" | "-j")) {
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
            Self::Go => GO_SNIPPETS,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::C => "c",
            Self::Cpp => "c++",
            Self::Java => "java",
            Self::Go => "go",
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
            "as" | "const"
                | "fn"
                | "let"
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
                | "static"
                | "unsafe"
                | "use"
                | "where"
                | "while"
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
        Language::Go => matches!(
            word.as_str(),
            "break"
                | "case"
                | "chan"
                | "const"
                | "continue"
                | "default"
                | "defer"
                | "else"
                | "fallthrough"
                | "for"
                | "func"
                | "go"
                | "goto"
                | "if"
                | "import"
                | "interface"
                | "map"
                | "package"
                | "range"
                | "return"
                | "select"
                | "struct"
                | "switch"
                | "type"
                | "var"
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
                | "Arc"
                | "Equal"
                | "Greater"
                | "JoinHandle"
                | "Less"
                | "MaybeUninit"
                | "Ordering"
                | "str"
                | "bool"
                | "i32"
                | "u16"
                | "usize"
        ),
        Language::C => matches!(
            word.as_str(),
            "FILE"
                | "NULL"
                | "SIZE_MAX"
                | "max_align_t"
                | "ptrdiff_t"
                | "size_t"
                | "stderr"
                | "stdin"
                | "stdout"
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
                | "unique_ptr"
                | "vector"
        ),
        Language::Java => matches!(
            word.as_str(),
            "String"
                | "Integer"
                | "Long"
                | "Double"
                | "ArraysSupport"
                | "Function"
                | "NullPointerException"
                | "Optional"
                | "List"
                | "Map"
                | "Set"
                | "Object"
                | "System"
        ),
        Language::Go => matches!(
            word.as_str(),
            "any"
                | "bool"
                | "byte"
                | "comparable"
                | "complex64"
                | "complex128"
                | "error"
                | "false"
                | "float32"
                | "float64"
                | "int"
                | "int8"
                | "int16"
                | "int32"
                | "int64"
                | "nil"
                | "rune"
                | "string"
                | "true"
                | "uint"
                | "uint8"
                | "uint16"
                | "uint32"
                | "uint64"
                | "uintptr"
                | "Builder"
                | "CancelFunc"
                | "Context"
                | "Once"
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
        assert!(app.target().contains("remove_reference_t"));

        let chars: Vec<char> = "const int value".chars().collect();
        assert_eq!(syntax_color(&chars, 0, Language::Cpp), Color::Cyan);
        assert_eq!(syntax_color(&chars, 6, Language::Cpp), Color::Blue);
    }

    #[test]
    fn c_mode_uses_c_snippets_and_highlighting() {
        let mut app = App::new(Language::C);
        app.snippet = 0;
        assert!(app.target().contains("void *malloc"));

        let chars: Vec<char> = "const size_t length".chars().collect();
        assert_eq!(syntax_color(&chars, 0, Language::C), Color::Cyan);
        assert_eq!(syntax_color(&chars, 6, Language::C), Color::Blue);
    }

    #[test]
    fn java_mode_uses_java_snippets_and_highlighting() {
        let mut app = App::new(Language::Java);
        app.snippet = 0;
        assert!(app.target().contains("requireNonNull"));

        let chars: Vec<char> = "public String value".chars().collect();
        assert_eq!(syntax_color(&chars, 0, Language::Java), Color::Cyan);
        assert_eq!(syntax_color(&chars, 7, Language::Java), Color::Blue);
    }

    #[test]
    fn go_mode_uses_go_snippets_and_highlighting() {
        let mut app = App::new(Language::Go);
        app.snippet = 0;
        assert!(app.target().contains("func BinarySearch"));

        let chars: Vec<char> = "func Search(n int)".chars().collect();
        assert_eq!(syntax_color(&chars, 0, Language::Go), Color::Cyan);
        assert_eq!(syntax_color(&chars, 14, Language::Go), Color::Blue);
    }

    #[test]
    fn startup_flags_select_the_language() {
        assert_eq!(Language::from_args([]), Language::Rust);
        assert_eq!(Language::from_args(["--c"]), Language::C);
        assert_eq!(Language::from_args(["--cpp"]), Language::Cpp);
        assert_eq!(Language::from_args(["-c"]), Language::Cpp);
        assert_eq!(Language::from_args(["--java"]), Language::Java);
        assert_eq!(Language::from_args(["-j"]), Language::Java);
        assert_eq!(Language::from_args(["--go"]), Language::Go);
        assert_eq!(Language::from_args(["-g"]), Language::Go);
    }
}
