/// Multibody Body defaults use StateSelect.avoid for absolute pose/velocity while
/// joints use StateSelect.prefer. Primary demotion is driven by `demote_by_state_select`
/// plus Prefer-driven Default body-absolute demotion (covers unexpanded aggregates).
/// Remaining helpers cover structural redundancy (planar cuts, q/qd mirrors, sensors).

fn is_excluded_joint_alias(name: &str) -> bool {
    name.contains("Torque")
        || name.contains("torque")
        || name.contains("Sensor")
        || name.contains("sensor")
        || name.contains("flange")
        || name.contains("support")
        || name.contains("_axis")
        || name.contains("_body_")
        || name.contains("_der")
        || looks_like_frame_r0_v0(name)
}

/// RobotR3-style short revolutes: `mechanics_r1_phi`, `r2_w`, ...
fn looks_like_rn_revolute_dof(name: &str) -> bool {
    let Some((base, suf)) = name.rsplit_once('_') else {
        return false;
    };
    if suf != "phi" && suf != "w" && suf != "s" && suf != "v" {
        return false;
    }
    let Some(rpos) = base.rfind('r') else {
        return false;
    };
    if rpos > 0 && base.as_bytes().get(rpos - 1) != Some(&b'_') {
        return false;
    }
    let after = &base[rpos + 1..];
    !after.is_empty() && after.chars().all(|c| c.is_ascii_digit())
}

/// Preferred mechanical DOF states: Multibody joints and 1D drives that should
/// win over Body absolute pose (including short names like `b1_phi`, `cylinder_s`).
fn is_multibody_joint_dof_state(name: &str) -> bool {
    if is_excluded_joint_alias(name) {
        return false;
    }
    let dof = name.ends_with("_phi")
        || name.ends_with("_w")
        || name.ends_with("_s")
        || name.ends_with("_v");
    if !dof {
        return false;
    }
    let joint_kw = name.contains("revolute")
        || name.contains("prismatic")
        || name.contains("cylindrical")
        || name.contains("universal")
        || name.contains("freeMotion")
        || name.contains("FreeMotion")
        || name.contains("rollingWheel")
        || name.contains("RollingWheel")
        || name.contains("joint")
        || name.contains("bearing")
        || name.contains("inertia")
        || name.contains("Inertia")
        || name.contains("cylinder");
    if joint_kw {
        return true;
    }
    if looks_like_rn_revolute_dof(name) {
        return true;
    }
    // Short Multibody joint instances: b1_phi, j2_w, rev_phi, ...
    if let Some((base, _)) = name.rsplit_once('_') {
        if base.len() <= 4
            && (base.chars().any(|c| c.is_ascii_digit()) || base == "rev")
        {
            return true;
        }
    }
    false
}

/// Mechanics array mirrors such as `mechanics_q_1` / `mechanics_qd_2` that alias
/// joint DOFs (`mechanics_r1_phi` / `mechanics_r1_w`).
fn is_joint_q_array_state(name: &str) -> bool {
    let Some((prefix, idx)) = name.rsplit_once('_') else {
        return false;
    };
    if !idx.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let leaf = prefix.rsplit('_').next().unwrap_or("");
    leaf == "q" || leaf == "qd"
}

fn is_sensor_angle_or_speed_state(name: &str) -> bool {
    if !(name.contains("sensor") || name.contains("Sensor") || name.contains("accSensor")) {
        return false;
    }
    name.contains("_flange")
        || name.ends_with("_phi")
        || name.ends_with("_w")
        || name.ends_with("_s")
        || name.ends_with("_v")
}

fn is_drive_bearing_friction_state(name: &str) -> bool {
    (name.contains("bearingFriction") || name.contains("BearingFriction"))
        && (name.ends_with("_phi") || name.ends_with("_w"))
}

/// 1D motor mechanical angle/speed that duplicates Multibody joint DOFs.
fn is_drive_motor_mech_state(name: &str) -> bool {
    if !(name.ends_with("_phi") || name.ends_with("_w")) {
        return false;
    }
    name.contains("Jmotor")
        || name.contains("emf")
        || ((name.contains("motor") || name.contains("Motor"))
            && !name.contains("_La_")
            && !name.ends_with("_i")
            && !name.ends_with("_v"))
}

fn looks_like_frame_r0_v0(name: &str) -> bool {
    name.ends_with("_r_0")
        || name.ends_with("_v_0")
        || name.contains("_r_0_")
        || name.contains("_v_0_")
}

