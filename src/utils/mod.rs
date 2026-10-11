pub mod image;
pub mod system;

pub fn lng_trs<T>(is_korean: bool, en: T, ko: T) -> T {
    if is_korean {
        ko
    } else {
        en
    }
}

pub fn parse_quoted<'a>(
    iter: &mut impl Iterator<Item = &'a str>,
    arg_name: &str,
    max_len: usize,
) -> Result<String, String> {
    let first = iter.next().ok_or_else(|| format!("{} null", arg_name))?;
    if !first.starts_with('"') {
        return Err(format!("{} must start with a quote (\")", arg_name));
    }

    let mut text = first
        .trim_start_matches('"')
        .trim_end_matches('"')
        .to_string();
    let mut found_end_quote = first.len() >= 2 && first.ends_with('"');

    while !found_end_quote {
        if let Some(next) = iter.next() {
            if !text.is_empty() {
                text.push(' ');
            }
            if let Some(stripped) = next.strip_suffix('"') {
                text.push_str(stripped);
                found_end_quote = true;
            } else {
                text.push_str(next);
            }
        } else {
            return Err(format!("{} missing closing quote (\")", arg_name));
        }
    }

    if text.len() > max_len {
        return Err(format!("{} length must be <= {}", arg_name, max_len));
    }

    Ok(text)
}
