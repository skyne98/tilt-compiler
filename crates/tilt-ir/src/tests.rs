// ===================================================================
// FILE: tests.rs (tilt-ir crate)
//
// DESC: Tests for the IR lowering module, covering semantic analysis
//       and IR generation for various TILT language constructs.
// ===================================================================

#[cfg(test)]
mod tests {
    use crate::{BlockId, SemanticError, ValueId, lowering::lower_program};
    use tilt_ast::*;

    fn create_test_program(items: Vec<TopLevelItem>) -> Program {
        Program { items }
    }

    #[test]
    fn test_lower_empty_program() {
        let ast = create_test_program(vec![]);
        let result = lower_program(&ast).unwrap();
        assert_eq!(result.imports.len(), 0);
        assert_eq!(result.functions.len(), 0);
    }

    #[test]
    fn test_lower_simple_import() {
        let import = ImportDecl {
            module: "env",
            name: "print",
            calling_convention: None,
            params: vec![],
            return_type: Type::Void,
        };
        let ast = create_test_program(vec![TopLevelItem::Import(import)]);
        let result = lower_program(&ast).unwrap();

        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module, "env");
        assert_eq!(result.imports[0].name, "print");
        assert_eq!(result.imports[0].return_type, Type::Void);
    }

    #[test]
    fn test_lower_simple_function() {
        let function = FunctionDef {
            name: "main",
            params: vec![],
            return_type: Type::Void,
            blocks: vec![Block {
                label: "entry",
                instructions: vec![],
                terminator: Terminator::Ret(None),
            }],
        };
        let ast = create_test_program(vec![TopLevelItem::Function(function)]);
        let result = lower_program(&ast).unwrap();

        assert_eq!(result.functions.len(), 1);
        assert_eq!(result.functions[0].name, "main");
        assert_eq!(result.functions[0].return_type, Type::Void);
        assert_eq!(result.functions[0].blocks.len(), 1);
        assert_eq!(result.functions[0].blocks[0].label, "entry");
    }

    #[test]
    fn test_lower_function_with_call() {
        let import = ImportDecl {
            module: "env",
            name: "getc",
            calling_convention: None,
            params: vec![],
            return_type: Type::I32,
        };
        let function = FunctionDef {
            name: "main",
            params: vec![],
            return_type: Type::Void,
            blocks: vec![Block {
                label: "entry",
                instructions: vec![Instruction::Assign {
                    dest: TypedIdentifier {
                        name: "result",
                        ty: Type::I32,
                    },
                    expr: Expression::Call {
                        name: "getc",
                        args: vec![],
                    },
                }],
                terminator: Terminator::Ret(None),
            }],
        };
        let ast = create_test_program(vec![
            TopLevelItem::Import(import),
            TopLevelItem::Function(function),
        ]);
        let result = lower_program(&ast).unwrap();

        assert_eq!(result.functions.len(), 1);
        assert_eq!(result.functions[0].blocks[0].instructions.len(), 1);

        if let crate::Instruction::Call {
            dest,
            function,
            return_type,
            ..
        } = &result.functions[0].blocks[0].instructions[0]
        {
            assert_eq!(*dest, ValueId(0));
            assert_eq!(function, "getc");
            assert_eq!(*return_type, Type::I32);
        } else {
            panic!("Expected Call instruction");
        }
    }

    #[test]
    fn test_lower_void_call() {
        let import = ImportDecl {
            module: "env",
            name: "putc",
            calling_convention: None,
            params: vec![],
            return_type: Type::Void,
        };
        let function = FunctionDef {
            name: "main",
            params: vec![],
            return_type: Type::Void,
            blocks: vec![Block {
                label: "entry",
                instructions: vec![Instruction::ExpressionStatement {
                    expr: Expression::Call {
                        name: "putc",
                        args: vec![],
                    },
                }],
                terminator: Terminator::Ret(None),
            }],
        };
        let ast = create_test_program(vec![
            TopLevelItem::Import(import),
            TopLevelItem::Function(function),
        ]);
        let result = lower_program(&ast).unwrap();

        assert_eq!(result.functions[0].blocks[0].instructions.len(), 1);

        if let crate::Instruction::CallVoid { function, .. } =
            &result.functions[0].blocks[0].instructions[0]
        {
            assert_eq!(function, "putc");
        } else {
            panic!("Expected CallVoid instruction");
        }
    }

    #[test]
    fn test_lower_branch_instructions() {
        let function = FunctionDef {
            name: "test",
            params: vec![],
            return_type: Type::Void,
            blocks: vec![
                Block {
                    label: "entry",
                    instructions: vec![],
                    terminator: Terminator::Br { label: "exit" },
                },
                Block {
                    label: "exit",
                    instructions: vec![],
                    terminator: Terminator::Ret(None),
                },
            ],
        };
        let ast = create_test_program(vec![TopLevelItem::Function(function)]);
        let result = lower_program(&ast).unwrap();

        assert_eq!(result.functions[0].blocks.len(), 2);

        if let crate::Terminator::Br { target } = &result.functions[0].blocks[0].terminator {
            assert_eq!(*target, BlockId(1)); // Should point to second block
        } else {
            panic!("Expected Br terminator");
        }
    }

    // Error case tests
    #[test]
    fn test_undefined_function_error() {
        let function = FunctionDef {
            name: "main",
            params: vec![],
            return_type: Type::Void,
            blocks: vec![Block {
                label: "entry",
                instructions: vec![Instruction::Assign {
                    dest: TypedIdentifier {
                        name: "result",
                        ty: Type::I32,
                    },
                    expr: Expression::Call {
                        name: "undefined_func",
                        args: vec![],
                    },
                }],
                terminator: Terminator::Ret(None),
            }],
        };
        let ast = create_test_program(vec![TopLevelItem::Function(function)]);
        let result = lower_program(&ast);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], SemanticError::FunctionNotFound { .. }));
    }

    #[test]
    fn test_type_mismatch_error() {
        let import = ImportDecl {
            module: "env",
            name: "void_func",
            calling_convention: None,
            params: vec![],
            return_type: Type::Void,
        };
        let function = FunctionDef {
            name: "main",
            params: vec![],
            return_type: Type::Void,
            blocks: vec![Block {
                label: "entry",
                instructions: vec![Instruction::Assign {
                    dest: TypedIdentifier {
                        name: "result",
                        ty: Type::I32,
                    },
                    expr: Expression::Call {
                        name: "void_func",
                        args: vec![],
                    },
                }],
                terminator: Terminator::Ret(None),
            }],
        };
        let ast = create_test_program(vec![
            TopLevelItem::Import(import),
            TopLevelItem::Function(function),
        ]);
        let result = lower_program(&ast);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], SemanticError::TypeMismatch { .. }));
    }

    #[test]
    fn test_undefined_block_error() {
        let function = FunctionDef {
            name: "test",
            params: vec![],
            return_type: Type::Void,
            blocks: vec![Block {
                label: "entry",
                instructions: vec![],
                terminator: Terminator::Br {
                    label: "undefined_block",
                },
            }],
        };
        let ast = create_test_program(vec![TopLevelItem::Function(function)]);
        let result = lower_program(&ast);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], SemanticError::UndefinedBlock { .. }));
    }

    #[test]
    fn test_duplicate_function_error() {
        let func1 = FunctionDef {
            name: "duplicate",
            params: vec![],
            return_type: Type::Void,
            blocks: vec![Block {
                label: "entry",
                instructions: vec![],
                terminator: Terminator::Ret(None),
            }],
        };
        let func2 = FunctionDef {
            name: "duplicate",
            params: vec![],
            return_type: Type::I32,
            blocks: vec![Block {
                label: "entry",
                instructions: vec![],
                terminator: Terminator::Ret(None), // This will also cause a type error
            }],
        };
        let ast = create_test_program(vec![
            TopLevelItem::Function(func1),
            TopLevelItem::Function(func2),
        ]);
        let result = lower_program(&ast);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        // Should have at least one duplicate definition error
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::DuplicateDefinition { .. }))
        );
    }

    #[test]
    fn test_return_type_mismatch() {
        let function = FunctionDef {
            name: "test",
            params: vec![],
            return_type: Type::I32,
            blocks: vec![Block {
                label: "entry",
                instructions: vec![],
                terminator: Terminator::Ret(None), // Void return for I32 function
            }],
        };
        let ast = create_test_program(vec![TopLevelItem::Function(function)]);
        let result = lower_program(&ast);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], SemanticError::TypeMismatch { .. }));
    }
}

