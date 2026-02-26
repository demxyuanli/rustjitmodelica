use cranelift_jit::JITBuilder;

// Math Wrappers
extern "C" fn modelica_mod(x: f64, y: f64) -> f64 {
    x.rem_euclid(y)
}
extern "C" fn modelica_rem(x: f64, y: f64) -> f64 {
    x % y
}
extern "C" fn modelica_sign(x: f64) -> f64 {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}
extern "C" fn modelica_min(x: f64, y: f64) -> f64 {
    x.min(y)
}
extern "C" fn modelica_max(x: f64, y: f64) -> f64 {
    x.max(y)
}

pub fn register_symbols(builder: &mut JITBuilder) {
    // Register symbols for math functions
    builder.symbol("sin", f64::sin as *const u8);
    builder.symbol("cos", f64::cos as *const u8);
    builder.symbol("tan", f64::tan as *const u8);
    builder.symbol("asin", f64::asin as *const u8);
    builder.symbol("acos", f64::acos as *const u8);
    builder.symbol("atan", f64::atan as *const u8);
    builder.symbol("atan2", f64::atan2 as *const u8);
    builder.symbol("sinh", f64::sinh as *const u8);
    builder.symbol("cosh", f64::cosh as *const u8);
    builder.symbol("tanh", f64::tanh as *const u8);
    builder.symbol("sqrt", f64::sqrt as *const u8);
    builder.symbol("exp", f64::exp as *const u8);
    builder.symbol("log", f64::ln as *const u8);
    builder.symbol("log10", f64::log10 as *const u8);
    builder.symbol("abs", f64::abs as *const u8);
    builder.symbol("ceil", f64::ceil as *const u8);
    builder.symbol("floor", f64::floor as *const u8);
    
    // Extended Math
    builder.symbol("mod", modelica_mod as *const u8);
    builder.symbol("rem", modelica_rem as *const u8);
    builder.symbol("sign", modelica_sign as *const u8);
    builder.symbol("min", modelica_min as *const u8);
    builder.symbol("max", modelica_max as *const u8);

    // Modelica.Math Aliases
    builder.symbol("Modelica.Math.sin", f64::sin as *const u8);
    builder.symbol("Modelica.Math.cos", f64::cos as *const u8);
    builder.symbol("Modelica.Math.tan", f64::tan as *const u8);
    builder.symbol("Modelica.Math.asin", f64::asin as *const u8);
    builder.symbol("Modelica.Math.acos", f64::acos as *const u8);
    builder.symbol("Modelica.Math.atan", f64::atan as *const u8);
    builder.symbol("Modelica.Math.atan2", f64::atan2 as *const u8);
    builder.symbol("Modelica.Math.sinh", f64::sinh as *const u8);
    builder.symbol("Modelica.Math.cosh", f64::cosh as *const u8);
    builder.symbol("Modelica.Math.tanh", f64::tanh as *const u8);
    builder.symbol("Modelica.Math.exp", f64::exp as *const u8);
    builder.symbol("Modelica.Math.log", f64::ln as *const u8);
    builder.symbol("Modelica.Math.log10", f64::log10 as *const u8);
    builder.symbol("Modelica.Math.sqrt", f64::sqrt as *const u8);
    builder.symbol("Modelica.Math.ceil", f64::ceil as *const u8);
    builder.symbol("Modelica.Math.floor", f64::floor as *const u8);
    builder.symbol("Modelica.Math.mod", modelica_mod as *const u8);
    builder.symbol("Modelica.Math.rem", modelica_rem as *const u8);
    builder.symbol("Modelica.Math.sign", modelica_sign as *const u8);
    builder.symbol("Modelica.Math.min", modelica_min as *const u8);
    builder.symbol("Modelica.Math.max", modelica_max as *const u8);
}
