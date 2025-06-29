// ===================================================================
// FILE: tests.rs (tilt-parser crate)
//
// DESC: Comprehensive unit tests for the TILT parser, covering all
//       edge cases and grammar constructs. Tests lexing, parsing,
//       and AST generation for various TILT language features.
// ===================================================================

#[cfg(test)]
mod tests {
    use crate::lexer::Token;
    use crate::tilt;
    use logos::Logos;
    use tilt_ast::*;

    // Helper function to tokenize input
    fn tokenize(input: &str) -> Vec<Token> {
        Token::lexer(input).collect::<Result<Vec<_>, _>>().unwrap()
    }

    // Helper function to tokenize input and create triples
    fn tokenize_with_positions(input: &str) -> Vec<(usize, Token, usize)> {
        let mut lexer = Token::lexer(input);
        let mut tokens = Vec::new();

        while let Some(token) = lexer.next() {
            let token = token.unwrap();
            let span = lexer.span();
            tokens.push((span.start, token, span.end));
        }
        tokens
    }

    // Helper function to parse with error handling
    fn parse_program(input: &str) -> Result<Program, String> {
        let tokens = tokenize_with_positions(input);
        let parser = tilt::ProgramParser::new();
        parser
            .parse(tokens.into_iter())
            .map_err(|e| format!("{:?}", e))
    }

    // Helper function to parse expressions
    fn parse_expression(input: &str) -> Result<Expression, String> {
        let tokens = tokenize_with_positions(input);
        let parser = tilt::ExpressionParser::new();
        parser
            .parse(tokens.into_iter())
            .map_err(|e| format!("{:?}", e))
    }

    // Helper function to parse instructions
    fn parse_instruction(input: &str) -> Result<Instruction, String> {
        let tokens = tokenize_with_positions(input);
        let parser = tilt::InstructionParser::new();
        parser
            .parse(tokens.into_iter())
            .map_err(|e| format!("{:?}", e))
    }

    // Helper function to parse terminators
    fn parse_terminator(input: &str) -> Result<Terminator, String> {
        let tokens = tokenize_with_positions(input);
        let parser = tilt::TerminatorParser::new();
        parser
            .parse(tokens.into_iter())
            .map_err(|e| format!("{:?}", e))
    }

    // Helper function to parse blocks
    fn parse_block(input: &str) -> Result<Block, String> {
        let tokens = tokenize_with_positions(input);
        let parser = tilt::BlockParser::new();
        parser
            .parse(tokens.into_iter())
            .map_err(|e| format!("{:?}", e))
    }

    // ===============================
    // LEXER TESTS
    // ===============================

    #[test]
    fn test_lexer_keywords() {
        let input = "fn import ret br br_if phi call";
        let tokens = tokenize(input);
        assert_eq!(
            tokens,
            vec![
                Token::Fn,
                Token::Import,
                Token::Ret,
                Token::Br,
                Token::BrIf,
                Token::Phi,
                Token::Call,
            ]
        );
    }

    #[test]
    fn test_lexer_types() {
        let input = "i32 i64 f32 f64 void";
        let tokens = tokenize(input);
        assert_eq!(
            tokens,
            vec![
                Token::TI32,
                Token::TI64,
                Token::TF32,
                Token::TF64,
                Token::TVoid,
            ]
        );
    }

    #[test]
    fn test_lexer_punctuation() {
        let input = "{ } ( ) [ ] : = , ->";
        let tokens = tokenize(input);
        assert_eq!(
            tokens,
            vec![
                Token::LBrace,
                Token::RBrace,
                Token::LParen,
                Token::RParen,
                Token::LBracket,
                Token::RBracket,
                Token::Colon,
                Token::Equals,
                Token::Comma,
                Token::Arrow,
            ]
        );
    }

    #[test]
    fn test_lexer_identifiers() {
        let input = "my_var another_func test123 _private";
        let tokens = tokenize(input);
        if let Token::Identifier(name) = &tokens[0] {
            assert_eq!(*name, "my_var");
        }
        if let Token::Identifier(name) = &tokens[1] {
            assert_eq!(*name, "another_func");
        }
        if let Token::Identifier(name) = &tokens[2] {
            assert_eq!(*name, "test123");
        }
        if let Token::Identifier(name) = &tokens[3] {
            assert_eq!(*name, "_private");
        }
    }

