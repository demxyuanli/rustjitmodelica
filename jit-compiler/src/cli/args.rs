pub(crate) fn parse_numeric_prefix(s: &str) -> Option<f64> {
    let v = s.trim_start();
    let mut end = 0usize;
    for (i, ch) in v.char_indices() {
        if ch.is_ascii_digit() || matches!(ch, '.' | '+' | '-' | 'e' | 'E') {
            end = i + ch.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    v[..end]
        .parse::<f64>()
        .ok()
        .filter(|x| x.is_finite() && *x > 0.0)
}

pub(crate) fn find_call_args<'a>(text: &'a str, call_name: &str) -> Option<&'a str> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
        let b = bytes[i];
        if b == b'"' {
            i += 1;
            while i < n {
                if bytes[i] == b'\\' {
                    i = i.saturating_add(2);
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        if i + call_name.len() <= n && &text[i..i + call_name.len()] == call_name {
            let prev_ok = if i == 0 {
                true
            } else {
                let c = bytes[i - 1] as char;
                !(c.is_ascii_alphanumeric() || c == '_')
            };
            if !prev_ok {
                i += 1;
                continue;
            }
            let mut j = i + call_name.len();
            while j < n && (bytes[j] as char).is_ascii_whitespace() {
                j += 1;
            }
            if j >= n || bytes[j] != b'(' {
                i += 1;
                continue;
            }
            let args_start = j + 1;
            let mut depth = 1i32;
            let mut k = args_start;
            while k < n {
                let ch = bytes[k];
                if ch == b'"' {
                    k += 1;
                    while k < n {
                        if bytes[k] == b'\\' {
                            k = k.saturating_add(2);
                        } else if bytes[k] == b'"' {
                            k += 1;
                            break;
                        } else {
                            k += 1;
                        }
                    }
                    continue;
                }
                if ch == b'(' {
                    depth += 1;
                } else if ch == b')' {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&text[args_start..k]);
                    }
                }
                k += 1;
            }
            return None;
        }
        i += 1;
    }
    None
}

pub(crate) fn split_top_level_args(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut depth = 0i32;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i = i.saturating_add(2);
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        match ch {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                parts.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start <= s.len() {
        parts.push(s[start..].trim());
    }
    parts
}

pub(crate) fn parse_rustmodlica_overdet_tol(annotation: &str) -> Option<f64> {
    let args = find_call_args(annotation, "__RustModlica")?;
    for item in split_top_level_args(args) {
        let Some(eq_idx) = item.find('=') else {
            continue;
        };
        let key = item[..eq_idx].trim();
        if key != "overdetTol" {
            continue;
        }
        let value = item[eq_idx + 1..].trim();
        return parse_numeric_prefix(value);
    }
    None
}
