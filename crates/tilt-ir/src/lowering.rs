// ===================================================================
// FILE: lowering.rs (tilt-ir crate)
//
// DESC: The one-pass semantic analyzer and IR generator. Takes an AST
//       and produces validated IR or semantic errors.
// ===================================================================

use std::collections::HashMap;
use crate::*;
use tilt_ast;

/// Context for lowering AST to IR with semantic validation
pub struct LoweringContext {
    /// Current function being processed
    current_function: Option<Function>,
    /// Map from block labels to block IDs for the current function
    block_map: HashMap<String, BlockId>,
    /// Map from variable names to value IDs for the current function
    value_map: HashMap<String, (ValueId, Type)>,
    /// Available functions (from imports and function definitions)
    functions: HashMap<String, (Vec<Type>, Type)>, // (params, return_type)
    /// Errors collected during lowering
    errors: Vec<SemanticError>,
    /// Next block ID to assign
    next_block_id: usize,
}

impl LoweringContext {
    pub fn new() -> Self {
        Self {
            current_function: None,
            block_map: HashMap::new(),
            value_map: HashMap::new(),
            functions: HashMap::new(),
            errors: Vec::new(),
            next_block_id: 0,
        }
    }

    /// Generate the next unique block ID
    fn next_block(&mut self) -> BlockId {
        let id = BlockId(self.next_block_id);
        self.next_block_id += 1;
        id
    }

    /// Add an error to the context
    fn error(&mut self, error: SemanticError) {
        self.errors.push(error);
    }

    /// Register a function signature
    fn register_function(&mut self, name: String, params: Vec<Type>, return_type: Type) {
        if self.functions.contains_key(&name) {
            self.error(SemanticError::DuplicateDefinition {
                name: name.clone(),
                location: "function definition".to_string(),
            });
        } else {
            self.functions.insert(name, (params, return_type));
        }
    }

    /// Look up a function signature
    fn lookup_function(&self, name: &str) -> Option<&(Vec<Type>, Type)> {
        self.functions.get(name)
    }

    /// Register a variable in the current scope
    fn register_variable(&mut self, name: String, value_id: ValueId, ty: Type) {
        if self.value_map.contains_key(&name) {
            self.error(SemanticError::DuplicateDefinition {
                name: name.clone(),
                location: "variable definition".to_string(),
            });
        } else {
            self.value_map.insert(name, (value_id, ty));
        }
    }

    /// Look up a variable in the current scope
    fn lookup_variable(&self, name: &str) -> Option<(ValueId, Type)> {
        self.value_map.get(name).copied()
    }

    /// Clear function-local state
    fn clear_function_scope(&mut self) {
        self.block_map.clear();
        self.value_map.clear();
        self.current_function = None;
    }
}

/// Main entry point for lowering AST to IR
pub fn lower_program(ast: &tilt_ast::Program) -> Result<Program, Vec<SemanticError>> {
    let mut ctx = LoweringContext::new();
    
    // First pass: collect all import and function signatures
    for item in &ast.items {
        match item {
            tilt_ast::TopLevelItem::Import(import) => {
                let params = import.params.iter().map(|p| p.ty).collect();
                ctx.register_function(import.name.to_string(), params, import.return_type);
            }
            tilt_ast::TopLevelItem::Function(func) => {
                let params = func.params.iter().map(|p| p.ty).collect();
                ctx.register_function(func.name.to_string(), params, func.return_type);
            }
        }
    }

    // Second pass: lower each item
    let mut ir_imports = Vec::new();
    let mut ir_functions = Vec::new();

    for item in &ast.items {
        match item {
            tilt_ast::TopLevelItem::Import(import) => {
                let ir_import = lower_import(&mut ctx, import);
                ir_imports.push(ir_import);
            }
            tilt_ast::TopLevelItem::Function(func) => {
                match lower_function(&mut ctx, func) {
                    Ok(ir_func) => ir_functions.push(ir_func),
                    Err(_) => {
                        // Errors are already added to ctx.errors
                    }
                }
            }
        }
    }

    if ctx.errors.is_empty() {
        Ok(Program {
            imports: ir_imports,
            functions: ir_functions,
        })
    } else {
        Err(ctx.errors)
    }
}