    #[test]
    fn test_lexer_numbers() {
        let input = "123 0 -456 999";
        let tokens = tokenize(input);
        if let Token::Number(num) = &tokens[0] {
            assert_eq!(*num, "123");
        }
        if let Token::Number(num) = &tokens[1] {
            assert_eq!(*num, "0");
        }
        if let Token::Number(num) = &tokens[2] {
            assert_eq!(*num, "-456");
        }
        if let Token::Number(num) = &tokens[3] {
            assert_eq!(*num, "999");
        }
    }

    #[test]
    fn test_lexer_strings() {
        let input = r#""hello" "world with spaces""#;
        let tokens = tokenize(input);
        if let Token::String(s) = &tokens[0] {
            assert_eq!(*s, "hello");
        }
        if let Token::String(s) = &tokens[1] {
            assert_eq!(*s, "world with spaces");
        }
    }

    #[test]
    fn test_lexer_comments() {
        let input = "fn # this is a comment\nmy_func";
        let tokens = tokenize(input);
        assert_eq!(tokens, vec![Token::Fn, Token::Identifier("my_func"),]);
    }

    #[test]
    fn test_lexer_whitespace() {
        let input = "fn\n\t  my_func\r\n";
        let tokens = tokenize(input);
        assert_eq!(tokens, vec![Token::Fn, Token::Identifier("my_func"),]);
    }

    // ===============================
    // TYPE PARSING TESTS
    // ===============================

    #[test]
    fn test_parse_types() {
        let parser = tilt::TypeParser::new();

        assert_eq!(
            parser.parse(vec![(0, Token::TI32, 3)].into_iter()).unwrap(),
            Type::I32
        );
        assert_eq!(
            parser.parse(vec![(0, Token::TI64, 3)].into_iter()).unwrap(),
            Type::I64
        );
        assert_eq!(
            parser.parse(vec![(0, Token::TF32, 3)].into_iter()).unwrap(),
            Type::F32
        );
        assert_eq!(
            parser.parse(vec![(0, Token::TF64, 3)].into_iter()).unwrap(),
            Type::F64
        );
        assert_eq!(
            parser
                .parse(vec![(0, Token::TVoid, 4)].into_iter())
                .unwrap(),
            Type::Void
        );
    }

    // ===============================
    // VALUE PARSING TESTS
    // ===============================

    #[test]
    fn test_parse_value_constant() {
        let parser = tilt::ValueParser::new();
        let tokens = vec![(0, Token::Number("42"), 2)];
        let result = parser.parse(tokens.into_iter()).unwrap();
        assert_eq!(result, Value::Constant(42));
    }

    #[test]
    fn test_parse_value_variable() {
        let parser = tilt::ValueParser::new();
        let tokens = vec![(0, Token::Identifier("my_var"), 6)];
        let result = parser.parse(tokens.into_iter()).unwrap();
        assert_eq!(result, Value::Variable("my_var"));
    }

    #[test]
    fn test_parse_value_negative_constant() {
        let parser = tilt::ValueParser::new();
        let tokens = vec![(0, Token::Number("-123"), 4)];
        let result = parser.parse(tokens.into_iter()).unwrap();
        assert_eq!(result, Value::Constant(-123));
    }

    // ===============================
    // EXPRESSION PARSING TESTS
    // ===============================

    #[test]
    fn test_parse_call_no_args() {
        let result = parse_expression("call my_func()").unwrap();
        assert_eq!(
            result,
            Expression::Call {
                name: "my_func",
                args: vec![]
            }
        );
    }

    #[test]
    fn test_parse_call_one_arg() {
        let result = parse_expression("call my_func(42)").unwrap();
        assert_eq!(
            result,
            Expression::Call {
                name: "my_func",
                args: vec![Value::Constant(42)]
            }
        );
    }

    #[test]
    fn test_parse_call_two_args() {
        let result = parse_expression("call my_func(x, 42)").unwrap();
        assert_eq!(
            result,
            Expression::Call {
                name: "my_func",
                args: vec![Value::Variable("x"), Value::Constant(42)]
            }
        );
    }

    // ===============================
    // INSTRUCTION PARSING TESTS
    // ===============================

    #[test]
    fn test_parse_instruction_assignment() {
        let result = parse_instruction("result:i32 = call my_func()").unwrap();
        assert_eq!(
            result,
            Instruction::Assign {
                dest: TypedIdentifier {
                    name: "result",
                    ty: Type::I32
                },
                expr: Expression::Call {
                    name: "my_func",
                    args: vec![]
                }
            }
        );
    }

