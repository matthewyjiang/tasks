pub(crate) fn normalize_query(query: &str) -> String {
    query.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_query_trims_whitespace() {
        assert_eq!(normalize_query("  open  "), "open");
    }
}

use regex::Regex;
use taskmanager_core::Task;

pub(crate) fn regex_from_query(query: &str) -> Result<Regex, regex::Error> {
    let pattern = if query.is_empty() { ".*" } else { query };
    Regex::new(&format!("(?i){pattern}"))
}

pub(crate) fn regex_matches_task(regex: &Regex, task: &Task) -> bool {
    regex.is_match(&task.title)
        || regex.is_match(&task.body)
        || task.tags.iter().any(|tag| regex.is_match(tag))
}
