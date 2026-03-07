/// Checks whether a commit message follows the Conventional Commits spec.
/// Expects: `type[(scope)][!]: description`
pub fn is_conventional_commit(message: &str) -> bool {
    let first_line = message.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return false;
    }

    // Split on first ':'
    let Some(colon_pos) = first_line.find(':') else {
        return false;
    };

    let prefix = &first_line[..colon_pos];
    let description = first_line[colon_pos + 1..].trim();

    // Description after colon must be non-empty
    if description.is_empty() {
        return false;
    }

    // Strip optional '!' before colon (breaking change marker)
    let prefix = prefix.strip_suffix('!').unwrap_or(prefix);

    // Strip optional scope in parens
    let type_part = if let Some(paren_start) = prefix.find('(') {
        if !prefix.ends_with(')') {
            return false;
        }
        &prefix[..paren_start]
    } else {
        prefix
    };

    // Type must be non-empty, lowercase alphanumeric
    !type_part.is_empty()
        && type_part
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::is_conventional_commit;

    #[test]
    fn valid_conventional_commits() {
        assert!(is_conventional_commit("feat: add login"));
        assert!(is_conventional_commit("fix: resolve crash"));
        assert!(is_conventional_commit("feat(auth): add oauth"));
        assert!(is_conventional_commit("fix(ui): button alignment"));
        assert!(is_conventional_commit("chore: bump deps"));
        assert!(is_conventional_commit("docs: update readme"));
        assert!(is_conventional_commit("feat!: breaking change"));
        assert!(is_conventional_commit("feat(api)!: remove endpoint"));
        assert!(is_conventional_commit("ci: update workflow"));
        assert!(is_conventional_commit("refactor: simplify logic"));
        assert!(is_conventional_commit("feat: add thing\n\nbody text here"));
    }

    #[test]
    fn invalid_conventional_commits() {
        assert!(!is_conventional_commit(""));
        assert!(!is_conventional_commit("just a message"));
        assert!(!is_conventional_commit("Fix the bug"));
        assert!(!is_conventional_commit("feat:"));
        assert!(!is_conventional_commit("feat: "));
        assert!(!is_conventional_commit("FEAT: uppercase type"));
        assert!(!is_conventional_commit("feat(: bad scope"));
        assert!(!is_conventional_commit(": no type"));
    }
}
