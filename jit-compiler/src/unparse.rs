// Minimal Modelica AST to .mo text for diagram round-trip (declarations + connect + other equations).

use crate::ast::*;
use std::fmt::Write;

fn write_expression(buf: &mut String, e: &Expression) {
    match e {
        Expression::Variable(s) => {
            buf.push_str(s);
        }
        Expression::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                let _ = write!(buf, "{}", *n as i64);
            } else {
                let _ = write!(buf, "{}", n);
            }
        }
        Expression::BinaryOp(l, op, r) => {
            buf.push('(');
            write_expression(buf, l);
            let s = match op {
                Operator::Add => " + ",
                Operator::Sub => " - ",
                Operator::Mul => " * ",
                Operator::Div => " / ",
                Operator::Less => " < ",
                Operator::Greater => " > ",
                Operator::LessEq => " <= ",
                Operator::GreaterEq => " >= ",
                Operator::Equal => " == ",
                Operator::NotEqual => " <> ",
                Operator::And => " and ",
                Operator::Or => " or ",
            };
            buf.push_str(s);
            write_expression(buf, r);
            buf.push(')');
        }
        Expression::Call(name, args) => {
            buf.push_str(name);
            buf.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                write_expression(buf, a);
            }
            buf.push(')');
        }
        Expression::Der(inner) => {
            buf.push_str("der(");
            write_expression(buf, inner);
            buf.push(')');
        }
        Expression::Dot(inner, name) => {
            write_expression(buf, inner);
            buf.push('.');
            buf.push_str(name);
        }
        Expression::If(cond, t, f) => {
            buf.push_str("if ");
            write_expression(buf, cond);
            buf.push_str(" then ");
            write_expression(buf, t);
            buf.push_str(" else ");
            write_expression(buf, f);
        }
        Expression::ArrayAccess(base, idx) => {
            write_expression(buf, base);
            buf.push('[');
            write_expression(buf, idx);
            buf.push(']');
        }
        Expression::Range(s, step, e) => {
            write_expression(buf, s);
            buf.push_str(" : ");
            write_expression(buf, step);
            buf.push_str(" : ");
            write_expression(buf, e);
        }
        Expression::ArrayLiteral(els) => {
            buf.push('{');
            for (i, el) in els.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                write_expression(buf, el);
            }
            buf.push('}');
        }
        Expression::StringLiteral(s) => {
            buf.push('"');
            for c in s.chars() {
                if c == '"' || c == '\\' {
                    buf.push('\\');
                }
                buf.push(c);
            }
            buf.push('"');
        }
        Expression::Sample(inner) => {
            buf.push_str("sample(");
            write_expression(buf, inner);
            buf.push(')');
        }
        Expression::Interval(inner) => {
            buf.push_str("interval(");
            write_expression(buf, inner);
            buf.push(')');
        }
        Expression::Hold(inner) => {
            buf.push_str("hold(");
            write_expression(buf, inner);
            buf.push(')');
        }
        Expression::Previous(inner) => {
            buf.push_str("previous(");
            write_expression(buf, inner);
            buf.push(')');
        }
        Expression::SubSample(a, b) => {
            buf.push_str("subSample(");
            write_expression(buf, a);
            buf.push_str(", ");
            write_expression(buf, b);
            buf.push(')');
        }
        Expression::SuperSample(a, b) => {
            buf.push_str("superSample(");
            write_expression(buf, a);
            buf.push_str(", ");
            write_expression(buf, b);
            buf.push(')');
        }
        Expression::ShiftSample(a, b) => {
            buf.push_str("shiftSample(");
            write_expression(buf, a);
            buf.push_str(", ");
            write_expression(buf, b);
            buf.push(')');
        }
    }
}

fn write_modification(buf: &mut String, m: &Modification) {
    if m.redeclare {
        buf.push_str("redeclare ");
    }
    if m.each {
        buf.push_str("each ");
    }
    buf.push_str(&m.name);
    if let Some(ref v) = m.value {
        buf.push_str(" = ");
        write_expression(buf, v);
    }
}

