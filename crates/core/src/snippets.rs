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
}

pub fn busiest_port(ports: &[u16]) -> Option<u16> {
    let mut hits = HashMap::<u16, usize>::new();
    for port in ports {
        *hits.entry(*port).or_default() += 1;
    }
    hits.into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(port, _)| port)
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
}

pub fn longest_group_size(groups: &BTreeMap<usize, Vec<&str>>) -> usize {
    groups
        .values()
        .map(|words| words.len())
        .max()
        .unwrap_or_default()
}

pub fn total_word_count(groups: &BTreeMap<usize, Vec<&str>>) -> usize {
    groups.values().map(|words| words.len()).sum::<usize>()
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
}

pub fn print_common_layouts() {
    println!("{}", describe_layout::<u8>());
    println!("{}", describe_layout::<String>());
    println!("{}", describe_layout::<Vec<u32>>());
    println!("{}", describe_layout::<Option<Box<u64>>>());
}"#,
    ),
    (
        "sync / count_with_workers",
        r#"pub fn count_with_workers(worker_count: usize, jobs_each: usize) -> usize {
    let counter = Arc::<Mutex<usize>>::new(Mutex::new(0));
    let mut handles = Vec::<JoinHandle<()>>::with_capacity(worker_count);
    for worker_id in 0..worker_count {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..jobs_each {
                let mut guard = counter.lock().expect("mutex poisoned");
                *guard += 1;
            }
            println!("worker {worker_id} finished all of its jobs");
        }));
    }
    for handle in handles {
        handle.join().expect("worker thread panicked");
    }
    let total = *counter.lock().unwrap();
    total
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
}

pub fn parse_user_age(raw_age: &str) -> Option<u8> {
    raw_age.trim().parse::<u8>().ok().filter(|age| *age >= 13)
}

pub fn adult_user_count(users: &[User]) -> usize {
    users
        .iter()
        .filter_map(|user| user.age_text.as_deref())
        .filter_map(parse_user_age)
        .filter(|age| *age >= 18)
        .count()
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
}

pub fn first_valid_setting(paths: &[PathBuf]) -> Option<Settings> {
    paths.iter().find_map(|path| {
        let text = fs::read_to_string(path).ok()?;
        text.parse::<Settings>().ok()
    })
}

pub fn count_missing_files(paths: &[PathBuf]) -> usize {
    paths.iter().filter(|path| !path.exists()).count()
}"#,
    ),
    (
        "stats / running_statistics",
        r#"pub struct RunningStatistics {
    count: u64,
    total: f64,
    smallest: Option<f64>,
    largest: Option<f64>,
}

impl RunningStatistics {
    pub fn record(&mut self, value: f64) {
        self.count += 1;
        self.total += value;
        let smallest = self.smallest.map_or(value, |old| old.min(value));
        let largest = self.largest.map_or(value, |old| old.max(value));
        self.smallest = Some(smallest);
        self.largest = Some(largest);
    }

    pub fn mean(&self) -> Option<f64> {
        (self.count > 0).then(|| self.total / self.count as f64)
    }

    pub fn spread(&self) -> Option<f64> {
        Some(self.largest? - self.smallest?)
    }
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
}

pub fn total_area(shapes: &[Box<dyn Shape>]) -> f64 {
    shapes.iter().map(|shape| shape.area()).sum::<f64>()
}

pub fn describe_all(shapes: &[Box<dyn Shape>]) -> Vec<String> {
    shapes
        .iter()
        .map(|shape| format!("{} covers {:.2}", shape.name(), shape.area()))
        .collect::<Vec<_>>()
}"#,
    ),
    (
        "string / most_common_words",
        r#"pub fn most_common_words(text: &str, limit: usize) -> Vec<(String, usize)> {
    let mut counts = HashMap::<String, usize>::new();
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        if word.is_empty() {
            continue;
        }
        *counts.entry(word.to_lowercase()).or_insert(0) += 1;
    }
    let mut ranked = counts.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0))
    });
    ranked.truncate(limit);
    ranked
}

pub fn unique_word_count(text: &str) -> usize {
    text.split_whitespace()
        .map(str::to_lowercase)
        .collect::<HashSet<_>>()
        .len()
}"#,
    ),
    (
        "config / server_config",
        r#"#[derive(Debug, Default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub worker_threads: usize,
}

impl ServerConfig {
    pub fn from_environment() -> Self {
        let port = std::env::var("PORT")
            .ok()
            .and_then(|text| text.parse::<u16>().ok())
            .unwrap_or(8080);
        let worker_threads = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1);
        Self {
            host: String::from("localhost"),
            port,
            worker_threads,
        }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
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
