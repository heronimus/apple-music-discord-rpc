pub fn lucene_escape(term: &str) -> String {
    let special_chars = [
        '+', '-', '&', '|', '!', '(', ')', '{', '}', '[', ']', '^', '"', '~', '*', '?', ':', '\\',
    ];
    let mut result = String::with_capacity(term.len() * 2);
    for c in term.chars() {
        if special_chars.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

pub fn remove_parentheses_content(term: &str) -> String {
    term.chars()
        .scan(0, |depth, c| {
            match c {
                '(' => *depth += 1,
                ')' if *depth > 0 => *depth -= 1,
                _ if *depth == 0 => return Some(Some(c)),
                _ => {}
            }
            Some(None)
        })
        .flatten()
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn truncate_string(value: &str) -> String {
    let max_length: usize = 128;
    if value.len() <= max_length {
        value.to_string()
    } else {
        let mut truncated = value.to_string();
        truncated.truncate(max_length - 3);
        truncated.push_str("...");
        truncated
    }
}