fn write_declaration(buf: &mut String, d: &Declaration) {
    if d.is_parameter {
        buf.push_str("  parameter ");
    } else if d.is_flow {
        buf.push_str("  flow ");
    } else if d.is_discrete {
        buf.push_str("  discrete ");
    } else if d.is_input {
        buf.push_str("  input ");
    } else if d.is_output {
        buf.push_str("  output ");
    } else {
        buf.push_str("  ");
    }
    buf.push_str(&d.type_name);
    buf.push(' ');
    buf.push_str(&d.name);
    if let Some(ref sz) = d.array_size {
        buf.push('[');
        write_expression(buf, sz);
        buf.push(']');
    }
    let has_mod = d.start_value.is_some() || !d.modifications.is_empty();
    if has_mod {
        buf.push_str("(");
        if let Some(ref v) = d.start_value {
            write_expression(buf, v);
            if !d.modifications.is_empty() {
                buf.push_str(", ");
            }
        }
        for (i, m) in d.modifications.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            write_modification(buf, m);
        }
        buf.push_str(")");
    }
    buf.push_str(";\n");
}

fn write_equation(buf: &mut String, eq: &Equation) {
    match eq {
        Equation::Simple(lhs, rhs) => {
            buf.push_str("  ");
            write_expression(buf, lhs);
            buf.push_str(" = ");
            write_expression(buf, rhs);
            buf.push_str(";\n");
        }
        Equation::Connect(a, b) => {
            buf.push_str("  connect(");
            write_expression(buf, a);
            buf.push_str(", ");
            write_expression(buf, b);
            buf.push_str(");\n");
        }
        Equation::MultiAssign(lhss, rhs) => {
            buf.push_str("  (");
            for (i, l) in lhss.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                write_expression(buf, l);
            }
            buf.push_str(") = ");
            write_expression(buf, rhs);
            buf.push_str(";\n");
        }
        Equation::For(var, start, end, body) => {
            buf.push_str("  for ");
            buf.push_str(var);
            buf.push_str(" in ");
            write_expression(buf, start);
            buf.push_str(" : ");
            write_expression(buf, end);
            buf.push_str(" loop\n");
            for e in body {
                write_equation(buf, e);
            }
            buf.push_str("  end for;\n");
        }
        Equation::When(cond, body, else_whens) => {
            buf.push_str("  when ");
            write_expression(buf, cond);
            buf.push_str(" then\n");
            for e in body {
                write_equation(buf, e);
            }
            for (c, eqs) in else_whens {
                buf.push_str("  elsewhen ");
                write_expression(buf, c);
                buf.push_str(" then\n");
                for e in eqs {
                    write_equation(buf, e);
                }
            }
            buf.push_str("  end when;\n");
        }
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => {
            buf.push_str("  if ");
            write_expression(buf, cond);
            buf.push_str(" then\n");
            for e in then_eqs {
                write_equation(buf, e);
            }
            for (c, eqs) in elseif_list {
                buf.push_str("  elseif ");
                write_expression(buf, c);
                buf.push_str(" then\n");
                for e in eqs {
                    write_equation(buf, e);
                }
            }
            if let Some(ref eqs) = else_eqs {
                buf.push_str("  else\n");
                for e in eqs {
                    write_equation(buf, e);
                }
            }
            buf.push_str("  end if;\n");
        }
        Equation::Reinit(var, expr) => {
            buf.push_str("  reinit(");
            buf.push_str(var);
            buf.push_str(", ");
            write_expression(buf, expr);
            buf.push_str(");\n");
        }
        Equation::Assert(cond, msg) => {
            buf.push_str("  assert(");
            write_expression(buf, cond);
            buf.push_str(", ");
            write_expression(buf, msg);
            buf.push_str(");\n");
        }
        Equation::Terminate(msg) => {
            buf.push_str("  terminate(");
            write_expression(buf, msg);
            buf.push_str(");\n");
        }
        Equation::SolvableBlock {
            unknowns,
            tearing_var,
            equations,
            residuals,
        } => {
            buf.push_str("  (");
            for (i, u) in unknowns.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                buf.push_str(u);
            }
            buf.push_str(") = block(");
            if let Some(ref t) = tearing_var {
                buf.push_str("tearing var ");
                buf.push_str(t);
                buf.push_str("; ");
            }
            for e in equations {
                write_equation(buf, e);
            }
            for (i, r) in residuals.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                write_expression(buf, r);
            }
            buf.push_str(");\n");
        }
    }
}

fn write_extends(buf: &mut String, e: &ExtendsClause) {
    buf.push_str("  extends ");
    buf.push_str(&e.model_name);
    if !e.modifications.is_empty() {
        buf.push_str("(");
        for (i, m) in e.modifications.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            write_modification(buf, m);
        }
        buf.push_str(")");
    }
    buf.push_str(";\n");
}

