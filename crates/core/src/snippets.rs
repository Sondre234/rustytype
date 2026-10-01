//! Typing exercises: (title, source). Keep lines under ~76 columns so they
//! fit an 80-column terminal, and keep titles unique (scores reference them).

pub const SNIPPETS: &[(&str, &str)] = &[
    (
        "parse / read_port_numbers",
        r#"pub fn read_port_numbers(input: &str) -> Result<Vec<u16>, ParseIntError> {
    let mut seen_ports = Vec::<u16>::with_capacity(input.len() / 4);
    for word in input.split_whitespace() {
        let port = word.trim_matches(',').parse::<u16>()?;
        if !seen_ports.contains(&port) {
            seen_ports.push(port);
        }
    }
    seen_ports.sort_unstable();
    Ok(seen_ports)
}"#,
    ),
    (
        "iter / group_words_by_length",
        r#"pub fn group_words_by_length(text: &str) -> BTreeMap<usize, Vec<&str>> {
    let mut groups = BTreeMap::<usize, Vec<&str>>::new();
    for word in text.split_whitespace() {
        groups.entry(word.len()).or_default().push(word);
    }
    for words in groups.values_mut() {
        words.sort_unstable();
        words.dedup();
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
}

pub fn total_bytes_needed<T>(count: usize) -> Option<usize> {
    let size = std::mem::size_of::<T>();
    let padding = size.next_multiple_of(std::mem::align_of::<T>()) - size;
    count.checked_mul(size + padding)
}"#,
    ),
    (
        "sync / count_with_workers",
        r#"pub fn count_with_workers(worker_count: usize, jobs_each: usize) -> usize {
    let counter = Mutex::<usize>::new(0);
    thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                for _ in 0..jobs_each {
                    *counter.lock().unwrap() += 1;
                }
            });
        }
    });
    counter.into_inner().unwrap()
}"#,
    ),
    (
        "option / display_name_for",
        r#"pub fn display_name_for(user: &User) -> String {
    let nickname = user.nickname.as_deref().filter(|name| !name.is_empty());
    let full_name = user.full_name.as_deref().map(str::trim);
    match (nickname, full_name) {
        (Some(nickname), Some(full_name)) => {
            format!("{nickname} ({full_name})")
        }
        (Some(nickname), None) => nickname.to_string(),
        (None, Some(full_name)) => full_name.to_string(),
        (None, None) => String::from("anonymous"),
    }
}"#,
    ),
    (
        "result / load_all_settings",
        r#"pub fn load_all_settings(
    paths: &[PathBuf],
) -> Result<Vec<Settings>, LoadError> {
    paths
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path).map_err(LoadError::Read)?;
            text.parse::<Settings>().map_err(LoadError::Parse)
        })
        .collect::<Result<Vec<_>, _>>()
}"#,
    ),
    (
        "stats / mean_and_spread",
        r#"pub fn mean_and_spread(values: &[f64]) -> Option<(f64, f64)> {
    let smallest = values.iter().copied().reduce(f64::min)?;
    let largest = values.iter().copied().reduce(f64::max)?;
    let total = values.iter().sum::<f64>();
    let mean = total / values.len() as f64;
    Some((mean, largest - smallest))
}

pub fn count_above_mean(values: &[f64]) -> usize {
    let (mean, _) = mean_and_spread(values).unwrap_or_default();
    values.iter().filter(|value| **value > mean).count()
}"#,
    ),
    (
        "traits / largest_shape",
        r#"pub trait Shape {
    fn name(&self) -> &'static str;
    fn area(&self) -> f64;
}

pub fn largest_shape(shapes: &[Box<dyn Shape>]) -> Option<&dyn Shape> {
    shapes
        .iter()
        .max_by(|left, right| left.area().total_cmp(&right.area()))
        .map(|shape| shape.as_ref())
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
    ranked.sort_by(|left, right| {
        right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0))
    });
    ranked.truncate(limit);
    ranked
}"#,
    ),
    (
        "config / read_environment",
        r#"pub fn port_from_environment() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|text| text.parse::<u16>().ok())
        .unwrap_or(8080)
}

pub fn worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
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