/// Lower an import declaration
fn lower_import(_ctx: &mut LoweringContext, import: &tilt_ast::ImportDecl) -> ImportDecl {
    ImportDecl {
        module: import.module.to_string(),
        name: import.name.to_string(),
        params: import.params.iter().map(|p| p.ty).collect(),
        return_type: import.return_type,
    }
}

/// Lower a function definition
fn lower_function(ctx: &mut LoweringContext, func: &tilt_ast::FunctionDef) -> Result<Function, ()> {
    ctx.clear_function_scope();
    
    let mut ir_func = Function::new(
        func.name.to_string(),
        func.params.iter().map(|p| p.ty).collect(),
        func.return_type,
    );

    ctx.current_function = Some(ir_func.clone());

    // Register function parameters as variables
    for param in &func.params {
        let value_id = ir_func.next_value();
        ctx.register_variable(param.name.to_string(), value_id, param.ty);
    }

    // First pass: assign block IDs to all block labels
    for block in &func.blocks {
        let block_id = ctx.next_block();
        if ctx.block_map.contains_key(block.label) {
            ctx.error(SemanticError::DuplicateDefinition {
                name: block.label.to_string(),
                location: format!("block in function '{}'", func.name),
            });
        } else {
            ctx.block_map.insert(block.label.to_string(), block_id);
        }
    }

    // Set entry block (first block)
    if let Some(first_block) = func.blocks.first() {
        if let Some(&entry_id) = ctx.block_map.get(first_block.label) {
            ir_func.entry_block = entry_id;
        }
    }

    // Second pass: lower each block
    let mut ir_blocks = Vec::new();
    for block in &func.blocks {
        match lower_block(ctx, &mut ir_func, block) {
            Ok(ir_block) => ir_blocks.push(ir_block),
            Err(_) => {
                // Errors already added to ctx
            }
        }
    }

    ir_func.blocks = ir_blocks;

    if ctx.errors.is_empty() {
        Ok(ir_func)
    } else {
        Err(())
    }
}

/// Lower a basic block
fn lower_block(
    ctx: &mut LoweringContext,
    func: &mut Function,
    block: &tilt_ast::Block,
) -> Result<BasicBlock, ()> {
    let block_id = ctx.block_map.get(block.label)
        .copied()
        .expect("Block ID should have been assigned in first pass");

    let mut ir_block = BasicBlock::new(block_id, block.label.to_string());

    // Lower instructions
    for instruction in &block.instructions {
        match lower_instruction(ctx, func, instruction) {
            Ok(ir_instruction) => ir_block.instructions.push(ir_instruction),
            Err(_) => {
                // Error already added to ctx
            }
        }
    }

    // Lower terminator
    match lower_terminator(ctx, func, &block.terminator) {
        Ok(ir_terminator) => ir_block.terminator = ir_terminator,
        Err(_) => {
            // Error already added to ctx
        }
    }

    Ok(ir_block)
}

