#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Model {
    pub name: String,
    pub is_connector: bool,
    pub extends: Vec<ExtendsClause>, 
    pub declarations: Vec<Declaration>,
    pub equations: Vec<Equation>,
    pub algorithms: Vec<AlgorithmStatement>, // New
}

#[derive(Debug, Clone)]
pub enum AlgorithmStatement {
    Assignment(Expression, Expression), // lhs := rhs
    If(Expression, Vec<AlgorithmStatement>, Vec<(Expression, Vec<AlgorithmStatement>)>, Option<Vec<AlgorithmStatement>>), // if cond then stmts elseif cond stmts else stmts
    For(String, Box<Expression>, Vec<AlgorithmStatement>), // for i in range loop stmts
    While(Expression, Vec<AlgorithmStatement>), // while cond loop stmts
    When(Expression, Vec<AlgorithmStatement>, Vec<(Expression, Vec<AlgorithmStatement>)>), // when cond then stmts elsewhen cond stmts
    Reinit(String, Expression), // reinit(var, expr)
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
}

#[derive(Debug, Clone)]
pub struct Declaration {
    #[allow(dead_code)]
    pub type_name: String,
    pub name: String,
    pub is_parameter: bool,
    pub is_flow: bool, // New
    pub is_discrete: bool, // New
    pub start_value: Option<Expression>,
    pub array_size: Option<Expression>, 
    pub modifications: Vec<Modification>, // New
}

#[derive(Debug, Clone)]
pub enum Equation {
    Simple(Expression, Expression), // lhs = rhs
    For(String, Box<Expression>, Box<Expression>, Vec<Equation>), // for i in start:end loop ... end for;
    Connect(Expression, Expression), // connect(a, b)
    When(Expression, Vec<Equation>, Vec<(Expression, Vec<Equation>)>), // when cond then eqs elsewhen cond eqs
    Reinit(String, Expression), // reinit(var, expr)
    // A block of simultaneous equations that must be solved together
    // tearing_var: The variable to iterate on (Newton-Raphson)
    // equations: The sorted equations inside the loop (calculating other vars from tearing var)
    // residuals: The equations that must effectively be zero (residuals)
    SolvableBlock {
        unknowns: Vec<String>,
        tearing_var: Option<String>,
        equations: Vec<Equation>,
        residuals: Vec<Expression> 
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