/// Serializes a Model to .mo source (single top-level class).
pub fn model_to_mo(m: &Model) -> String {
    let mut buf = String::new();
    if m.is_connector {
        buf.push_str("connector ");
    } else if m.is_block {
        buf.push_str("block ");
    } else if m.is_record {
        buf.push_str("record ");
    } else if m.is_function {
        buf.push_str("function ");
    } else {
        buf.push_str("model ");
    }
    buf.push_str(&m.name);
    buf.push_str(";\n");

    for e in &m.extends {
        write_extends(&mut buf, e);
    }
    for d in &m.declarations {
        write_declaration(&mut buf, d);
    }
    if !m.equations.is_empty() {
        buf.push_str("equation\n");
        for eq in &m.equations {
            write_equation(&mut buf, eq);
        }
    }
    if !m.initial_equations.is_empty() {
        buf.push_str("initial equation\n");
        for eq in &m.initial_equations {
            write_equation(&mut buf, eq);
        }
    }
    if !m.algorithms.is_empty() {
        buf.push_str("algorithm\n");
        for a in &m.algorithms {
            write_algorithm_statement(&mut buf, a);
        }
    }
    if !m.initial_algorithms.is_empty() {
        buf.push_str("initial algorithm\n");
        for a in &m.initial_algorithms {
            write_algorithm_statement(&mut buf, a);
        }
    }
    buf.push_str("end ");
    buf.push_str(&m.name);
    if let Some(ref ann) = m.annotation {
        let trimmed = ann.trim();
        let trimmed = trimmed.strip_suffix(';').unwrap_or(trimmed).trim();
        if !trimmed.is_empty() {
            buf.push_str(" ");
            buf.push_str(trimmed);
        }
    }
    buf.push_str(";\n");
    buf
}

fn write_algorithm_statement(buf: &mut String, a: &AlgorithmStatement) {
    match a {
        AlgorithmStatement::Assignment(lhs, rhs) => {
            buf.push_str("  ");
            write_expression(buf, lhs);
            buf.push_str(" := ");
            write_expression(buf, rhs);
            buf.push_str(";\n");
        }
        AlgorithmStatement::Reinit(var, expr) => {
            buf.push_str("  reinit(");
            buf.push_str(var);
            buf.push_str(", ");
            write_expression(buf, expr);
            buf.push_str(");\n");
        }
        AlgorithmStatement::If(cond, then_s, elseif_list, else_s) => {
            buf.push_str("  if ");
            write_expression(buf, cond);
            buf.push_str(" then\n");
            for s in then_s {
                write_algorithm_statement(buf, s);
            }
            for (c, stmts) in elseif_list {
                buf.push_str("  elseif ");
                write_expression(buf, c);
                buf.push_str(" then\n");
                for s in stmts {
                    write_algorithm_statement(buf, s);
                }
            }
            if let Some(ref stmts) = else_s {
                buf.push_str("  else\n");
                for s in stmts {
                    write_algorithm_statement(buf, s);
                }
            }
            buf.push_str("  end if;\n");
        }
        AlgorithmStatement::For(var, range, body) => {
            buf.push_str("  for ");
            buf.push_str(var);
            buf.push_str(" in ");
            write_expression(buf, range);
            buf.push_str(" loop\n");
            for s in body {
                write_algorithm_statement(buf, s);
            }
            buf.push_str("  end for;\n");
        }
        AlgorithmStatement::While(cond, body) => {
            buf.push_str("  while ");
            write_expression(buf, cond);
            buf.push_str(" loop\n");
            for s in body {
                write_algorithm_statement(buf, s);
            }
            buf.push_str("  end while;\n");
        }
        AlgorithmStatement::When(cond, body, else_whens) => {
            buf.push_str("  when ");
            write_expression(buf, cond);
            buf.push_str(" then\n");
            for s in body {
                write_algorithm_statement(buf, s);
            }
            for (c, stmts) in else_whens {
                buf.push_str("  elsewhen ");
                write_expression(buf, c);
                buf.push_str(" then\n");
                for s in stmts {
                    write_algorithm_statement(buf, s);
                }
            }
            buf.push_str("  end when;\n");
        }
        AlgorithmStatement::Assert(cond, msg) => {
            buf.push_str("  assert(");
            write_expression(buf, cond);
            buf.push_str(", ");
            write_expression(buf, msg);
            buf.push_str(");\n");
        }
        AlgorithmStatement::Terminate(msg) => {
            buf.push_str("  terminate(");
            write_expression(buf, msg);
            buf.push_str(");\n");
        }
    }
}
