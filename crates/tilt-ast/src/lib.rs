// ===================================================================
// FILE: lib.rs (tilt-ast crate)
//
// DESC: Defines the Abstract Syntax Tree (AST) for the TILT language.
//       These data structures are the output of the parser and represent
//       the program's semantic structure.
// ===================================================================

// We use lifetimes ('a) to borrow strings directly from the source code.
pub type Identifier<'a> = &'a str;

#[derive(Debug, PartialEq, Clone)]
pub struct Program<'a> {
    pub items: Vec<TopLevelItem<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum TopLevelItem<'a> {
    Import(ImportDecl<'a>),
    Function(FunctionDef<'a>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct ImportDecl<'a> {
    pub module: &'a str,
    pub name: &'a str,
    pub params: Vec<TypedIdentifier<'a>>,
    pub return_type: Type,
}

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionDef<'a> {
    pub name: Identifier<'a>,
    pub params: Vec<TypedIdentifier<'a>>,
    pub return_type: Type,
    pub blocks: Vec<Block<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Block<'a> {
    pub label: Identifier<'a>,
    pub instructions: Vec<Instruction<'a>>,
    pub terminator: Terminator<'a>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Instruction<'a> {
    // e.g., `res:i32 = i32.add v1, v2`
    Assign {
        dest: TypedIdentifier<'a>,
        expr: Expression<'a>,
    },
    // e.g., `i32.store addr, val`
    Store {
        op: &'a str, // e.g., "i32.store"
        address: Value<'a>,
        value: Value<'a>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expression<'a> {
    // e.g., `i32.add v1, v2` or `i32.const 123`
    Operation {
        op: &'a str,
        args: Vec<Value<'a>>,
    },
    // e.g., `call my_func(arg1)`
    Call {
        name: Identifier<'a>,
        args: Vec<Value<'a>>,
    },
    // e.g., `phi [entry: v_init], [loop: v_next]`
    Phi {
        nodes: Vec<(Identifier<'a>, Value<'a>)>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Terminator<'a> {
    // `ret` or `ret some_val`
    Ret(Option<Value<'a>>),
    // `br my_label`
    Br { label: Identifier<'a> },
    // `br_if cond, true_label, false_label`
    BrIf {
        cond: Value<'a>,
        true_label: Identifier<'a>,
        false_label: Identifier<'a>,
    },
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct TypedIdentifier<'a> {
    pub name: Identifier<'a>,
    pub ty: Type,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Type {
    I32,
    I64,
    F32,
    F64,
    Void,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Value<'a> {
    // A reference to another SSA value, e.g., `my_var`
    Variable(Identifier<'a>),
    // A literal constant, e.g., `123`
    Constant(i64),
}
