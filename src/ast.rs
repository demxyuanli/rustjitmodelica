/// Top-level class kind: model (or connector, block, etc.) or function.
/// T1-3: Dedicated Function variant for "function ... end function"; parser produces ClassItem::Function, loader converts to Model for pipeline.
#[derive(Debug, Clone)]
pub enum ClassItem {
    Model(Model),
    Function(Function),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Function {
    pub name: String,
    pub extends: Vec<ExtendsClause>,
    pub declarations: Vec<Declaration>,
    pub algorithms: Vec<AlgorithmStatement>,
    pub initial_algorithms: Vec<AlgorithmStatement>,
    /// F3-4: external "C" [name(args)]; parse-only; linking documented in ABI.
    pub external_info: Option<ExternalDecl>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExternalDecl {
    pub language: Option<String>,
    pub c_name: Option<String>,
}

impl From<Function> for Model {
    fn from(f: Function) -> Model {
        Model {
            name: f.name,
            is_connector: false,
            is_function: true,
            is_record: false,
            is_block: false,
            extends: f.extends,
            declarations: f.declarations,
            equations: vec![],
            algorithms: f.algorithms,
            initial_equations: vec![],
            initial_algorithms: f.initial_algorithms,
            annotation: None,
            inner_classes: vec![],
            is_operator_record: false,
            type_aliases: vec![],
            external_info: f.external_info,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Model {
    pub name: String,
    pub is_connector: bool,
    pub is_function: bool,
    pub is_record: bool,
    pub is_block: bool,
    pub extends: Vec<ExtendsClause>,
    pub declarations: Vec<Declaration>,
    pub equations: Vec<Equation>,
    pub algorithms: Vec<AlgorithmStatement>,
    pub initial_equations: Vec<Equation>,
    pub initial_algorithms: Vec<AlgorithmStatement>,
    /// Parsed annotation (e.g. annotation(...)); stored as raw string, ignored in backend (F1-5).
    pub annotation: Option<String>,
    /// F1-3: nested classes inside package/model (e.g. package P model A ... end A; end P).
    pub inner_classes: Vec<Model>,
    /// F1-4: operator record (parse-only; MSL compatibility).
    pub is_operator_record: bool,
    /// F1-4: type alias (e.g. type MyReal = Real;) parse-only; name -> base_type.
    pub type_aliases: Vec<(String, String)>,
    /// F3-4: when is_function, external decl if present.
    pub external_info: Option<ExternalDecl>,
}

#[derive(Debug, Clone)]
pub enum AlgorithmStatement {
    Assignment(Expression, Expression), // lhs := rhs
    If(Expression, Vec<AlgorithmStatement>, Vec<(Expression, Vec<AlgorithmStatement>)>, Option<Vec<AlgorithmStatement>>), // if cond then stmts elseif cond stmts else stmts
    For(String, Box<Expression>, Vec<AlgorithmStatement>), // for i in range loop stmts
    While(Expression, Vec<AlgorithmStatement>), // while cond loop stmts
    When(Expression, Vec<AlgorithmStatement>, Vec<(Expression, Vec<AlgorithmStatement>)>), // when cond then stmts elsewhen cond stmts
    Reinit(String, Expression), // reinit(var, expr)
    Assert(Expression, Expression),   // assert(condition, message)
    Terminate(Expression),            // terminate(message)
}

#[derive(Debug, Clone)]
pub struct ExtendsClause {
    pub model_name: String,
    pub modifications: Vec<Modification>,
}

#[derive(Debug, Clone)]
pub struct Modification {
    pub name: String,
    pub value: Option<Expression>,
    /// F1-6: each modifier in extends; when true, apply to all array elements.
    pub each: bool,
    /// F1-6: redeclare modifier; replace component type in extends.
    pub redeclare: bool,
    /// When redeclare is true, new type for the component (e.g. "Real"); applied in extends/expand.
    pub redeclare_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Declaration {
    #[allow(dead_code)]
    pub type_name: String,
    pub name: String,
    /// MSL-5: replaceable component (parse-only; allows redeclare in modifier).
    pub replaceable: bool,
    pub is_parameter: bool,
    pub is_flow: bool,
    pub is_discrete: bool,
    pub is_input: bool,
    pub is_output: bool,
    pub start_value: Option<Expression>,
    pub array_size: Option<Expression>,
    pub modifications: Vec<Modification>,
    /// Parsed annotation; ignored in backend (F1-5).
    #[allow(dead_code)]
    pub annotation: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Equation {
    Simple(Expression, Expression), // lhs = rhs
    /// F3-3: (lhs1, lhs2, ...) = rhs; multi-output function call
    MultiAssign(Vec<Expression>, Expression),
    For(String, Box<Expression>, Box<Expression>, Vec<Equation>),
    Connect(Expression, Expression),
    When(Expression, Vec<Equation>, Vec<(Expression, Vec<Equation>)>),
    If(Expression, Vec<Equation>, Vec<(Expression, Vec<Equation>)>, Option<Vec<Equation>>), // cond, then, elseif list, else
    Reinit(String, Expression),
    Assert(Expression, Expression),   // assert(condition, message)
    Terminate(Expression),            // terminate(message)
    SolvableBlock {
        unknowns: Vec<String>,
        tearing_var: Option<String>,
        equations: Vec<Equation>,
        residuals: Vec<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Variable(String),
    Number(f64),
    BinaryOp(Box<Expression>, Operator, Box<Expression>),
    Call(String, Vec<Expression>),
    Der(Box<Expression>), 
    ArrayAccess(Box<Expression>, Box<Expression>), // expr[i]
    Dot(Box<Expression>, String), // expr.name
    If(Box<Expression>, Box<Expression>, Box<Expression>), // if cond then true_expr else false_expr
    Range(Box<Expression>, Box<Expression>, Box<Expression>), // start:step:end
    ArrayLiteral(Vec<Expression>), // {e1, e2, ...}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    Less,
    Greater,
    LessEq,
    GreaterEq,
    Equal,
    NotEqual,
    And,
    Or,
}
