fn is_relative_angle_state(name: &str) -> bool {
    name.ends_with("_phi_rel") || name.ends_with("_s_rel")
}

fn relative_velocity_partner(phi_rel: &str) -> Option<String> {
    if let Some(base) = phi_rel.strip_suffix("_phi_rel") {
        return Some(format!("{}_w_rel", base));
    }
    if let Some(base) = phi_rel.strip_suffix("_s_rel") {
        return Some(format!("{}_v_rel", base));
    }
    None
}

fn is_absolute_mech_state(name: &str, states: &HashSet<String>) -> bool {
    states.contains(name)
        && !is_relative_angle_state(name)
        && !name.contains("_flange")
        && !name.ends_with("_w_rel")
        && !name.ends_with("_v_rel")
}

/// Affine form: sum_i (scale_i * var_i) + offset.
#[derive(Clone, Debug, Default)]
struct AffinePot {
    terms: HashMap<String, f64>,
    offset: f64,
}

impl AffinePot {
    fn from_var(name: &str) -> Self {
        let mut terms = HashMap::new();
        terms.insert(name.to_string(), 1.0);
        Self { terms, offset: 0.0 }
    }
    fn from_const(c: f64) -> Self {
        Self {
            terms: HashMap::new(),
            offset: c,
        }
    }
    fn add(&self, other: &Self) -> Self {
        let mut out = self.clone();
        out.offset += other.offset;
        for (k, s) in &other.terms {
            *out.terms.entry(k.clone()).or_insert(0.0) += *s;
        }
        out.terms.retain(|_, s| s.abs() > 1e-15);
        out
    }
    fn sub(&self, other: &Self) -> Self {
        let mut neg = other.clone();
        neg.offset = -neg.offset;
        for s in neg.terms.values_mut() {
            *s = -*s;
        }
        self.add(&neg)
    }
    fn scale(&self, c: f64) -> Self {
        let mut out = self.clone();
        out.offset *= c;
        for s in out.terms.values_mut() {
            *s *= c;
        }
        out.terms.retain(|_, s| s.abs() > 1e-15);
        out
    }
}

fn parse_affine_expr(expr: &Expression) -> Option<AffinePot> {
    use crate::ast::Operator;
    use crate::string_intern::resolve_id;
    match expr {
        Expression::Number(n) => Some(AffinePot::from_const(*n)),
        Expression::Variable(id) => Some(AffinePot::from_var(&resolve_id(*id))),
        Expression::BinaryOp(l, Operator::Add, r) => {
            Some(parse_affine_expr(l)?.add(&parse_affine_expr(r)?))
        }
        Expression::BinaryOp(l, Operator::Sub, r) => {
            Some(parse_affine_expr(l)?.sub(&parse_affine_expr(r)?))
        }
        Expression::BinaryOp(l, Operator::Mul, r) => match (l.as_ref(), r.as_ref()) {
            (Expression::Number(c), other) | (other, Expression::Number(c)) => {
                Some(parse_affine_expr(other)?.scale(*c))
            }
            _ => None,
        },
        Expression::BinaryOp(l, Operator::Div, r) => {
            if let Expression::Number(c) = r.as_ref() {
                if c.abs() > 1e-15 {
                    return Some(parse_affine_expr(l)?.scale(1.0 / *c));
                }
            }
            None
        }
        _ => None,
    }
}

