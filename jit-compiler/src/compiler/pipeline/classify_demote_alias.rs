/// Connector potentials (flange.phi, pin.v, ...) often appear under `der()` in sensors
/// and thus get marked as states, while `connect` also equates them to a real component
/// state (e.g. `inertia.phi`). Keeping both as independent integrators makes the connect
/// equality a skipped residual and leaves sensor outputs at 0. Demote connector potentials
/// that are simply equated to another selected state.
fn is_connector_potential_name(name: &str) -> bool {
    // e.g. inertia1_flange_a_phi, wSensor_flange_phi, fixed1_flange_phi, vsine_p_v
    (name.contains("_flange") && (name.ends_with("_phi") || name.ends_with("_s")))
        || name.ends_with("_p_v")
        || name.ends_with("_n_v")
}

/// Local potentials aliased to flanges (LossyGear `phi_a`, Force `s = flange.s - support`).
fn is_local_alias_potential_name(name: &str) -> bool {
    name.ends_with("_phi_a")
        || name.ends_with("_phi_b")
        || name.ends_with("_s_a")
        || name.ends_with("_s_b")
}

fn is_demotable_alias_state(name: &str) -> bool {
    is_connector_potential_name(name) || is_local_alias_potential_name(name)
}

/// Translational/Rotational `Sources.Position` (and Accelerate/Move) force a flange.
/// Instance leaf `position` / `position1` / `accelerate` / `move`.
fn is_prescribed_motion_source_state(name: &str) -> bool {
    if name.contains("sensor") || name.contains("Sensor") {
        return false;
    }
    if !(name.ends_with("_s")
        || name.ends_with("_v")
        || name.ends_with("_phi")
        || name.ends_with("_w"))
    {
        return false;
    }
    let prefix = mech_component_prefix(name);
    let leaf = prefix.rsplit('_').next().unwrap_or(prefix);
    let l = leaf.to_ascii_lowercase();
    l == "position"
        || l.starts_with("position")
        || l == "accelerate"
        || l == "move"
}

/// Prefer keeping Prefer/Always (or mass/inertia/body by name) over flange aliases.
fn is_preferred_absolute_state(
    name: &str,
    ranks: &HashMap<String, crate::ast::StateSelect>,
) -> bool {
    if let Some(ss) = ranks.get(name) {
        if ss.is_preferred() {
            return true;
        }
        if ss.is_demotable_when_preferred_exists() {
            return false;
        }
    }
    if is_demotable_alias_state(name)
        || is_relative_angle_state(name)
        || name.contains("_flange")
        || name.ends_with("_w_rel")
        || name.ends_with("_v_rel")
        || name.ends_with("_phi_a")
        || name.ends_with("_phi_b")
        || name.ends_with("_w_a")
        || name.ends_with("_w_b")
    {
        return false;
    }
    let p = mech_component_prefix(name).to_ascii_lowercase();
    p.contains("mass")
        || p.contains("inertia")
        || p.contains("body")
        || p.contains("spool")
        || p.contains("plate")
        || p.contains("wheel")
}

/// Force/sensor locals such as `quadraticSpeedDependentForce_s` / `windSpeed_s`.
fn is_secondary_local_potential(
    name: &str,
    ranks: &HashMap<String, crate::ast::StateSelect>,
) -> bool {
    if is_preferred_absolute_state(name, ranks)
        || is_demotable_alias_state(name)
        || is_relative_angle_state(name)
        || name.contains("_flange")
    {
        return false;
    }
    if !(name.ends_with("_s") || name.ends_with("_phi")) {
        return false;
    }
    let p = mech_component_prefix(name).to_ascii_lowercase();
    p.contains("force")
        || p.contains("speed")
        || p.contains("sensor")
        || p.contains("drag")
        || p.contains("roll")
        || p.contains("wind")
        || p.contains("torque")
        || p.contains("bearingfriction")
        || p.contains("supportfriction")
}

fn is_alias_demote_candidate(
    name: &str,
    ranks: &HashMap<String, crate::ast::StateSelect>,
) -> bool {
    is_demotable_alias_state(name) || is_secondary_local_potential(name, ranks)
}

fn velocity_partners_of_position(pos: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(base) = pos.strip_suffix("_phi_a") {
        out.push(format!("{base}_w_a"));
    } else if let Some(base) = pos.strip_suffix("_phi_b") {
        out.push(format!("{base}_w_b"));
    } else if let Some(base) = pos.strip_suffix("_s_a") {
        out.push(format!("{base}_v_a"));
    } else if let Some(base) = pos.strip_suffix("_s_b") {
        out.push(format!("{base}_v_b"));
    } else if let Some(base) = pos.strip_suffix("_phi") {
        out.push(format!("{base}_w"));
    } else if let Some(base) = pos.strip_suffix("_s") {
        out.push(format!("{base}_v"));
    }
    out
}

