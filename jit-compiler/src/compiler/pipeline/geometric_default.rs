pub fn geometric_default_for_name(name: &str) -> f64 {
    let lower = name.to_lowercase();
    let parts: Vec<&str> = lower.split(|c: char| c == '.' || c == '_').collect();
    let len = parts.len();

    if len >= 4 {
        let last = parts[len - 1];
        let second_last = parts[len - 2];
        if let Ok(component) = last.parse::<usize>() {
            if second_last == "0" && component >= 1 && component <= 3 {
                for i in 0..(len - 2) {
                    let p = parts[i];
                    if p == "e" || p == "n" || p == "delta"
                        || p == "ex" || p == "ey" || p == "ez"
                    {
                        return if component == 1 { 1.0 } else { 0.0 };
                    }
                }
            }
        }
    }

    for i in 0..len {
        let p = parts[i];
        if p == "e" || p == "n" {
            if i + 1 < len {
                let next = parts[i + 1];
                if next == "lat" || next == "long" || next == "axis" || next == "s"
                    || next == "n" || next == "0"
                {
                    return 1.0;
                }
                if next == "1" || next == "0" {
                    return 1.0;
                }
                if next == "2" || next == "3" {
                    return 0.0;
                }
            }
        }
        if p == "delta" && i + 1 < len {
            let next = parts[i + 1];
            if next == "0" {
                return 1.0;
            }
        }
    }

    if len >= 2 {
        let last = parts[len - 1];
        let second_last = parts[len - 2];
        if second_last == "t" {
            if let Ok(row) = last.parse::<usize>() {
                if last.len() == 1 {
                    let col_part = if len >= 3 { parts[len - 3] } else { "" };
                    if let Ok(col) = col_part.parse::<usize>() {
                        return if row == col { 1.0 } else { 0.0 };
                    }
                }
            }
        }
    }

    if lower.contains("_t_1_1") || lower.contains("_t_2_2") || lower.contains("_t_3_3")
        || lower.contains(".t[1,1]") || lower.contains(".t[2,2]") || lower.contains(".t[3,3]")
    {
        return 1.0;
    }
    if lower.contains("_t_1_") || lower.contains("_t_2_") || lower.contains("_t_3_")
        || lower.contains(".t[")
    {
        return 0.0;
    }

    0.0
}