/// Damper/Spring `phi_rel`/`s_rel = flange_b - flange_a` is redundant once flanges
/// resolve to absolute states (and offsets like mass L/2) or Fixed constants.
fn demote_dependent_relative_states(equations: &[Equation], states: &mut HashSet<String>) {
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
    // Some connect expansions equate connector roots (mass1_flange_a = damper_flange_a)
    // without the potential suffix; relative laws use *_s / *_phi. Mirror both.
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
    // Root -> preferred affine (const / absolute state / mass_s+offset).
    let mut root_affine: HashMap<String, AffinePot> = HashMap::new();

    let demote_trace_early = std::env::var("RUSTMODLICA_STATE_DEMOTE_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false);

    let affine_rank = |a: &AffinePot| -> i32 {
        if a.terms.keys().any(|v| is_absolute_mech_state(v, states)) {
            3
        } else if a.terms.is_empty() {
            2
        } else {
            1
        }
    };
    let set_root_affine = |root_affine: &mut HashMap<String, AffinePot>, root: String, val: AffinePot| {
        match root_affine.get(&root) {
            Some(old) if affine_rank(old) >= affine_rank(&val) => {}
            _ => {
                root_affine.insert(root, val);
            }
        }
    };

    // Pass 1: equate variables (flange <-> flange, flange <-> state).
    for eq in equations {
        let Equation::Simple(lhs, rhs) = eq else {
            continue;
        };
        if let (Expression::Variable(lid), Expression::Variable(rid)) = (lhs, rhs) {
            let a = resolve_id(*lid);
            let b = resolve_id(*rid);
            if is_relative_angle_state(&a) || is_relative_angle_state(&b) {
                continue;
            }
            union_potential_pair(&mut parent, &a, &b);
        }
    }

    // Re-root components so an absolute state becomes the representative when present.
    {
        let names: Vec<String> = parent.keys().cloned().collect();
        for name in &names {
            if is_absolute_mech_state(name, states) {
                let r = find(&mut parent, name);
                if r != *name {
                    parent.insert(r, name.clone());
                    parent.insert(name.clone(), name.clone());
                }
            }
        }
    }

    // Pass 2: attach consts and affine flange defs (mass_s +/- L/2) onto roots.
    // Also union each flange with absolute states mentioned in its affine so
    // mass.s enters the component even without a Variable=Variable equality.
    for eq in equations {
        let Equation::Simple(lhs, rhs) = eq else {
            continue;
        };
        match (lhs, rhs) {
            (Expression::Variable(lid), Expression::Number(n))
            | (Expression::Number(n), Expression::Variable(lid)) => {
                let v = resolve_id(*lid);
                if is_relative_angle_state(&v) {
                    continue;
                }
                let root = find(&mut parent, &v);
                set_root_affine(&mut root_affine, root, AffinePot::from_const(*n));
            }
            (Expression::Variable(lid), other) => {
                let v = resolve_id(*lid);
                if is_relative_angle_state(&v) || is_absolute_mech_state(&v, states) {
                    continue;
                }
                if let Some(aff) = parse_affine_expr(other) {
                    // Union flange with every term (rod center `s`, mass `s`, params).
                    for t in aff.terms.keys() {
                        union(&mut parent, &v, t);
                    }
                    // Attach when it mentions an absolute state/const, or a single
                    // center potential (PartialRigid: flange = s +/- L/2).
                    let attach = affine_rank(&aff) >= 2
                        || (aff.terms.len() == 1
                            && aff
                                .terms
                                .keys()
                                .next()
                                .map(|t| t.ends_with("_s") || t.ends_with("_phi"))
                                .unwrap_or(false));
                    if attach {
                        let root = find(&mut parent, &v);
                        set_root_affine(&mut root_affine, root, aff);
                    }
                }
            }
            (other, Expression::Variable(rid)) => {
                let v = resolve_id(*rid);
                if is_relative_angle_state(&v) || is_absolute_mech_state(&v, states) {
                    continue;
                }
                if let Some(aff) = parse_affine_expr(other) {
                    // Union flange with every term (rod center `s`, mass `s`, params).
                    for t in aff.terms.keys() {
                        union(&mut parent, &v, t);
                    }
                    // Attach when it mentions an absolute state/const, or a single
                    // center potential (PartialRigid: flange = s +/- L/2).
                    let attach = affine_rank(&aff) >= 2
                        || (aff.terms.len() == 1
                            && aff
                                .terms
                                .keys()
                                .next()
                                .map(|t| t.ends_with("_s") || t.ends_with("_phi"))
                                .unwrap_or(false));
                    if attach {
                        let root = find(&mut parent, &v);
                        set_root_affine(&mut root_affine, root, aff);
                    }
                }
            }
            _ => {}
        }
    }

    // Re-root again after affine unions pulled absolute states into components.
    {
        let names: Vec<String> = parent.keys().cloned().collect();
        for name in &names {
            if is_absolute_mech_state(name, states) {
                let r = find(&mut parent, name);
                if r != *name {
                    parent.insert(r, name.clone());
                    parent.insert(name.clone(), name.clone());
                }
            }
        }
        // Move affine attachments onto the new absolute roots.
        let old = root_affine.clone();
        root_affine.clear();
        for (prev_root, aff) in old {
            let new_root = find(&mut parent, &prev_root);
            set_root_affine(&mut root_affine, new_root, aff);
        }
    }

    // If a component root is an absolute state without explicit affine, use the state itself.
    for s in states.iter() {
        if is_absolute_mech_state(s, states) && parent.contains_key(s) {
            let root = find(&mut parent, s);
            set_root_affine(
                &mut root_affine,
                root,
                AffinePot::from_var(s),
            );
        }
    }

    let resolve_name = |parent: &mut HashMap<String, String>,
                        root_affine: &HashMap<String, AffinePot>,
                        name: &str|
     -> AffinePot {
        let root = find(parent, name);
        if let Some(a) = root_affine.get(&root) {
            a.clone()
        } else if is_absolute_mech_state(&root, states) {
            AffinePot::from_var(&root)
        } else {
            // Prefer union-find root (e.g. Fixed.s0 parameter) over the flange name.
            AffinePot::from_var(&root)
        }
    };

    let demote_trace = demote_trace_early;

    let mut demote_rel: Vec<String> = Vec::new();
    for eq in equations {
        let Equation::Simple(lhs, rhs) = eq else {
            continue;
        };
        let Expression::Variable(lid) = lhs else {
            continue;
        };
        let rel = resolve_id(*lid);
        if !states.contains(&rel) || !is_relative_angle_state(&rel) {
            continue;
        }
        let Some(raw) = parse_affine_expr(rhs) else {
            if demote_trace {
                eprintln!("[state-demote] skip {rel}: rhs not affine");
            }
            continue;
        };
        let mut expanded = AffinePot::from_const(raw.offset);
        for (v, scale) in &raw.terms {
            expanded = expanded.add(&resolve_name(&mut parent, &root_affine, v).scale(*scale));
        }
        expanded.terms.remove(&rel);
        if demote_trace {
            let terms: Vec<String> = expanded
                .terms
                .iter()
                .map(|(k, s)| format!("{s}*{k}"))
                .collect();
            eprintln!(
                "[state-demote] {rel} => offset={} terms=[{}]",
                expanded.offset,
                terms.join(", ")
            );
        }
        let ok = expanded.terms.keys().all(|v| {
            is_absolute_mech_state(v, states)
                || (!states.contains(v) && !is_relative_angle_state(v))
        });
        let binds_abs = expanded
            .terms
            .keys()
            .any(|v| is_absolute_mech_state(v, states));
        let pure_const = expanded.terms.is_empty();
        if ok && (binds_abs || pure_const) {
            demote_rel.push(rel);
        } else if demote_trace {
            eprintln!(
                "[state-demote] keep {rel}: ok={ok} binds_abs={binds_abs} pure_const={pure_const}"
            );
        }
    }

    for rel in demote_rel {
        states.remove(&rel);
        if let Some(w) = relative_velocity_partner(&rel) {
            states.remove(&w);
        }
    }
}

fn mech_component_prefix(name: &str) -> &str {
    for suf in [
        "_flange_a_s",
        "_flange_b_s",
        "_flange_a_phi",
        "_flange_b_phi",
        "_flange_s",
        "_flange_phi",
        "_phi_rel",
        "_s_rel",
        "_w_rel",
        "_v_rel",
        "_phi_a",
        "_phi_b",
        "_w_a",
        "_w_b",
        "_s_a",
        "_s_b",
        "_v_a",
        "_v_b",
        "_phi",
        "_s",
        "_w",
        "_v",
        "_a",
    ] {
        if let Some(p) = name.strip_suffix(suf) {
            return p;
        }
    }
    name
}


/// AccSensor/derived: `v = der(flange.s); a = der(v)` marks `v` as a state, but `v` is
/// algebraically the velocity of an absolute position that already has its own
/// integrator (mass.v / inertia.w). Demote the sensor-local velocity.
fn demote_redundant_sensor_velocity_states(equations: &[Equation], states: &mut HashSet<String>) {
    use crate::string_intern::resolve_id;

    let mut pos_to_vel: HashMap<String, HashSet<String>> = HashMap::new();

    for eq in equations {
        let Equation::Simple(lhs, rhs) = eq else {
            continue;
        };
        let Expression::Variable(vid) = lhs else {
            continue;
        };
        let vel = resolve_id(*vid);
        if !states.contains(&vel) {
            continue;
        }
        let pos = match rhs {
            Expression::Der(inner) => match inner.as_ref() {
                Expression::Variable(id) => Some(resolve_id(*id)),
                other => crate::analysis::flatten_dot_to_name(other),
            },
            Expression::Variable(id) => {
                let n = resolve_id(*id);
                n.strip_prefix("der_").map(|s| s.to_string())
            }
            _ => None,
        };
        let Some(pos) = pos else {
            continue;
        };
        pos_to_vel.entry(pos).or_default().insert(vel);
    }
    if pos_to_vel.is_empty() {
        return;
    }

    let mut parent: HashMap<String, String> = HashMap::new();
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
    for eq in equations {
        let Equation::Simple(lhs, rhs) = eq else {
            continue;
        };
        if let (Expression::Variable(a), Expression::Variable(b)) = (lhs, rhs) {
            let an = resolve_id(*a);
            let bn = resolve_id(*b);
            if an.contains("_flange")
                || bn.contains("_flange")
                || an.ends_with("_s")
                || an.ends_with("_phi")
                || bn.ends_with("_s")
                || bn.ends_with("_phi")
            {
                union(&mut parent, &an, &bn);
                let a_pot = an.ends_with("_s") || an.ends_with("_phi");
                let b_pot = bn.ends_with("_s") || bn.ends_with("_phi");
                if !a_pot && !b_pot && (an.contains("_flange") || bn.contains("_flange")) {
                    for suf in ["_s", "_phi"] {
                        union(&mut parent, &format!("{an}{suf}"), &format!("{bn}{suf}"));
                    }
                }
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

    let mut root_vels: HashMap<String, HashSet<String>> = HashMap::new();
    for (pos, vels) in &pos_to_vel {
        let root = find(&mut parent, pos);
        root_vels.entry(root).or_default().extend(vels.iter().cloned());
    }
    for s in states.iter() {
        if !is_absolute_mech_state(s, states) {
            continue;
        }
        let root = find(&mut parent, s);
        let e = root_vels.entry(root).or_default();
        if let Some(base) = s.strip_suffix("_s") {
            e.insert(format!("{base}_v"));
        } else if let Some(base) = s.strip_suffix("_phi") {
            e.insert(format!("{base}_w"));
        }
    }

    let mut demote: Vec<String> = Vec::new();
    for (pos, vels) in &pos_to_vel {
        let root = find(&mut parent, pos);
        let Some(group) = root_vels.get(&root) else {
            continue;
        };
        let abs_pos = states
            .iter()
            .find(|s| is_absolute_mech_state(s, states) && find(&mut parent, s) == root)
            .cloned();
        let keep_prefix = abs_pos
            .as_ref()
            .map(|p| mech_component_prefix(p).to_string());
        for v in vels {
            if !states.contains(v) {
                continue;
            }
            let same_owner = keep_prefix
                .as_ref()
                .map(|p| mech_component_prefix(v) == p.as_str())
                .unwrap_or(false);
            if same_owner {
                continue;
            }
            // Only AccSensor-style locals: component name contains "Sensor".
            // Do not demote sibling mass/inertia velocities even if flange unions
            // accidentally share a potential root across a contact network.
            if !mech_component_prefix(v).contains("Sensor") {
                continue;
            }
            let has_keeper = keep_prefix
                .as_ref()
                .map(|p| {
                    let cand_v = format!("{p}_v");
                    let cand_w = format!("{p}_w");
                    group.iter().any(|w| {
                        w != v
                            && states.contains(w)
                            && (w == &cand_v
                                || w == &cand_w
                                || mech_component_prefix(w) == p.as_str())
                    })
                })
                .unwrap_or(false);
            if has_keeper {
                demote.push(v.clone());
            }
        }
    }

    for v in demote {
        states.remove(&v);
    }
}