    #[test]
    fn test_parse_instruction_assignment_with_args() {
        let result = parse_instruction("sum:i64 = call add_func(a, 10)").unwrap();
        assert_eq!(
            result,
            Instruction::Assign {
                dest: TypedIdentifier {
                    name: "sum",
                    ty: Type::I64
                },
                expr: Expression::Call {
                    name: "add_func",
                    args: vec![Value::Variable("a"), Value::Constant(10)]
                }
            }
        );
    }

    // ===============================
    // TERMINATOR PARSING TESTS
    // ===============================

    #[test]
    fn test_parse_terminator_ret() {
        let result = parse_terminator("ret").unwrap();
        assert_eq!(result, Terminator::Ret(None));
    }

    #[test]
    fn test_parse_terminator_br() {
        let result = parse_terminator("br exit").unwrap();
        assert_eq!(
            result,
            Terminator::Br {
                label: "exit",
                args: vec![]
            }
        );
    }

    #[test]
    fn test_parse_terminator_br_if() {
        let result = parse_terminator("br_if condition, true_block, false_block").unwrap();
        assert_eq!(
            result,
            Terminator::BrIf {
                cond: Value::Variable("condition"),
                true_label: "true_block",
                false_label: "false_block",
                true_args: vec![],
                false_args: vec![],
            }
        );
    }

    #[test]
    fn test_parse_terminator_br_if_with_constant() {
        let result = parse_terminator("br_if 1, loop_body, exit").unwrap();
        assert_eq!(
            result,
            Terminator::BrIf {
                cond: Value::Constant(1),
                true_label: "loop_body",
                false_label: "exit",
                true_args: vec![],
                false_args: vec![],
            }
        );
    }

    // ===============================
    // BLOCK PARSING TESTS
    // ===============================

    #[test]
    fn test_parse_block_empty() {
        let result = parse_block("entry: ret").unwrap();
        assert_eq!(
            result,
            Block {
                label: "entry",
                params: vec![],
                instructions: vec![],
                terminator: Terminator::Ret(None)
            }
        );
    }

    #[test]
    fn test_parse_block_with_instructions() {
        let input = r#"
        loop_body:
            result:i32 = call compute()
            counter:i32 = call increment(result)
            br_if counter, loop_body, exit
        "#;
        let result = parse_block(input).unwrap();

        assert_eq!(result.label, "loop_body");
        assert_eq!(result.instructions.len(), 2);

        // Check first instruction
        if let Instruction::Assign { dest, expr } = &result.instructions[0] {
            assert_eq!(dest.name, "result");
            assert_eq!(dest.ty, Type::I32);
            if let Expression::Call { name, args } = expr {
                assert_eq!(*name, "compute");
                assert_eq!(args.len(), 0);
            }
        }

        // Check terminator
        if let Terminator::BrIf {
            cond,
            true_label,
            false_label,
            ..
        } = &result.terminator
        {
            assert_eq!(*cond, Value::Variable("counter"));
            assert_eq!(*true_label, "loop_body");
            assert_eq!(*false_label, "exit");
        }
    }

    // ===============================
    // IMPORT DECLARATION TESTS
    // ===============================

