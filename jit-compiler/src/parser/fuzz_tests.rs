//! Fuzz / property-based tests for the Modelica parser.
//! Ensures the parser doesn't crash on random or edge-case inputs.

#[cfg(test)]
mod tests {
    use crate::parser::ModelicaParser;
    use pest::Parser;

    /// Parse a model string and ensure no panic.
    fn parse_or_none(input: &str) {
        let result = ModelicaParser::parse(crate::parser::Rule::model_file, input);
        // We don't care about success — just that it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_empty_input() {
        parse_or_none("");
    }

    #[test]
    fn test_only_whitespace() {
        parse_or_none("   \n  \t  \r\n  ");
    }

    #[test]
    fn test_minimal_model() {
        parse_or_none("model A end A;");
    }

    #[test]
    fn test_minimal_connector() {
        parse_or_none("connector C Real x; end C;");
    }

    #[test]
    fn test_minimal_function() {
        parse_or_none("function f input Real x; output Real y; algorithm y := x; end f;");
    }

    #[test]
    fn test_nested_models() {
        parse_or_none("model A model B Real x; end B; B b; end A;");
    }

    #[test]
    fn test_extends() {
        parse_or_none("model A extends B; end A;");
    }

    #[test]
    fn test_for_equation() {
        parse_or_none("model A Real x[3]; equation for i in 1:3 loop x[i] = i; end for; end A;");
    }

    #[test]
    fn test_when_equation() {
        parse_or_none("model A Real x; equation when time > 1 then x = 1; end when; end A;");
    }

    #[test]
    fn test_if_equation() {
        parse_or_none("model A Real x; equation if time > 1 then x = 1; else x = 0; end if; end A;");
    }

    #[test]
    fn test_connect_equation() {
        parse_or_none("connector C Real x; flow Real f; end C; model A C c1, c2; equation connect(c1, c2); end A;");
    }

    #[test]
    fn test_initial_equation() {
        parse_or_none("model A Real x; initial equation x = 0; equation der(x) = 1; end A;");
    }

    #[test]
    fn test_algorithm_section() {
        parse_or_none("model A Real x; algorithm x := 1.0; end A;");
    }

    #[test]
    fn test_array_literal() {
        parse_or_none("model A Real x[3] = {1, 2, 3}; end A;");
    }

    #[test]
    fn test_parameter_with_value() {
        parse_or_none("model A parameter Real p = 3.14; end A;");
    }

    #[test]
    fn test_comments() {
        parse_or_none("model A \"this is a model\" Real x \"a variable\"; end A;");
    }

    #[test]
    fn test_annotation() {
        parse_or_none("model A annotation(Icon(graphics={Rectangle(extent={{-100,100},{100,-100}})})); end A;");
    }

    #[test]
    fn test_partial_model() {
        parse_or_none("partial model A Real x; end A;");
    }

    #[test]
    fn test_expandable_connector() {
        parse_or_none("expandable connector C end C;");
    }

    #[test]
    fn test_encapsulated_model() {
        parse_or_none("encapsulated model A Real x; end A;");
    }

    #[test]
    fn test_pure_function() {
        parse_or_none("pure function f input Real x; output Real y; algorithm y := x; end f;");
    }

    #[test]
    fn test_redeclare() {
        parse_or_none("model A replaceable model B Real x; end B; B b; end A;");
    }

    #[test]
    fn test_inner_outer() {
        parse_or_none("model A outer Real x; end A;");
    }

    #[test]
    fn test_conditional_component() {
        parse_or_none("model A Real x if true; end A;");
    }

    #[test]
    fn test_enumeration() {
        parse_or_none("type E = enumeration(a, b, c);");
    }

    #[test]
    fn test_import_clause() {
        parse_or_none("model A import SI = Modelica.SIunits; Real x; end A;");
    }

    #[test]
    fn test_multiple_classes() {
        parse_or_none("model A Real x; end A; model B Real y; end B;");
    }

    #[test]
    fn test_deep_expressions() {
        parse_or_none("model A Real x; equation x = 1 + 2 * (3 - 4) / (5 + 6) ^ 7; end A;");
    }

    #[test]
    fn test_malformed_unclosed_model() {
        // Should not panic on unclosed model
        parse_or_none("model A Real x;");
    }

    #[test]
    fn test_malformed_random_garbage() {
        parse_or_none("!@#$%^&*()_+{}|:\"<>?~~~");
    }

    #[test]
    fn test_malformed_huge_identifier() {
        let big = "a".repeat(10000);
        let input = format!("model {} end {};", big, big);
        parse_or_none(&input);
    }
}
