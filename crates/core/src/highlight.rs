use crossterm::style::Color;

pub(crate) fn syntax_color(chars: &[char], index: usize) -> Color {
    let ch = chars[index];
    let in_string = chars[..index]
        .iter()
        .rev()
        .take_while(|&&c| c != '\n')
        .filter(|&&c| c == '"')
        .count()
        % 2
        == 1;
    if ch == '"' || in_string {
        return Color::Yellow;
    }
    if ch.is_ascii_digit() {
        return Color::Magenta;
    }
    let is_word = |c: &char| c.is_alphanumeric() || *c == '_';
    let start = chars[..index]
        .iter()
        .rposition(|c| !is_word(c))
        .map_or(0, |position| position + 1);
    let end = chars[index..]
        .iter()
        .position(|c| !is_word(c))
        .map_or(chars.len(), |offset| index + offset);
    let word: String = chars[start..end].iter().collect();
    if matches!(
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
            | "for"
            | "in"
            | "loop"
            | "while"
            | "break"
            | "continue"
            | "async"
            | "await"
            | "move"
            | "pub"
            | "static"
            | "trait"
            | "type"
            | "dyn"
            | "unsafe"
            | "use"
            | "where"
            | "return"
            | "Self"
            | "self"
    ) {
        Color::Blue
    } else if word.starts_with(|c: char| c.is_ascii_uppercase()) {
        Color::Cyan
    } else {
        Color::Grey
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_types_and_strings_get_distinct_colors() {
        let chars: Vec<char> = "let name = String::from(\"x\")".chars().collect();
        assert_eq!(syntax_color(&chars, 0), Color::Blue);
        assert_eq!(syntax_color(&chars, 4), Color::Grey);
        assert_eq!(syntax_color(&chars, 11), Color::Cyan);
        assert_eq!(syntax_color(&chars, 25), Color::Yellow);
    }
}