fn is_multibody_body_absolute_state(name: &str) -> bool {
    // FreeMotion / RollingWheel prefer their own absolute DOFs as states.
    if name.contains("freeMotion")
        || name.contains("FreeMotion")
        || name.contains("rollingWheel")
        || name.contains("RollingWheel")
    {
        return false;
    }
    if is_multibody_joint_dof_state(name) {
        return false;
    }
    if name.contains("_body_Q")
        || name.contains("_body_phi_d")
        || name.contains("_body_phi")
        || name.contains("_body_w_a")
        || name.contains("_body_v_0")
        || name.contains("_body_r_0")
        || name.contains("_body_frame_")
    {
        return true;
    }
    // BodyShape / FixedTranslation outer pose aliases (boxBody1_r_0_1).
    if looks_like_frame_r0_v0(name) {
        return true;
    }
    false
}

fn is_torque_source_local_angle(name: &str) -> bool {
    (name.contains("Torque") || name.contains("torque"))
        && (name.ends_with("_phi") || name.ends_with("_w"))
}

fn is_sensor_derivative_state(name: &str) -> bool {
    (name.contains("sensor") || name.contains("Sensor"))
        && (name.contains("_der") || name.ends_with("_u"))
}

fn demote_multibody_absolute_body_states(
    equations: &[Equation],
    declarations: &[Declaration],
    instances: &HashMap<String, String>,
    states: &mut HashSet<String>,
) -> Vec<String> {
    // StateSelect prefer/avoid is the primary Multibody selection rule.
    let mut demote = demote_by_state_select(declarations, states);
    // Body `r_0 = frame_a.r_0`: Avoid on r_0 must also demote the equated frame connector.
    demote.extend(demote_simple_aliases_of_demoted(equations, states, &demote));

    // Residual structural heuristics (q/qd mirrors, sensors, planar cuts, drive-chain).
    // Prefer+Default body-absolute demotion below is StateSelect-driven (not gated by
    // RUSTMODLICA_MB_STATE_DEMOTE). That env only disables drive-motor legacy extras.
    let demote_body_legacy = std::env::var("RUSTMODLICA_MB_STATE_DEMOTE")
        .ok()
        .map(|v| {
            let t = v.trim();
            !(t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("off"))
        })
        .unwrap_or(true);

    let demote_trace = std::env::var("RUSTMODLICA_STATE_DEMOTE_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false);

    let ranks = crate::state_select::build_state_select_map(declarations);
    let has_ss_prefer = states.iter().any(|s| {
        ranks
            .get(s)
            .copied()
            .unwrap_or(crate::ast::StateSelect::Default)
            .is_preferred()
    });
    let has_joint_dof = has_ss_prefer || states.iter().any(|s| is_multibody_joint_dof_state(s));
    let has_preferred = has_joint_dof;

    // RelativeSensor / Der intermediates must not become independent integrators.
    demote.extend(
        states
            .iter()
            .filter(|s| is_sensor_derivative_state(s) || is_sensor_angle_or_speed_state(s))
            .cloned(),
    );

    // RobotR3: `q`/`qd` arrays duplicate revolute DOFs (`rN_phi`/`rN_w`).
    demote.extend(redundant_joint_array_states(states));

    if has_preferred {
        // MSL intent: when Prefer/Always joint DOFs exist, Body absolutes that still
        // carry Default (common for unexpanded aggregates missing Avoid attrs) are
        // demoted. This is StateSelect coverage, not a name-heuristic fallback.
        demote.extend(
            states
                .iter()
                .filter(|s| {
                    if !is_multibody_body_absolute_state(s) {
                        return false;
                    }
                    let rank = ranks
                        .get(*s)
                        .copied()
                        .unwrap_or(crate::ast::StateSelect::Default);
                    !rank.is_preferred()
                })
                .cloned(),
        );

        let has_rn = looks_like_rn_revolute_dof_present(states);
        demote.extend(
            states
                .iter()
                .filter(|s| {
                    is_torque_source_local_angle(s)
                        || (demote_body_legacy
                            && has_rn
                            && (is_drive_bearing_friction_state(s)
                                || is_drive_motor_mech_state(s)))
                })
                .cloned(),
        );
    } else {
        // Free-flying body (no joint DOF): still must not keep both quaternion and
        // Euler angle integrators (MSL default useQuaternions=true prefers Q).
        demote.extend(redundant_orientation_states(states));
        // BodyShape outer r_0/v_0 vs inner Body body_* duplicates.
        demote.extend(redundant_bodyshape_pose_states(states));
        demote.extend(
            states
                .iter()
                .filter(|s| is_torque_source_local_angle(s))
                .cloned(),
        );
    }

    // Planar kinematic loops: keep the lowest-index revolute DOF when a planar
    // cut joint is present (RevolutePlanarLoopConstraint / *cut*).
    let has_loop = model_has_kinematic_loop(equations, declarations, instances);
    if has_loop {
        demote.extend(secondary_revolute_states(states));
    }
    // Engine-style planar loops: prefer 1D inertia drive over internal joint DOFs.
    demote.extend(secondary_engine_loop_joint_states(states));
    // Spatial four-bar / analytic assemblies: keep Always joint DOFs, demote
    // Prefer/Default loop joints and rod-constraint internals (rRod_0, ...).
    demote.extend(secondary_kinematic_loop_states(states, &ranks, has_loop));

    demote.sort();
    demote.dedup();
    for name in &demote {
        if demote_trace {
            eprintln!("[state-demote] multibody-absolute {name}");
        }
        states.remove(name);
    }
    demote
}

/// When StateSelect demotes Avoid vars, also demote simple `a = b` partners still selected.
fn demote_simple_aliases_of_demoted(
    equations: &[Equation],
    states: &mut HashSet<String>,
    seeds: &[String],
) -> Vec<String> {
    if seeds.is_empty() {
        return Vec::new();
    }
    let seed: HashSet<&str> = seeds.iter().map(String::as_str).collect();
    let mut extra: Vec<String> = Vec::new();
    for eq in equations {
        let Equation::Simple(lhs, rhs) = eq else {
            continue;
        };
        let (Expression::Variable(lid), Expression::Variable(rid)) = (lhs, rhs) else {
            continue;
        };
        let a = crate::string_intern::resolve_id(*lid);
        let b = crate::string_intern::resolve_id(*rid);
        if seed.contains(a.as_str()) && states.contains(&b) {
            extra.push(b);
        } else if seed.contains(b.as_str()) && states.contains(&a) {
            extra.push(a);
        }
    }
    // Body `r_0 = frame_a.r_0` may not be scalar Variable=Variable after flatten;
    // still demote `*_frame_a_r_0_*` when matching `*_r_0_*` was Avoid-demoted.
    for s in seeds {
        if let Some(frame) = paired_frame_a_r0(s) {
            if states.contains(&frame) {
                extra.push(frame);
            }
        }
    }
    extra.sort();
    extra.dedup();
    for name in &extra {
        states.remove(name);
    }
    extra
}

fn paired_frame_a_r0(name: &str) -> Option<String> {
    let (prefix, idx) = name.rsplit_once("_r_0_")?;
    if !idx.chars().all(|c| c.is_ascii_digit()) || prefix.contains("_frame_") {
        return None;
    }
    Some(format!("{prefix}_frame_a_r_0_{idx}"))
}

fn type_is_kinematic_loop_joint(t: &str) -> bool {
    t.contains("PlanarLoop")
        || t.contains("RevolutePlanarLoopConstraint")
        || t.contains("SphericalSpherical")
        || t.contains("UniversalSpherical")
        || t.contains("JointSSP")
        || t.contains("JointUSR")
        || t.contains("JointSSR")
        || t.contains("JointRRR")
        || t.contains("JointUSP")
        || t.contains("JointRRP")
}

fn model_has_kinematic_loop(
    equations: &[Equation],
    declarations: &[Declaration],
    instances: &HashMap<String, String>,
) -> bool {
    if declarations
        .iter()
        .any(|d| type_is_kinematic_loop_joint(&d.type_name))
    {
        return true;
    }
    if instances.values().any(|t| type_is_kinematic_loop_joint(t)) {
        return true;
    }
    let empty = HashSet::new();
    for eq in equations {
        for v in extract_unknowns(eq, &empty) {
            if v.contains("PlanarLoop")
                || v.contains("planarLoop")
                || v.contains("cut")
                || v.contains("Cut")
                || v.contains("universalSpherical")
                || v.contains("UniversalSpherical")
                || v.contains("jointSSP")
                || v.contains("JointSSP")
            {
                return true;
            }
        }
    }
    false
}

fn is_loop_assembly_internal_state(name: &str) -> bool {
    name.contains("_rRod_0")
        || name.contains("rRod_0_")
        || name.contains("_v_CM_0")
        || name.contains("_prismatic_distance")
        || name.ends_with("_axis_s")
        || name.contains("constraintResidue")
}

fn joint_dof_instance_count(states: &HashSet<String>) -> usize {
    let mut inst: HashSet<&str> = HashSet::new();
    for s in states {
        if !is_multibody_joint_dof_state(s) {
            continue;
        }
        let Some((base, suf)) = s.rsplit_once('_') else {
            continue;
        };
        if suf == "phi" || suf == "w" || suf == "s" || suf == "v" {
            inst.insert(base);
        }
    }
    inst.len()
}

/// Closed kinematic loops: Always joint DOFs stay; Prefer/Default loop joints
/// and assembly rod constraints become algebraic. Gated so independent parallel
/// joints (SpringMassSystem: Always p2 + Prefer p1) are not collapsed.
fn secondary_kinematic_loop_states(
    states: &HashSet<String>,
    ranks: &HashMap<String, crate::ast::StateSelect>,
    has_loop_assembly: bool,
) -> Vec<String> {
    let has_always_joint = states.iter().any(|s| {
        is_multibody_joint_dof_state(s)
            && ranks.get(s).copied().unwrap_or(crate::ast::StateSelect::Default)
                == crate::ast::StateSelect::Always
    });
    let closed_chain = has_always_joint && joint_dof_instance_count(states) >= 3;
    if !has_loop_assembly && !closed_chain {
        return Vec::new();
    }
    states
        .iter()
        .filter(|s| {
            if is_loop_assembly_internal_state(s) {
                return true;
            }
            if !is_multibody_joint_dof_state(s) {
                return false;
            }
            ranks.get(*s).copied().unwrap_or(crate::ast::StateSelect::Default)
                != crate::ast::StateSelect::Always
        })
        .cloned()
        .collect()
}

fn revolute_instance_index(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("revolute")?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn secondary_revolute_states(states: &HashSet<String>) -> Vec<String> {
    let mut indices: Vec<u32> = states
        .iter()
        .filter_map(|s| {
            if s.ends_with("_phi") || s.ends_with("_w") {
                revolute_instance_index(s)
            } else {
                None
            }
        })
        .collect();
    indices.sort_unstable();
    indices.dedup();
    if indices.len() < 2 {
        return Vec::new();
    }
    let keep = indices[0];
    states
        .iter()
        .filter(|s| {
            matches!(revolute_instance_index(s), Some(i) if i != keep)
                && (s.ends_with("_phi") || s.ends_with("_w"))
        })
        .cloned()
        .collect()
}

/// When a planar cut loop is driven by a 1D `inertia_*` state, demote internal
/// Multibody joint DOFs (`b1_phi`, `cylinder_s`, `bearing_phi`, ...).
fn secondary_engine_loop_joint_states(states: &HashSet<String>) -> Vec<String> {
    let has_inertia = states
        .iter()
        .any(|s| s.contains("inertia") && (s.ends_with("_phi") || s.ends_with("_w")));
    if !has_inertia {
        return Vec::new();
    }
    states
        .iter()
        .filter(|s| {
            if s.contains("inertia") {
                return false;
            }
            is_multibody_joint_dof_state(s)
        })
        .cloned()
        .collect()
}

fn looks_like_rn_revolute_dof_present(states: &HashSet<String>) -> bool {
    states.iter().any(|s| looks_like_rn_revolute_dof(s))
}

fn redundant_joint_array_states(states: &HashSet<String>) -> Vec<String> {
    let has_physical_joint = states.iter().any(|s| is_multibody_joint_dof_state(s));
    if !has_physical_joint {
        return Vec::new();
    }
    let has_q = states.iter().any(|s| is_joint_q_array_state(s));
    if !has_q {
        return Vec::new();
    }
    states
        .iter()
        .filter(|s| is_joint_q_array_state(s))
        .cloned()
        .collect()
}

/// When both `*_body_Q*` and Euler/`w_a` are states, demote the non-quaternion set.
fn redundant_orientation_states(states: &HashSet<String>) -> Vec<String> {
    let has_q = states.iter().any(|s| s.contains("_body_Q"));
    if !has_q {
        return Vec::new();
    }
    states
        .iter()
        .filter(|s| {
            s.contains("_body_phi_d")
                || s.contains("_body_phi")
                || s.contains("_body_w_a")
        })
        .cloned()
        .collect()
}

/// Prefer BodyShape `*_r_0*` / `*_v_0*` over nested `*_body_r_0*` / `*_body_v_0*`
/// when both layers are selected (same free-body pose twice).
fn redundant_bodyshape_pose_states(states: &HashSet<String>) -> Vec<String> {
    let has_outer_r0 = states
        .iter()
        .any(|s| looks_like_frame_r0_v0(s) && !s.contains("_body_"));
    if !has_outer_r0 {
        return Vec::new();
    }
    states
        .iter()
        .filter(|s| {
            (s.contains("_body_r_0") || s.contains("_body_v_0") || s.contains("_body_frame_"))
                && looks_like_frame_r0_v0(s)
        })
        .cloned()
        .collect()
}
