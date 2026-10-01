//! Typing exercises: (title, source). Keep them short (about 6-9 lines), keep
//! lines under ~76 columns so they fit an 80-column terminal, and keep titles
//! unique (scores reference them).

pub const SNIPPETS: &[(&str, &str)] = &[
    (
        "parse / read_port_numbers",
        r#"pub fn read_port_numbers(input: &str) -> Vec<u16> {
    let mut seen_ports = input
        .split_whitespace()
        .filter_map(|word| word.parse::<u16>().ok())
        .collect::<Vec<_>>();
    seen_ports.sort_unstable();
    seen_ports.dedup();
    seen_ports
}"#,
    ),
    (
        "iter / group_words_by_length",
        r#"pub fn group_words_by_length(text: &str) -> BTreeMap<usize, Vec<&str>> {
    let mut groups = BTreeMap::<usize, Vec<&str>>::new();
    for word in text.split_whitespace() {
        groups.entry(word.len()).or_default().push(word);
    }
    groups
}"#,
    ),
    (
        "mem / describe_layout",
        r#"pub fn describe_layout<T>() -> String {
    let name = std::any::type_name::<T>();
    let size = std::mem::size_of::<T>();
    let alignment = std::mem::align_of::<T>();
    format!("{name} takes {size} bytes with alignment {alignment}")
}"#,
    ),
    (
        "sync / count_with_workers",
        r#"pub fn count_with_workers(workers: usize) -> usize {
    let counter = Mutex::<usize>::new(0);
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| *counter.lock().unwrap() += 1);
        }
    });
    counter.into_inner().unwrap()
}"#,
    ),
    (
        "option / display_name_for",
        r#"pub fn display_name_for(user: &User) -> String {
    let nickname = user.nickname.as_deref().filter(|name| !name.is_empty());
    match (nickname, user.full_name.as_deref()) {
        (Some(nickname), Some(full)) => format!("{nickname} ({full})"),
        (Some(nickname), None) => nickname.to_string(),
        (None, Some(full)) => full.to_string(),
        (None, None) => String::from("anonymous"),
    }
}"#,
    ),
    (
        "result / load_all_settings",
        r#"pub fn load_all_settings(paths: &[PathBuf]) -> Vec<Settings> {
    paths
        .iter()
        .filter_map(|path| {
            let text = fs::read_to_string(path).ok()?;
            text.parse::<Settings>().ok()
        })
        .collect::<Vec<_>>()
}"#,
    ),
    (
        "stats / mean_and_spread",
        r#"pub fn mean_and_spread(values: &[f64]) -> Option<(f64, f64)> {
    let smallest = values.iter().copied().reduce(f64::min)?;
    let largest = values.iter().copied().reduce(f64::max)?;
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    Some((mean, largest - smallest))
}"#,
    ),
    (
        "traits / largest_shape",
        r#"pub fn largest_shape(shapes: &[Box<dyn Shape>]) -> Option<&dyn Shape> {
    shapes
        .iter()
        .map(|shape| shape.as_ref())
        .max_by(|left, right| left.area().total_cmp(&right.area()))
}"#,
    ),
    (
        "string / most_common_words",
        r#"pub fn most_common_words(text: &str, limit: usize) -> Vec<(String, usize)> {
    let mut counts = HashMap::<String, usize>::new();
    for word in text.split_whitespace() {
        *counts.entry(word.to_lowercase()).or_default() += 1;
    }
    let mut ranked = counts.into_iter().collect::<Vec<_>>();
    ranked.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    ranked.into_iter().take(limit).collect()
}"#,
    ),
    (
        "config / read_environment",
        r#"pub fn port_from_environment() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|text| text.parse::<u16>().ok())
        .unwrap_or(8080)
}"#,
    ),
];

pub fn index_of(title: &str) -> Option<usize> {
    SNIPPETS.iter().position(|(name, _)| *name == title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_unique_and_lines_fit_a_small_terminal() {
        for (index, (title, source)) in SNIPPETS.iter().enumerate() {
            assert_eq!(index_of(title), Some(index), "duplicate title {title}");
            for line in source.lines() {
                assert!(line.chars().count() <= 76, "{title}: {line}");
            }
        }
    }
}
