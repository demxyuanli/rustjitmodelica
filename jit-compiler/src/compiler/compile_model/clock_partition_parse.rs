use super::super::ClockPartitionTrigger;

pub(crate) fn parse_clock_partition_trigger(id: &str) -> ClockPartitionTrigger {
    fn parse_clock_factor(token: &str) -> Option<f64> {
        if let Ok(v) = token.parse::<f64>() {
            if v <= 0.0 {
                return Some(1.0);
            }
            return Some(v);
        }
        if let Some(inner) = token
            .strip_prefix("Number(")
            .and_then(|s| s.strip_suffix(')'))
        {
            if let Ok(v) = inner.parse::<f64>() {
                if v <= 0.0 {
                    return Some(1.0);
                }
                return Some(v);
            }
            return None;
        }
        None
    }

    fn parse_sample_key(key: &str) -> Option<(f64, f64)> {
        let rest = key.strip_prefix("sample_")?;
        let mut parts = rest.splitn(2, '_');
        let interval = parts.next()?.parse::<f64>().ok()?;
        if interval <= 0.0 {
            return None;
        }
        let start = parts
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        Some((start, interval))
    }

    fn parse_derived_key(key: &str) -> Option<(f64, f64)> {
        let (op, rest) = if let Some(r) = key.strip_prefix("subSample_") {
            ("sub", r)
        } else if let Some(r) = key.strip_prefix("superSample_") {
            ("super", r)
        } else if let Some(r) = key.strip_prefix("shiftSample_") {
            ("shift", r)
        } else if let Some(r) = key.strip_prefix("backSample_") {
            ("back", r)
        } else {
            return None;
        };
        let split = rest.rfind('_')?;
        let (base_key, factor_token) = rest.split_at(split);
        let factor = parse_clock_factor(factor_token.trim_start_matches('_'))?;
        if factor == 0.0 {
            return None;
        }
        let (base_start, base_interval) = parse_clock_key(base_key)?;
        match op {
            "sub" => Some((base_start, base_interval * factor)),
            "super" => Some((base_start, base_interval / factor)),
            "shift" => Some((base_start + factor * base_interval, base_interval)),
            "back" => Some((
                base_start + (factor - 1.0) * base_interval,
                base_interval * factor,
            )),
            _ => None,
        }
    }

    fn parse_clock_key(key: &str) -> Option<(f64, f64)> {
        parse_sample_key(key).or_else(|| parse_derived_key(key))
    }

    if let Some((start, interval)) = parse_clock_key(id) {
        if interval > 0.0 {
            return ClockPartitionTrigger::Sample { start, interval };
        }
    }
    ClockPartitionTrigger::Always
}