#[cfg(test)]
mod integration_tests {
    use crate::{SemanticError, lowering::lower_program};
    use tilt_ast::*;

    #[test]
    fn test_complete_program() {
        // A more complex program with multiple functions and control flow
        let import = ImportDecl {
            module: "env",
            name: "print_i32",
            calling_convention: None,
            params: vec![TypedIdentifier {
                name: "value",
                ty: Type::I32,
            }],
            return_type: Type::Void,
        };

        let helper_func = FunctionDef {
            name: "add_one",
            params: vec![TypedIdentifier {
                name: "x",
                ty: Type::I32,
            }],
            return_type: Type::I32,
            blocks: vec![Block {
                label: "entry",
                instructions: vec![],
                terminator: Terminator::Ret(Some(Value::Variable("undefined_var"))), // This should fail
            }],
        };

        let main_func = FunctionDef {
            name: "main",
            params: vec![],
            return_type: Type::Void,
            blocks: vec![
                Block {
                    label: "entry",
                    instructions: vec![],
                    terminator: Terminator::Br { label: "exit" },
                },
                Block {
                    label: "exit",
                    instructions: vec![],
                    terminator: Terminator::Ret(None),
                },
            ],
        };

        let ast = Program {
            items: vec![
                TopLevelItem::Import(import),
                TopLevelItem::Function(helper_func),
                TopLevelItem::Function(main_func),
            ],
        };

        let result = lower_program(&ast);

        // This should fail because we're using an undefined variable 'x' in return
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::UndefinedIdentifier { .. }))
        );
    }
}
