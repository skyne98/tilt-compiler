// ===================================================================
// FILE: lexer.rs
//
// DESC: Defines the lexer using the `logos` crate. It scans the
//       source string and produces a stream of tokens, skipping
//       whitespace and comments.
// ===================================================================

use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
#[logos(skip r"[ \t\n\r\f]+")] // Ignore this regex
#[logos(skip r"#.*")] // Ignore comments
pub enum Token<'a> {
    // Keywords
    #[token("fn")]
    Fn,
    #[token("import")]
    Import,
    #[token("ret")]
    Ret,
    #[token("br")]
    Br,
    #[token("br_if")]
    BrIf,
    #[token("phi")]
    Phi,
    #[token("call")]
    Call,

    // Types
    #[token("i32")]
    TI32,
    #[token("i64")]
    TI64,
    #[token("f32")]
    TF32,
    #[token("f64")]
    TF64,
    #[token("usize")]
    TUsize,
    #[token("void")]
    TVoid,

    // Punctuation
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(":")]
    Colon,
    #[token("=")]
    Equals,
    #[token(",")]
    Comma,
    #[token("->")]
    Arrow,

    // Literals and Identifiers
    #[regex(r#""([^"\\]|\\.)*""#, |lex| &lex.slice()[1..lex.slice().len()-1])]
    String(&'a str),

    #[regex("-?[0-9]+", |lex| lex.slice())]
    Number(&'a str),

    // An identifier or an operation code like `i32.add`
    #[regex("[a-zA-Z_.][a-zA-Z0-9_.]*", |lex| lex.slice())]
    Identifier(&'a str),
}
