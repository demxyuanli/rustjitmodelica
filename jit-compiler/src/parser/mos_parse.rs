use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "src/mos.pest"]
pub struct MosParser;

#[derive(Debug, Clone)]
pub struct MosSpan {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub enum MosExpr {
    Number(f64),
    String(String),
    Bool(bool),
    Ident(String),
    Call {
        name: String,
        args: Vec<MosArg>,
    },
    Array(Vec<MosExpr>),
    Record(Vec<(String, MosExpr)>),
    Unary {
        op: String,
        expr: Box<MosExpr>,
    },
    Binary {
        op: String,
        left: Box<MosExpr>,
        right: Box<MosExpr>,
    },
    Range {
        start: Box<MosExpr>,
        step: Option<Box<MosExpr>>,
        stop: Box<MosExpr>,
    },
}

#[derive(Debug, Clone)]
pub struct MosArg {
    pub name: Option<String>,
    pub value: MosExpr,
}

#[derive(Debug, Clone)]
pub enum MosStmt {
    Expr {
        expr: MosExpr,
        span: MosSpan,
    },
    Assign {
        name: String,
        value: MosExpr,
        span: MosSpan,
    },
    If {
        cond: MosExpr,
        then_body: Vec<MosStmt>,
        elseif: Vec<(MosExpr, Vec<MosStmt>)>,
        else_body: Vec<MosStmt>,
        span: MosSpan,
    },
    For {
        var: String,
        iter: MosExpr,
        body: Vec<MosStmt>,
        span: MosSpan,
    },
}

fn span_of(p: &Pair<Rule>) -> MosSpan {
    let (line, col) = p.as_span().start_pos().line_col();
    MosSpan { line, col }
}

fn parse_primary(pair: Pair<Rule>) -> Result<MosExpr, String> {
    match pair.as_rule() {
        Rule::number => pair
            .as_str()
            .parse::<f64>()
            .map(MosExpr::Number)
            .map_err(|e| format!("invalid number '{}': {}", pair.as_str(), e)),
        Rule::string => {
            let raw = pair.as_str();
            let inner = raw.trim_matches('"').replace("\\\"", "\"");
            Ok(MosExpr::String(inner))
        }
        Rule::boolean => Ok(MosExpr::Bool(pair.as_str().eq_ignore_ascii_case("true"))),
        Rule::identifier | Rule::qualified_identifier => Ok(MosExpr::Ident(pair.as_str().to_string())),
        Rule::function_call => parse_function_call(pair),
        Rule::array_literal => {
            let mut items = Vec::new();
            for p in pair.into_inner() {
                if p.as_rule() == Rule::expr {
                    items.push(parse_expr(p)?);
                }
            }
            Ok(MosExpr::Array(items))
        }
        Rule::record_literal => {
            let mut fields = Vec::new();
            for p in pair.into_inner() {
                if p.as_rule() == Rule::record_field {
                    let mut it = p.into_inner();
                    let name = it
                        .next()
                        .ok_or_else(|| "record field missing name".to_string())?
                        .as_str()
                        .to_string();
                    let value = parse_expr(
                        it.next()
                            .ok_or_else(|| "record field missing value".to_string())?,
                    )?;
                    fields.push((name, value));
                }
            }
            Ok(MosExpr::Record(fields))
        }
        Rule::primary => {
            let mut it = pair.into_inner();
            let first = it.next().ok_or_else(|| "empty primary".to_string())?;
            parse_expr(first)
        }
        Rule::expr | Rule::comparison | Rule::additive | Rule::multiplicative | Rule::unary => {
            parse_expr(pair)
        }
        Rule::range_expr => {
            let mut it = pair.into_inner();
            let first = parse_expr(it.next().ok_or_else(|| "range missing start".to_string())?)?;
            let second = parse_expr(it.next().ok_or_else(|| "range missing bound".to_string())?)?;
            let third = it.next().map(parse_expr).transpose()?;
            if let Some(stop) = third {
                Ok(MosExpr::Range {
                    start: Box::new(first),
                    step: Some(Box::new(second)),
                    stop: Box::new(stop),
                })
            } else {
                Ok(MosExpr::Range {
                    start: Box::new(first),
                    step: None,
                    stop: Box::new(second),
                })
            }
        }
        _ => Err(format!("unsupported primary: {:?}", pair.as_rule())),
    }
}

fn parse_function_call(pair: Pair<Rule>) -> Result<MosExpr, String> {
    let raw_call = pair.as_str().to_string();
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .ok_or_else(|| "function call missing name".to_string())?
        .as_str()
        .to_string();
    let mut args = Vec::new();
    for next in inner {
        let mut flattened = Vec::new();
        if next.as_rule() == Rule::arg_list {
            flattened.extend(next.into_inner());
        } else {
            flattened.push(next);
        }
        for arg in flattened {
            match arg.as_rule() {
                Rule::named_argument => {
                    let mut it = arg.into_inner();
                    let key = it
                        .next()
                        .ok_or_else(|| "named argument missing key".to_string())?
                        .as_str()
                        .to_string();
                    let val = parse_expr(
                        it.next()
                            .ok_or_else(|| "named argument missing value".to_string())?,
                    )?;
                    args.push(MosArg {
                        name: Some(key),
                        value: val,
                    });
                }
                Rule::argument => {
                    let child = arg
                        .into_inner()
                        .next()
                        .ok_or_else(|| "empty argument".to_string())?;
                    match child.as_rule() {
                        Rule::named_argument => {
                            let mut it = child.into_inner();
                            let key = it
                                .next()
                                .ok_or_else(|| "named argument missing key".to_string())?
                                .as_str()
                                .to_string();
                            let val = parse_expr(
                                it.next()
                                    .ok_or_else(|| "named argument missing value".to_string())?,
                            )?;
                            args.push(MosArg {
                                name: Some(key),
                                value: val,
                            });
                        }
                        Rule::expr => args.push(MosArg {
                            name: None,
                            value: parse_expr(child)?,
                        }),
                        _ => {}
                    }
                }
                Rule::expr => args.push(MosArg {
                    name: None,
                    value: parse_expr(arg)?,
                }),
                _ => {}
            }
        }
    }
    if args.is_empty() {
        if let (Some(lp), Some(rp)) = (raw_call.find('('), raw_call.rfind(')')) {
            if rp > lp + 1 {
                let inside = raw_call[lp + 1..rp].trim();
                if !inside.is_empty() {
                    for raw_arg in inside.split(',') {
                        let a = raw_arg.trim();
                        if let Some(eq_idx) = a.find('=') {
                            let key = a[..eq_idx].trim().to_string();
                            let val = a[eq_idx + 1..].trim();
                            let value = if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                                MosExpr::String(val.trim_matches('"').to_string())
                            } else if let Ok(n) = val.parse::<f64>() {
                                MosExpr::Number(n)
                            } else if val.eq_ignore_ascii_case("true") || val.eq_ignore_ascii_case("false") {
                                MosExpr::Bool(val.eq_ignore_ascii_case("true"))
                            } else {
                                MosExpr::Ident(val.to_string())
                            };
                            args.push(MosArg { name: Some(key), value });
                        } else {
                            let value = if a.starts_with('"') && a.ends_with('"') && a.len() >= 2 {
                                MosExpr::String(a.trim_matches('"').to_string())
                            } else if let Ok(n) = a.parse::<f64>() {
                                MosExpr::Number(n)
                            } else if a.eq_ignore_ascii_case("true") || a.eq_ignore_ascii_case("false") {
                                MosExpr::Bool(a.eq_ignore_ascii_case("true"))
                            } else {
                                MosExpr::Ident(a.to_string())
                            };
                            args.push(MosArg { name: None, value });
                        }
                    }
                }
            }
        }
    }
    Ok(MosExpr::Call { name, args })
}

