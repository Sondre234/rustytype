/// For each char of `chars` that closes a pair, the index of its opener, so
/// autoclose knows which closer an opener brings along. Pairs are `()`, `[]`,
/// `{}`, generic `<>` and `""`. Operators (`a < b`, `->`, `=>`) pair with
/// nothing, and a bracket opened inside a string only closes inside it.
pub(crate) fn openers(chars: &[char]) -> Vec<Option<usize>> {
    let mut openers = vec![None; chars.len()];
    let mut open: Vec<usize> = Vec::new();
    let mut string_start: Option<usize> = None;
    let mut escaped = false;
    for (index, &ch) in chars.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        let reachable = |opener: usize| string_start.is_none_or(|start| opener > start);
        match ch {
            '\\' if string_start.is_some() => escaped = true,
            '"' => match string_start.take() {
                Some(start) => {
                    openers[index] = Some(start);
                    open.retain(|&opener| opener < start);
                }
                None => string_start = Some(index),
            },
            '(' | '[' | '{' => open.push(index),
            '<' if !matches!(chars.get(index + 1), None | Some(' ' | '=' | '<')) => open.push(index),
            '>' if index > 0 && matches!(chars[index - 1], '-' | '=') => {}
            ')' | ']' | '}' | '>' => {
                let wanted = match ch {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => '<',
                };
                if ch != '>' {
                    // A `<` still open here was a comparison, not generics.
                    while open.last().is_some_and(|&opener| reachable(opener) && chars[opener] == '<') {
                        open.pop();
                    }
                }
                if let Some(&opener) = open.last()
                    && reachable(opener)
                    && chars[opener] == wanted
                {
                    openers[index] = Some(opener);
                    open.pop();
                }
            }
            _ => {}
        }
    }
    openers
}
