pub(crate) fn is_builtin_function(name: &str) -> bool {
    if let Some(head) = name.split('.').next() {
        if !head.is_empty() {
            let c = head.chars().next().unwrap_or('\0');
            if c.is_ascii_uppercase() {
                return true;
            }
        }
    }
    if !name.contains('.') {
        let c = name.chars().next().unwrap_or('\0');
        if c.is_ascii_uppercase() {
            return true;
        }
    }
    if name.ends_with(".sample")
        || name.ends_with(".interval")
        || name.ends_with(".backSample")
        || name.ends_with(".subSample")
        || name.ends_with(".superSample")
        || name.ends_with(".shiftSample")
        || name.ends_with(".Clock")
    {
        return true;
    }
    if name.starts_with("Modelica.Math.")
        || name.starts_with("Modelica.ComplexMath.")
        || name.starts_with("Modelica.Electrical.Polyphase.")
        || name.starts_with("Polyphase.")
        || name.starts_with("Internal.")
        || name.contains(".Internal.")
        || name.starts_with("Connections.")
        || name.starts_with("Frames.")
        || name.contains(".Frames.")
        || name.starts_with("BaseClasses.")
        || name.contains(".BaseClasses.")
        || name.starts_with("Medium.")
        || name.starts_with("Modelica.Utilities.")
    {
        return true;
    }
    matches!(
        name,
        "abs"
            | "sign"
            | "sqrt"
            | "min"
            | "max"
            | "mod"
            | "rem"
            | "div"
            | "integer"
            | "smooth"
            | "ceil"
            | "floor"
            | "sin"
            | "cos"
            | "tan"
            | "asin"
            | "acos"
            | "atan"
            | "atan2"
            | "sinh"
            | "cosh"
            | "tanh"
            | "exp"
            | "log"
            | "log10"
            | "pow"
            | "inStream"
            | "actualStream"
            | "positiveMax"
            | "pre"
            | "edge"
            | "change"
            | "noEvent"
            | "initial"
            | "firstTick"
            | "terminal"
            | "backSample"
            | "subSample"
            | "superSample"
            | "shiftSample"
            | "sample"
            | "interval"
            | "Clock"
            | "Integer"
            | "Real"
            | "Boolean"
            | "size"
            | "vector"
            | "zeros"
            | "ones"
            | "fill"
            | "cat"
            | "named"
            | "homotopy"
            | "cardinality"
            | "not"
            | "product"
            | "assert"
            | "terminate"
            | "sum"
            | "cross"
            | "Complex"
            | "conj"
            | "real"
            | "imag"
            | "transpose"
            | "delay"
            | "loadResource"
    )
}