fn parse_expr(pair: Pair<Rule>) -> Result<MosExpr, String> {
    match pair.as_rule() {
        Rule::expr => {
            let mut it = pair.into_inner();
            let first = it.next().ok_or_else(|| "empty expr".to_string())?;
            parse_expr(first)
        }
        Rule::comparison | Rule::additive | Rule::multiplicative => {
            let mut it = pair.into_inner();
            let first = parse_expr(it.next().ok_or_else(|| "missing lhs".to_string())?)?;
            let mut acc = first;
            while let Some(op) = it.next() {
                let rhs = parse_expr(it.next().ok_or_else(|| "missing rhs".to_string())?)?;
                acc = MosExpr::Binary {
                    op: op.as_str().to_string(),
                    left: Box::new(acc),
                    right: Box::new(rhs),
                };
            }
            Ok(acc)
        }
        Rule::unary => {
            let mut ops = Vec::new();
            let mut prim: Option<Pair<Rule>> = None;
            for p in pair.into_inner() {
                if p.as_rule() == Rule::unary_op {
                    ops.push(p.as_str().to_string());
                } else {
                    prim = Some(p);
                }
            }
            let mut acc = parse_primary(prim.ok_or_else(|| "unary missing expr".to_string())?)?;
            for op in ops.into_iter().rev() {
                acc = MosExpr::Unary {
                    op,
                    expr: Box::new(acc),
                };
            }
            Ok(acc)
        }
        Rule::primary
        | Rule::number
        | Rule::string
        | Rule::boolean
        | Rule::identifier
        | Rule::qualified_identifier
        | Rule::function_call
        | Rule::array_literal
        | Rule::record_literal
        | Rule::range_expr => parse_primary(pair),
        _ => Err(format!("unsupported expression rule: {:?}", pair.as_rule())),
    }
}