/// Lower an instruction
fn lower_instruction(
    ctx: &mut LoweringContext,
    func: &mut Function,
    instruction: &tilt_ast::Instruction,
) -> Result<Instruction, ()> {
    match instruction {
        tilt_ast::Instruction::Assign { dest, expr } => {
            let dest_value_id = func.next_value();
            ctx.register_variable(dest.name.to_string(), dest_value_id, dest.ty);

            match expr {
                tilt_ast::Expression::Call { name, args } => {
                    // Look up function
                    if let Some((param_types, return_type)) = ctx.lookup_function(name).cloned() {
                        // Check argument count
                        if args.len() != param_types.len() {
                            ctx.error(SemanticError::ArgumentMismatch {
                                function: name.to_string(),
                                expected: param_types.len(),
                                found: args.len(),
                                location: format!("assignment in block '{}'", "current"), // TODO: better location
                            });
                            return Err(());
                        }

                        // Check return type matches destination
                        if return_type != dest.ty {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: dest.ty,
                                found: return_type,
                                location: format!("assignment in block '{}'", "current"),
                            });
                            return Err(());
                        }

                        // Lower arguments
                        let mut ir_args = Vec::new();
                        for (arg, expected_type) in args.iter().zip(param_types.iter()) {
                            match arg {
                                tilt_ast::Value::Variable(var_name) => {
                                    if let Some((value_id, actual_type)) = ctx.lookup_variable(var_name) {
                                        // Type check
                                        if actual_type != *expected_type {
                                            ctx.error(SemanticError::TypeMismatch {
                                                expected: *expected_type,
                                                found: actual_type,
                                                location: format!("argument to function '{}'", name),
                                            });
                                            return Err(());
                                        }
                                        ir_args.push(value_id);
                                    } else {
                                        ctx.error(SemanticError::UndefinedIdentifier {
                                            name: var_name.to_string(),
                                            location: format!("argument to function '{}'", name),
                                        });
                                        return Err(());
                                    }
                                }
                                tilt_ast::Value::Constant(const_val) => {
                                    // Create a constant instruction for this argument
                                    let const_value_id = func.next_value();
                                    func.constants.insert(const_value_id, (*const_val, *expected_type));
                                    ir_args.push(const_value_id);
                                }
                            }
                        }

                        Ok(Instruction::Call {
                            dest: dest_value_id,
                            function: name.to_string(),
                            args: ir_args,
                            return_type: return_type,
                        })
                    } else {
                        ctx.error(SemanticError::FunctionNotFound {
                            name: name.to_string(),
                            location: format!("assignment in block '{}'", "current"),
                        });
                        Err(())
                    }
                }
                tilt_ast::Expression::Operation { op, args } => {
                    // Parse operation (e.g., "i32.add" -> BinaryOperator::Add)
                    if let Some(dot_pos) = op.find('.') {
                        let type_part = &op[..dot_pos];
                        let op_part = &op[dot_pos + 1..];

                        let ty = match type_part {
                            "i32" => Type::I32,
                            "i64" => Type::I64,
                            "f32" => Type::F32,
                            "f64" => Type::F64,
                            _ => {
                                ctx.error(SemanticError::InvalidOperation {
                                    operation: op.to_string(),
                                    ty: dest.ty,
                                    location: "operation".to_string(),
                                });
                                return Err(());
                            }
                        };

                        // Check that destination type matches operation type
                        if ty != dest.ty {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: dest.ty,
                                found: ty,
                                location: "operation result".to_string(),
                            });
                            return Err(());
                        }

                        if args.len() == 2 {
                            // Binary operation
                            let binary_op = BinaryOperator::from_str(op_part, ty)
                                .map_err(|e| ctx.error(e))?;

                            let (lhs_id, lhs_type) = lower_value(ctx, &args[0])?;
                            let (rhs_id, rhs_type) = lower_value(ctx, &args[1])?;

                            // Type check operands
                            if lhs_type != ty {
                                ctx.error(SemanticError::TypeMismatch {
                                    expected: ty,
                                    found: lhs_type,
                                    location: "binary operation left operand".to_string(),
                                });
                                return Err(());
                            }
                            if rhs_type != ty {
                                ctx.error(SemanticError::TypeMismatch {
                                    expected: ty,
                                    found: rhs_type,
                                    location: "binary operation right operand".to_string(),
                                });
                                return Err(());
                            }

                            Ok(Instruction::BinaryOp {
                                dest: dest_value_id,
                                op: binary_op,
                                ty,
                                lhs: lhs_id,
                                rhs: rhs_id,
                            })
                        } else if args.len() == 1 {
                            // Unary operation or constant
                            if op_part == "const" {
                                // Handle constants like "i32.const 42"
                                if let tilt_ast::Value::Constant(val) = &args[0] {
                                    Ok(Instruction::Const {
                                        dest: dest_value_id,
                                        value: *val,
                                        ty,
                                    })
                                } else {
                                    ctx.error(SemanticError::InvalidOperation {
                                        operation: format!("{}.const requires a constant value", type_part),
                                        ty,
                                        location: "constant operation".to_string(),
                                    });
                                    Err(())
                                }
                            } else {
                                // Unary operation
                                let unary_op = UnaryOperator::from_str(op_part, ty)
                                    .map_err(|e| ctx.error(e))?;

                                let (operand_id, operand_type) = lower_value(ctx, &args[0])?;

                                if operand_type != ty {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: ty,
                                        found: operand_type,
                                        location: "unary operation operand".to_string(),
                                    });
                                    return Err(());
                                }

                                Ok(Instruction::UnaryOp {
                                    dest: dest_value_id,
                                    op: unary_op,
                                    ty,
                                    operand: operand_id,
                                })
                            }
                        } else {
                            ctx.error(SemanticError::InvalidOperation {
                                operation: format!("{} with {} arguments", op, args.len()),
                                ty: dest.ty,
                                location: "operation".to_string(),
                            });
                            Err(())
                        }
                    } else {
                        ctx.error(SemanticError::InvalidOperation {
                            operation: op.to_string(),
                            ty: dest.ty,
                            location: "operation".to_string(),
                        });
                        Err(())
                    }
                }
                tilt_ast::Expression::Phi { nodes: _ } => {
                    // Phi nodes are handled as block parameters in our IR
                    // For now, we'll skip them and handle them separately
                    ctx.error(SemanticError::InvalidOperation {
                        operation: "phi nodes not yet implemented".to_string(),
                        ty: dest.ty,
                        location: "phi expression".to_string(),
                    });
                    Err(())
                }
            }
        }
        tilt_ast::Instruction::Call { name, args } => {
            // Void function call
            if let Some((param_types, return_type)) = ctx.lookup_function(name).cloned() {
                // Check that this is actually a void function
                if return_type != Type::Void {
                    ctx.error(SemanticError::TypeMismatch {
                        expected: Type::Void,
                        found: return_type,
                        location: format!("void call to function '{}'", name),
                    });
                    return Err(());
                }

                // Check argument count
                if args.len() != param_types.len() {
                    ctx.error(SemanticError::ArgumentMismatch {
                        function: name.to_string(),
                        expected: param_types.len(),
                        found: args.len(),
                        location: "void function call".to_string(),
                    });
                    return Err(());
                }

                // Lower arguments
                let mut ir_args = Vec::new();
                for (arg, expected_type) in args.iter().zip(param_types.iter()) {
                    match arg {
                        tilt_ast::Value::Variable(var_name) => {
                            if let Some((value_id, actual_type)) = ctx.lookup_variable(var_name) {
                                if actual_type != *expected_type {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: *expected_type,
                                        found: actual_type,
                                        location: format!("argument to function '{}'", name),
                                    });
                                    return Err(());
                                }
                                ir_args.push(value_id);
                            } else {
                                ctx.error(SemanticError::UndefinedIdentifier {
                                    name: var_name.to_string(),
                                    location: format!("argument to function '{}'", name),
                                });
                                return Err(());
                            }
                        }
                        tilt_ast::Value::Constant(const_val) => {
                            // Create a constant instruction for this argument
                            let const_value_id = func.next_value();
                            func.constants.insert(const_value_id, (*const_val, *expected_type));
                            ir_args.push(const_value_id);
                        }
                    }
                }

                Ok(Instruction::CallVoid {
                    function: name.to_string(),
                    args: ir_args,
                })
            } else {
                ctx.error(SemanticError::FunctionNotFound {
                    name: name.to_string(),
                    location: "void function call".to_string(),
                });
                Err(())
            }
        }
        tilt_ast::Instruction::Store { op, address, value } => {
            // Parse store operation (e.g., "i32.store")
            if let Some(dot_pos) = op.find('.') {
                let type_part = &op[..dot_pos];
                let op_part = &op[dot_pos + 1..];

                if op_part != "store" {
                    ctx.error(SemanticError::InvalidOperation {
                        operation: op.to_string(),
                        ty: Type::Void, // Store doesn't have a type
                        location: "store instruction".to_string(),
                    });
                    return Err(());
                }

                let ty = match type_part {
                    "i32" => Type::I32,
                    "i64" => Type::I64,
                    "f32" => Type::F32,
                    "f64" => Type::F64,
                    _ => {
                        ctx.error(SemanticError::InvalidOperation {
                            operation: op.to_string(),
                            ty: Type::Void,
                            location: "store instruction".to_string(),
                        });
                        return Err(());
                    }
                };

                let (addr_id, _addr_type) = lower_value(ctx, address)?; // Address type checking skipped for now
                let (val_id, val_type) = lower_value(ctx, value)?;

                // Check that value type matches store type
                if val_type != ty {
                    ctx.error(SemanticError::TypeMismatch {
                        expected: ty,
                        found: val_type,
                        location: "store value".to_string(),
                    });
                    return Err(());
                }

                Ok(Instruction::Store {
                    address: addr_id,
                    value: val_id,
                    ty,
                })
            } else {
                ctx.error(SemanticError::InvalidOperation {
                    operation: op.to_string(),
                    ty: Type::Void,
                    location: "store instruction".to_string(),
                });
                Err(())
            }
        }
    }
}