fn demote_state_and_velocity_partners(states: &mut HashSet<String>, pos: &str) {
    states.remove(pos);
    for v in velocity_partners_of_position(pos) {
        states.remove(&v);
    }
}

fn demote_equated_connector_states(
    equations: &[Equation],
    states: &mut HashSet<String>,
    ranks: &HashMap<String, crate::ast::StateSelect>,
) {
    if states.len() < 2 {
        return;
    }
    fn ensure(parent: &mut HashMap<String, String>, x: &str) {
        parent.entry(x.to_string()).or_insert_with(|| x.to_string());
    }
    fn find(parent: &mut HashMap<String, String>, x: &str) -> String {
        ensure(parent, x);
        let p = parent.get(x).cloned().unwrap_or_else(|| x.to_string());
        if p == x {
            return p;
        }
        let root = find(parent, &p);
        parent.insert(x.to_string(), root.clone());
        root
    }
    fn union(parent: &mut HashMap<String, String>, a: &str, b: &str) {
        ensure(parent, a);
        ensure(parent, b);
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    fn union_potential_pair(parent: &mut HashMap<String, String>, a: &str, b: &str) {
        union(parent, a, b);
        let a_pot = a.ends_with("_s") || a.ends_with("_phi");
        let b_pot = b.ends_with("_s") || b.ends_with("_phi");
        if !a_pot && !b_pot && (a.contains("_flange") || b.contains("_flange")) {
            for suf in ["_s", "_phi"] {
                union(parent, &format!("{a}{suf}"), &format!("{b}{suf}"));
            }
        }
    }

    let mut parent: HashMap<String, String> = HashMap::new();
    for eq in equations {
        if let Equation::Simple(lhs, rhs) = eq {
            if let (Expression::Variable(lid), Expression::Variable(rid)) = (lhs, rhs) {
                let a = crate::string_intern::resolve_id(*lid);
                let b = crate::string_intern::resolve_id(*rid);
                if is_relative_angle_state(&a) || is_relative_angle_state(&b) {
                    continue;
                }
                // Include non-state flange links so sensor.flange -> mass.flange_a
                // can reach mass.s through a later affine flange_a = s +/- L/2.
                if states.contains(&a)
                    || states.contains(&b)
                    || is_alias_demote_candidate(&a, ranks)
                    || is_alias_demote_candidate(&b, ranks)
                    || a.contains("_flange")
                    || b.contains("_flange")
                {
                    union_potential_pair(&mut parent, &a, &b);
                }
            }
            // flange_s = mass_s +/- L/2, or LossyGear phi_a = flange_a.phi - support.
            let try_affine_link = |parent: &mut HashMap<String, String>,
                                   name: &str,
                                   other: &Expression| {
                if is_relative_angle_state(name) {
                    return;
                }
                let Some(aff) = parse_affine_expr(other) else {
                    return;
                };
                // Only grow alias components from demotable locals / connector
                // potentials. Do not let relative-state affines merge masses.
                if !is_alias_demote_candidate(name, ranks) && !is_connector_potential_name(name) {
                    // Still allow PartialRigid flange = center +/- L/2.
                    let flange_like = name.contains("_flange")
                        || name.ends_with("_s")
                        || name.ends_with("_phi");
                    if !flange_like {
                        return;
                    }
                }
                for v in aff.terms.keys() {
                    if v == name || is_relative_angle_state(v) {
                        continue;
                    }
                    if states.contains(v)
                        || is_alias_demote_candidate(v, ranks)
                        || v.contains("_flange")
                        || v.ends_with("_phi")
                        || v.ends_with("_s")
                        || v.ends_with("_phi_support")
                        || v.ends_with("_s_support")
                    {
                        union(parent, name, v);
                    }
                }
            };
            match (lhs, rhs) {
                (Expression::Variable(lid), other) => {
                    let a = crate::string_intern::resolve_id(*lid);
                    try_affine_link(&mut parent, &a, other);
                }
                (other, Expression::Variable(rid)) => {
                    let b = crate::string_intern::resolve_id(*rid);
                    try_affine_link(&mut parent, &b, other);
                }
                _ => {}
            }
        }
    }
    if parent.is_empty() {
        return;
    }

    let mut comps: HashMap<String, Vec<String>> = HashMap::new();
    let state_list: Vec<String> = states.iter().cloned().collect();
    for s in &state_list {
        // Relative states are handled by demote_dependent_relative_states.
        if is_relative_angle_state(s) || !parent.contains_key(s) {
            continue;
        }
        let root = find(&mut parent, s);
        comps.entry(root).or_default().push(s.clone());
    }

    let demote_trace = std::env::var("RUSTMODLICA_STATE_DEMOTE_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false);

    for members in comps.values() {
        if members.len() < 2 {
            continue;
        }
        let has_preferred = members.iter().any(|m| is_preferred_absolute_state(m, ranks));
        if has_preferred {
            for m in members {
                if is_alias_demote_candidate(m, ranks) {
                    if demote_trace {
                        eprintln!("[state-demote] alias {m}");
                    }
                    demote_state_and_velocity_partners(states, m);
                }
            }
            let has_source = members
                .iter()
                .any(|m| is_prescribed_motion_source_state(m));
            if has_source {
                for m in members {
                    if is_prescribed_motion_source_state(m) {
                        continue;
                    }
                    if demote_trace {
                        eprintln!("[state-demote] prescribed-motion alias {m}");
                    }
                    demote_state_and_velocity_partners(states, m);
                }
            }
            continue;
        }
        // Only connector/local aliases in this component: keep one.
        let aliases: Vec<&String> = members
            .iter()
            .filter(|m| is_alias_demote_candidate(m, ranks) || is_connector_potential_name(m))
            .collect();
        if aliases.len() >= 2 {
            let mut sorted: Vec<String> = aliases.into_iter().cloned().collect();
            sorted.sort();
            for m in sorted.into_iter().skip(1) {
                demote_state_and_velocity_partners(states, &m);
            }
        }
    }
}

/// IdealRollingWheel: (phi - supportPhi)*radius = (s - supportS). Keeping both
/// mass.s and inertia.phi as independent states yields index-2.
fn demote_scaled_absolute_pairs(
    equations: &[Equation],
    states: &mut HashSet<String>,
    ranks: &HashMap<String, crate::ast::StateSelect>,
) {
    use crate::ast::Operator;
    use crate::string_intern::resolve_id;

    fn ensure(parent: &mut HashMap<String, String>, x: &str) {
        parent.entry(x.to_string()).or_insert_with(|| x.to_string());
    }
    fn find(parent: &mut HashMap<String, String>, x: &str) -> String {
        ensure(parent, x);
        let p = parent.get(x).cloned().unwrap_or_else(|| x.to_string());
        if p == x {
            return p;
        }
        let root = find(parent, &p);
        parent.insert(x.to_string(), root.clone());
        root
    }
    fn union(parent: &mut HashMap<String, String>, a: &str, b: &str) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    fn union_potential_pair(parent: &mut HashMap<String, String>, a: &str, b: &str) {
        union(parent, a, b);
        let a_pot = a.ends_with("_s") || a.ends_with("_phi");
        let b_pot = b.ends_with("_s") || b.ends_with("_phi");
        if !a_pot && !b_pot && (a.contains("_flange") || b.contains("_flange")) {
            for suf in ["_s", "_phi"] {
                union(parent, &format!("{a}{suf}"), &format!("{b}{suf}"));
            }
        }
    }

    let mut parent: HashMap<String, String> = HashMap::new();
    for eq in equations {
        let Equation::Simple(lhs, rhs) = eq else {
            continue;
        };
        if let (Expression::Variable(lid), Expression::Variable(rid)) = (lhs, rhs) {
            let a = resolve_id(*lid);
            let b = resolve_id(*rid);
            if a.contains("_flange")
                || b.contains("_flange")
                || a.ends_with("_s")
                || a.ends_with("_phi")
                || b.ends_with("_s")
                || b.ends_with("_phi")
                || states.contains(&a)
                || states.contains(&b)
            {
                union_potential_pair(&mut parent, &a, &b);
            }
        }
        let link_affine = |parent: &mut HashMap<String, String>, name: &str, other: &Expression| {
            if let Some(aff) = parse_affine_expr(other) {
                for t in aff.terms.keys() {
                    union(parent, name, t);
                }
            }
        };
        match (lhs, rhs) {
            (Expression::Variable(id), other) => {
                link_affine(&mut parent, &resolve_id(*id), other);
            }
            (other, Expression::Variable(id)) => {
                link_affine(&mut parent, &resolve_id(*id), other);
            }
            _ => {}
        }
    }

    // Prefer absolute state as representative of each component.
    {
        let names: Vec<String> = parent.keys().cloned().collect();
        for name in &names {
            if is_preferred_absolute_state(name, ranks) && states.contains(name) {
                let r = find(&mut parent, name);
                if r != *name {
                    parent.insert(r, name.clone());
                    parent.insert(name.clone(), name.clone());
                }
            }
        }
    }

    fn is_scale_factor_name(name: &str) -> bool {
        !name.contains("_flange")
            && !name.ends_with("_s")
            && !name.ends_with("_phi")
            && !name.ends_with("_v")
            && !name.ends_with("_w")
            && !name.ends_with("_phi_a")
            && !name.ends_with("_phi_b")
            && !name.ends_with("_s_a")
            && !name.ends_with("_s_b")
            && !name.contains("support")
            && !name.contains("Support")
    }

    /// Primary potential and whether a non-unit scale (radius/ratio) was applied.
    fn as_scaled_potential(expr: &Expression) -> Option<(String, bool)> {
        let (inner, scaled) = match expr {
            Expression::BinaryOp(l, Operator::Mul, r) => match (l.as_ref(), r.as_ref()) {
                (Expression::Number(c), other) | (other, Expression::Number(c)) => {
                    (other, (c.abs() - 1.0).abs() > 1e-12)
                }
                (Expression::Variable(id), other) | (other, Expression::Variable(id)) => {
                    let n = resolve_id(*id);
                    if is_scale_factor_name(&n) {
                        (other, true)
                    } else {
                        return None;
                    }
                }
                _ => return None,
            },
            other => (other, false),
        };
        let aff = parse_affine_expr(inner)?;
        if aff.terms.is_empty() {
            return None;
        }
        let mut primary: Option<String> = None;
        for name in aff.terms.keys() {
            let is_support = name.contains("support")
                || name.contains("Support")
                || name.ends_with("_phi_support")
                || name.ends_with("_s_support");
            if is_support {
                continue;
            }
            if primary.is_some() {
                return None;
            }
            primary = Some(name.clone());
        }
        Some((primary?, scaled))
    }

        let abs_in_root = |parent: &mut HashMap<String, String>, name: &str| -> Option<String> {
        states
            .iter()
            .filter(|s| {
                is_preferred_absolute_state(s, ranks) && find(parent, s) == find(parent, name)
            })
            .cloned()
            .max_by(|a, b| {
                // Prefer translational mass position names.
                let score = |n: &str| if n.ends_with("_s") { 2 } else if n.ends_with("_phi") { 1 } else { 0 };
                score(a).cmp(&score(b)).then_with(|| a.cmp(b))
            })
    };

    let demote_trace = std::env::var("RUSTMODLICA_STATE_DEMOTE_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false);

    let mut demote: Vec<String> = Vec::new();
    for eq in equations {
        let Equation::Simple(lhs, rhs) = eq else {
            continue;
        };
        let Some((a_name, a_scaled)) = as_scaled_potential(lhs) else {
            continue;
        };
        let Some((b_name, b_scaled)) = as_scaled_potential(rhs) else {
            continue;
        };
        if a_name == b_name {
            continue;
        }
        // IdealRollingWheel couples rotational phi with translational s.
        // Radius may be literal 1.0 (not "scaled"), so also accept cross-domain.
        let cross_domain = (a_name.ends_with("_phi") && b_name.ends_with("_s"))
            || (a_name.ends_with("_s") && b_name.ends_with("_phi"));
        if !a_scaled && !b_scaled && !cross_domain {
            continue;
        }
        let Some(a_abs) = abs_in_root(&mut parent, &a_name) else {
            continue;
        };
        let Some(b_abs) = abs_in_root(&mut parent, &b_name) else {
            continue;
        };
        if a_abs == b_abs {
            continue;
        }
        let demote_name = if a_abs.ends_with("_s") && b_abs.ends_with("_phi") {
            // Prefer rotational absolute: torque/inertia often drives IdealRollingWheel.
            a_abs.clone()
        } else if b_abs.ends_with("_s") && a_abs.ends_with("_phi") {
            b_abs.clone()
        } else if a_abs < b_abs {
            a_abs.clone()
        } else {
            b_abs.clone()
        };
        if demote_trace {
            eprintln!(
                "[state-demote] scaled-pair {a_name}->{a_abs} ~ {b_name}->{b_abs} demote={demote_name}"
            );
        }
        demote.push(demote_name);
    }
    for d in demote {
        demote_state_and_velocity_partners(states, &d);
    }
}