    #[test]
    fn test_parse_import_decl() {
        let parser = tilt::ImportDeclParser::new();
        let tokens = tokenize_with_positions(r#"import "stdlib" "print" -> void"#);
        let result = parser.parse(tokens.into_iter()).unwrap();

        assert_eq!(
            result,
            ImportDecl {
                module: "stdlib",
                name: "print",
                calling_convention: None,
                params: vec![],
                return_type: Type::Void
            }
        );
    }

    #[test]
    fn test_parse_import_decl_with_return_type() {
        let parser = tilt::ImportDeclParser::new();
        let tokens = tokenize_with_positions(r#"import "math" "sqrt" -> f64"#);
        let result = parser.parse(tokens.into_iter()).unwrap();

        assert_eq!(
            result,
            ImportDecl {
                module: "math",
                name: "sqrt",
                calling_convention: None,
                params: vec![],
                return_type: Type::F64
            }
        );
    }

    // ===============================
    // FUNCTION DEFINITION TESTS
    // ===============================

    #[test]
    fn test_parse_function_empty() {
        let parser = tilt::FunctionDefParser::new();
        let tokens = tokenize_with_positions("fn main() -> void { entry: ret }");
        let result = parser.parse(tokens.into_iter()).unwrap();

        assert_eq!(
            result,
            FunctionDef {
                name: "main",
                params: vec![],
                return_type: Type::Void,
                blocks: vec![Block {
                    label: "entry",
                    params: vec![],
                    instructions: vec![],
                    terminator: Terminator::Ret(None)
                }]
            }
        );
    }

    #[test]
    fn test_parse_function_with_multiple_blocks() {
        let input = r#"
        fn test_func() -> i32 {
            entry:
                x:i32 = call get_value()
                br_if x, positive, negative
            
            positive:
                ret
            
            negative:
                ret
        }
        "#;

        let parser = tilt::FunctionDefParser::new();
        let tokens = tokenize_with_positions(input);
        let result = parser.parse(tokens.into_iter()).unwrap();

        assert_eq!(result.name, "test_func");
        assert_eq!(result.return_type, Type::I32);
        assert_eq!(result.blocks.len(), 3);

        // Check entry block
        assert_eq!(result.blocks[0].label, "entry");
        assert_eq!(result.blocks[0].instructions.len(), 1);

        // Check positive block
        assert_eq!(result.blocks[1].label, "positive");

        // Check negative block
        assert_eq!(result.blocks[2].label, "negative");
    }

    // ===============================
    // PROGRAM PARSING TESTS
    // ===============================

    #[test]
    fn test_parse_empty_program() {
        let result = parse_program("").unwrap();
        assert_eq!(result, Program { items: vec![] });
    }

    #[test]
    fn test_parse_program_with_import() {
        let input = r#"import "stdlib" "print" -> void"#;
        let result = parse_program(input).unwrap();

        assert_eq!(result.items.len(), 1);
        if let TopLevelItem::Import(import) = &result.items[0] {
            assert_eq!(import.module, "stdlib");
            assert_eq!(import.name, "print");
            assert_eq!(import.return_type, Type::Void);
        }
    }

    #[test]
    fn test_parse_program_with_function() {
        let input = r#"
        fn main() -> void {
            entry: ret
        }
        "#;
        let result = parse_program(input).unwrap();

        assert_eq!(result.items.len(), 1);
        if let TopLevelItem::Function(func) = &result.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.return_type, Type::Void);
            assert_eq!(func.blocks.len(), 1);
        }
    }

    #[test]
    fn test_parse_program_complete() {
        let input = r#"
        import "stdlib" "print" -> void
        import "math" "sqrt" -> f64
        
        fn calculate() -> f64 {
            entry:
                x:f64 = call sqrt(16)
                y:f64 = call sqrt(x)
                ret
        }
        
        fn main() -> void {
            entry:
                result:f64 = call calculate()
                call print()
                ret
        }
        "#;

        let result = parse_program(input).unwrap();

        assert_eq!(result.items.len(), 4);

        // Check imports
        if let TopLevelItem::Import(import1) = &result.items[0] {
            assert_eq!(import1.module, "stdlib");
            assert_eq!(import1.name, "print");
        }

        if let TopLevelItem::Import(import2) = &result.items[1] {
            assert_eq!(import2.module, "math");
            assert_eq!(import2.name, "sqrt");
        }

        // Check functions
        if let TopLevelItem::Function(func1) = &result.items[2] {
            assert_eq!(func1.name, "calculate");
            assert_eq!(func1.return_type, Type::F64);
        }

        if let TopLevelItem::Function(func2) = &result.items[3] {
            assert_eq!(func2.name, "main");
            assert_eq!(func2.return_type, Type::Void);
        }
    }

    // ===============================
    // ERROR HANDLING TESTS
    // ===============================

