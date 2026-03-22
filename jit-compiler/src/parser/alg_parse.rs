use crate::ast::{AlgorithmStatement, Declaration};
use crate::parser::{algorithm, decl_parse, Rule};

pub fn parse_algorithm_section(
    pair: pest::iterators::Pair<Rule>,
    algorithms: &mut Vec<AlgorithmStatement>,
    mut hoist_declarations: Option<&mut Vec<Declaration>>,
) {
    let alg_stmt_inner = pair.into_inner();
    for stmt in alg_stmt_inner {
        let inner_stmt = stmt.into_inner().next().unwrap();
        if inner_stmt.as_rule() == Rule::declaration {
            if let Some(decls) = hoist_declarations.as_mut() {
                decl_parse::parse_declaration_pair(inner_stmt, decls);
                continue;
            }
        }
        algorithms.push(algorithm::parse_algorithm_stmt(inner_stmt));
    }
}

pub fn parse_initial_algorithm_section(
    pair: pest::iterators::Pair<Rule>,
    initial_algorithms: &mut Vec<AlgorithmStatement>,
    mut hoist_declarations: Option<&mut Vec<Declaration>>,
) {
    let alg_stmt_inner = pair.into_inner();
    for stmt in alg_stmt_inner {
        let inner_stmt = stmt.into_inner().next().unwrap();
        if inner_stmt.as_rule() == Rule::declaration {
            if let Some(decls) = hoist_declarations.as_mut() {
                decl_parse::parse_declaration_pair(inner_stmt, decls);
                continue;
            }
        }
        initial_algorithms.push(algorithm::parse_algorithm_stmt(inner_stmt));
    }
}
