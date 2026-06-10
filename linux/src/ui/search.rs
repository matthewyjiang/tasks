pub fn normalize_query(query: &str) -> String {
    query.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_query_trims_whitespace() {
        assert_eq!(normalize_query("  inbox  "), "inbox");
    }
}
