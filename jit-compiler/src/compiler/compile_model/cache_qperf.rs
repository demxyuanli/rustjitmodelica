use std::collections::BTreeMap;

pub(crate) fn sum_cache_stage_metric(
    qperf: &std::collections::HashMap<String, u64>,
    prefix: &str,
    stage: &str,
) -> u64 {
    let mut sum = 0_u64;
    for (k, v) in qperf {
        let Some(tail) = k.strip_prefix(prefix) else {
            continue;
        };
        let mut parts = tail.splitn(2, ':');
        let _scope = parts.next();
        if parts.next() == Some(stage) {
            sum += v;
        }
    }
    sum
}

pub(crate) fn build_cache_scope_stage_map(
    qperf: &std::collections::HashMap<String, u64>,
    prefix: &str,
) -> BTreeMap<String, BTreeMap<String, u64>> {
    let mut out: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for (k, v) in qperf {
        if !k.starts_with(prefix) {
            continue;
        }
        let tail = &k[prefix.len()..];
        let mut parts = tail.splitn(2, ':');
        let Some(scope) = parts.next() else {
            continue;
        };
        let Some(stage) = parts.next() else {
            continue;
        };
        out.entry(scope.to_string())
            .or_default()
            .insert(stage.to_string(), *v);
    }
    out
}