/// Lower a terminator
fn lower_terminator(
    ctx: &mut LoweringContext,
    _func: &mut Function,
    terminator: &tilt_ast::Terminator,
) -> Result<Terminator, ()> {
    match terminator {
        tilt_ast::Terminator::Ret(value_opt) => {
            if let Some(value) = value_opt {
                let (value_id, value_type) = lower_value(ctx, value)?;
                
                // Check that return type matches function return type
                if let Some(current_func) = &ctx.current_function {
                    if value_type != current_func.return_type {
                        ctx.error(SemanticError::TypeMismatch {
                            expected: current_func.return_type,
                            found: value_type,
                            location: "return value".to_string(),
                        });
                        return Err(());
                    }
                }

                Ok(Terminator::Ret {
                    value: Some(value_id),
                })
            } else {
                // Check that function return type is void
                if let Some(current_func) = &ctx.current_function {
                    if current_func.return_type != Type::Void {
                        ctx.error(SemanticError::TypeMismatch {
                            expected: current_func.return_type,
                            found: Type::Void,
                            location: "void return".to_string(),
                        });
                        return Err(());
                    }
                }

                Ok(Terminator::Ret { value: None })
            }
        }
        tilt_ast::Terminator::Br { label } => {
            if let Some(&target_id) = ctx.block_map.get(*label) {
                Ok(Terminator::Br { target: target_id })
            } else {
                ctx.error(SemanticError::UndefinedBlock {
                    label: label.to_string(),
                    location: "branch target".to_string(),
                });
                Err(())
            }
        }
        tilt_ast::Terminator::BrIf { cond, true_label, false_label } => {
            let (cond_id, cond_type) = lower_value(ctx, cond)?;

            // Check that condition is an integer type
            match cond_type {
                Type::I32 | Type::I64 => {
                    // OK
                }
                _ => {
                    ctx.error(SemanticError::TypeMismatch {
                        expected: Type::I32, // Could also accept I64
                        found: cond_type,
                        location: "branch condition".to_string(),
                    });
                    return Err(());
                }
            }

            let true_target = if let Some(&target_id) = ctx.block_map.get(*true_label) {
                target_id
            } else {
                ctx.error(SemanticError::UndefinedBlock {
                    label: true_label.to_string(),
                    location: "true branch target".to_string(),
                });
                return Err(());
            };

            let false_target = if let Some(&target_id) = ctx.block_map.get(*false_label) {
                target_id
            } else {
                ctx.error(SemanticError::UndefinedBlock {
                    label: false_label.to_string(),
                    location: "false branch target".to_string(),
                });
                return Err(());
            };

            Ok(Terminator::BrIf {
                cond: cond_id,
                true_target,
                false_target,
            })
        }
    }
}

/// Lower a value (variable reference or constant)
fn lower_value(
    ctx: &mut LoweringContext,
    value: &tilt_ast::Value,
) -> Result<(ValueId, Type), ()> {
    match value {
        tilt_ast::Value::Variable(name) => {
            if let Some((value_id, ty)) = ctx.lookup_variable(name) {
                Ok((value_id, ty))
            } else {
                ctx.error(SemanticError::UndefinedIdentifier {
                    name: name.to_string(),
                    location: "variable reference".to_string(),
                });
                Err(())
            }
        }
        tilt_ast::Value::Constant(_) => {
            // Constants need to be created as instructions
            // For now, we'll return an error and handle them in the context where the type is known
            ctx.error(SemanticError::InvalidOperation {
                operation: "bare constants not supported in this context".to_string(),
                ty: Type::I32, // Placeholder
                location: "constant value".to_string(),
            });
            Err(())
        }
    }
}