fn parse_stmt(pair: Pair<Rule>) -> Result<Option<MosStmt>, String> {
    match pair.as_rule() {
        Rule::comment => Ok(None),
        Rule::assign_stmt => {
            let span = span_of(&pair);
            let mut it = pair.into_inner();
            let name = it
                .next()
                .ok_or_else(|| "assign missing name".to_string())?
                .as_str()
                .to_string();
            let value = parse_expr(it.next().ok_or_else(|| "assign missing value".to_string())?)?;
            Ok(Some(MosStmt::Assign { name, value, span }))
        }
        Rule::expr_stmt => {
            let span = span_of(&pair);
            let mut it = pair.into_inner();
            let expr = parse_expr(it.next().ok_or_else(|| "expr_stmt missing expr".to_string())?)?;
            Ok(Some(MosStmt::Expr { expr, span }))
        }
        Rule::if_stmt => {
            let span = span_of(&pair);
            let mut it = pair.into_inner();
            let cond = parse_expr(it.next().ok_or_else(|| "if missing condition".to_string())?)?;
            let mut then_body = Vec::new();
            let mut elseif = Vec::new();
            let mut else_body = Vec::new();
            let mut in_else = false;
            for p in it {
                match p.as_rule() {
                    Rule::elseif_branch => {
                        let mut eit = p.into_inner();
                        let c = parse_expr(
                            eit.next()
                                .ok_or_else(|| "elseif missing condition".to_string())?,
                        )?;
                        let mut body = Vec::new();
                        for sp in eit {
                            if let Some(s) = parse_stmt(sp)? {
                                body.push(s);
                            }
                        }
                        elseif.push((c, body));
                    }
                    Rule::else_branch => {
                        in_else = true;
                        for sp in p.into_inner() {
                            if let Some(s) = parse_stmt(sp)? {
                                else_body.push(s);
                            }
                        }
                    }
                    _ => {
                        if let Some(s) = parse_stmt(p)? {
                            if in_else {
                                else_body.push(s);
                            } else {
                                then_body.push(s);
                            }
                        }
                    }
                }
            }
            Ok(Some(MosStmt::If {
                cond,
                then_body,
                elseif,
                else_body,
                span,
            }))
        }
        Rule::for_stmt => {
            let span = span_of(&pair);
            let mut it = pair.into_inner();
            let var = it
                .next()
                .ok_or_else(|| "for missing variable".to_string())?
                .as_str()
                .to_string();
            let iter_pair = it.next().ok_or_else(|| "for missing iterator expr".to_string())?;
            let iter = match iter_pair.as_rule() {
                Rule::for_iter => {
                    let p = iter_pair
                        .into_inner()
                        .next()
                        .ok_or_else(|| "for_iter missing payload".to_string())?;
                    parse_expr(p)?
                }
                _ => parse_expr(iter_pair)?,
            };
            let mut body = Vec::new();
            for p in it {
                if let Some(s) = parse_stmt(p)? {
                    body.push(s);
                }
            }
            Ok(Some(MosStmt::For { var, iter, body, span }))
        }
        Rule::stmt => {
            let inner = pair.into_inner().next().ok_or_else(|| "empty stmt".to_string())?;
            parse_stmt(inner)
        }
        _ => Err(format!("unsupported statement: {:?}", pair.as_rule())),
    }
}

pub fn parse_mos_script(input: &str) -> Result<Vec<MosStmt>, String> {
    let mut pairs =
        MosParser::parse(Rule::script, input).map_err(|e| format!("mos parse error: {}", e))?;
    let script_pair = pairs.next().ok_or_else(|| "empty mos parse tree".to_string())?;
    let mut out = Vec::new();
    for p in script_pair.into_inner() {
        if p.as_rule() == Rule::EOI {
            continue;
        }
        if let Some(stmt) = parse_stmt(p)? {
            out.push(stmt);
        }
    }
    Ok(out)
}