    #[test]
    fn test_parse_error_invalid_syntax() {
        let result = parse_program("fn invalid syntax");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error_missing_brace() {
        let result = parse_program("fn test() -> void { entry: ret");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error_invalid_type() {
        let result = parse_program("fn test() -> invalid_type { entry: ret }");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error_missing_terminator() {
        let result = parse_program("fn test() -> void { entry: x:i32 = call func() }");
        assert!(result.is_err());
    }

    // ===============================
    // EDGE CASE TESTS
    // ===============================

    #[test]
    fn test_parse_single_character_identifiers() {
        let result = parse_program("fn a() -> void { b: ret }").unwrap();
        if let TopLevelItem::Function(func) = &result.items[0] {
            assert_eq!(func.name, "a");
            assert_eq!(func.blocks[0].label, "b");
        }
    }

    #[test]
    fn test_parse_underscore_identifiers() {
        let result = parse_program("fn _test() -> void { _entry: ret }").unwrap();
        if let TopLevelItem::Function(func) = &result.items[0] {
            assert_eq!(func.name, "_test");
            assert_eq!(func.blocks[0].label, "_entry");
        }
    }

    #[test]
    fn test_parse_numeric_identifiers() {
        let result = parse_program("fn test123() -> void { block42: ret }").unwrap();
        if let TopLevelItem::Function(func) = &result.items[0] {
            assert_eq!(func.name, "test123");
            assert_eq!(func.blocks[0].label, "block42");
        }
    }

    #[test]
    fn test_parse_zero_constant() {
        let result = parse_expression("call func(0)").unwrap();
        if let Expression::Call { args, .. } = result {
            assert_eq!(args[0], Value::Constant(0));
        }
    }

    #[test]
    fn test_parse_large_constant() {
        let result = parse_expression("call func(999999)").unwrap();
        if let Expression::Call { args, .. } = result {
            assert_eq!(args[0], Value::Constant(999999));
        }
    }

    #[test]
    fn test_parse_mixed_case_sensitivity() {
        // Keywords should be case-sensitive
        let result = parse_program("Fn test() -> Void { Entry: Ret }");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_whitespace_variations() {
        let input1 = "fn test()->void{entry:ret}";
        let input2 = "fn   test( )  ->  void  {  entry :  ret  }";
        let input3 = "fn\ttest()\t->\tvoid\t{\tentry:\tret\t}";

        let result1 = parse_program(input1).unwrap();
        let result2 = parse_program(input2).unwrap();
        let result3 = parse_program(input3).unwrap();

        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
    }

    #[test]
    fn test_parse_comment_variations() {
        let input = r#"
        # Comment at start
        fn test() -> void { # Comment after brace
            entry: # Comment after label
                # Comment on its own line
                ret # Comment after terminator
        } # Comment at end
        "#;

        let result = parse_program(input).unwrap();
        if let TopLevelItem::Function(func) = &result.items[0] {
            assert_eq!(func.name, "test");
        }
    }

    #[test]
    fn test_parse_nested_function_calls() {
        // This would require extending the grammar to support nested expressions
        // For now, test that our current grammar handles single-level calls
        let result = parse_instruction("result:i32 = call outer(inner_result)").unwrap();
        if let Instruction::Assign { expr, .. } = result {
            if let Expression::Call { name, args } = expr {
                assert_eq!(name, "outer");
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], Value::Variable("inner_result"));
            }
        }
    }

    // ===============================
    // REGRESSION TESTS
    // ===============================

    #[test]
    fn test_parse_regression_empty_blocks() {
        // Ensure empty blocks (no instructions, only terminator) work
        let input = r#"
        fn test() -> void {
            entry: ret
            unreachable: ret
        }
        "#;

        let result = parse_program(input).unwrap();
        if let TopLevelItem::Function(func) = &result.items[0] {
            assert_eq!(func.blocks.len(), 2);
            assert_eq!(func.blocks[0].instructions.len(), 0);
            assert_eq!(func.blocks[1].instructions.len(), 0);
        }
    }

    #[test]
    fn test_parse_regression_all_types() {
        // Test all supported types in various contexts
        let input = r#"
        import "test" "func_i32" -> i32
        import "test" "func_i64" -> i64
        import "test" "func_f32" -> f32
        import "test" "func_f64" -> f64
        import "test" "func_void" -> void
        
        fn test() -> void {
            entry:
                a:i32 = call func_i32()
                b:i64 = call func_i64()
                c:f32 = call func_f32()
                d:f64 = call func_f64()
                call func_void()
                ret
        }
        "#;

        let result = parse_program(input).unwrap();
        assert_eq!(result.items.len(), 6); // 5 imports + 1 function
    }

    #[test]
    fn test_parse_regression_branch_variations() {
        // Test all terminator variations
        let input = r#"
        fn test() -> void {
            entry: br next
            next: br_if condition, loop_back, exit
            loop_back: ret
            exit: ret
        }
        "#;

        let result = parse_program(input).unwrap();
        if let TopLevelItem::Function(func) = &result.items[0] {
            assert_eq!(func.blocks.len(), 4);
        }
    }
}
