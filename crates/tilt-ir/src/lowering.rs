// ===================================================================
// FILE: lowering.rs (tilt-ir crate)
//
// DESC: The one-pass semantic analyzer and IR generator. Takes an AST
//       and produces validated IR or semantic errors.
// ===================================================================

use crate::*;
use std::collections::HashMap;
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
        calling_convention: import.calling_convention.map(|s| s.to_string()),
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
    let block_id = ctx
        .block_map
        .get(block.label)
        .copied()
        .expect("Block ID should have been assigned in first pass");

    let mut ir_block = BasicBlock::new(block_id, block.label.to_string());

    // Add block parameters
    for param in &block.params {
        let param_type = param.ty;
        let value_id = func.next_value();
        ir_block.params.push((value_id, param_type));

        // Map the parameter name to the value ID with its type
        ctx.value_map
            .insert(param.name.to_string(), (value_id, param_type));
    }

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
                                    if let Some((value_id, actual_type)) =
                                        ctx.lookup_variable(var_name)
                                    {
                                        // Type check
                                        if actual_type != *expected_type {
                                            ctx.error(SemanticError::TypeMismatch {
                                                expected: *expected_type,
                                                found: actual_type,
                                                location: format!(
                                                    "argument to function '{}'",
                                                    name
                                                ),
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
                                    func.constants.insert(
                                        const_value_id,
                                        (*const_val as i64, *expected_type),
                                    );
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
                    // First check if this is a function call (no dot in operation name)
                    if !op.contains('.')
                        && *op != "usize.add"
                        && *op != "alloc"
                        && *op != "free"
                        && !op.starts_with("sizeof.")
                    {
                        // Check if it's a known function
                        let function_signature = ctx.lookup_function(op).cloned();
                        if let Some((param_types, return_type)) = function_signature {
                            // Check argument count
                            if args.len() != param_types.len() {
                                ctx.error(SemanticError::ArgumentMismatch {
                                    function: op.to_string(),
                                    expected: param_types.len(),
                                    found: args.len(),
                                    location: "function call".to_string(),
                                });
                                return Err(());
                            }

                            // Check return type matches destination
                            if return_type != dest.ty {
                                ctx.error(SemanticError::TypeMismatch {
                                    expected: dest.ty,
                                    found: return_type,
                                    location: "function call return type".to_string(),
                                });
                                return Err(());
                            }

                            // Lower arguments
                            let mut arg_ids = Vec::new();
                            for (i, arg) in args.iter().enumerate() {
                                let expected_type = param_types[i];
                                let (arg_id, arg_type) =
                                    lower_value_with_func(ctx, func, arg, expected_type)?;

                                if arg_type != expected_type {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: expected_type,
                                        found: arg_type,
                                        location: format!(
                                            "argument {} to function '{}'",
                                            i + 1,
                                            op
                                        ),
                                    });
                                    return Err(());
                                }

                                arg_ids.push(arg_id);
                            }

                            return Ok(Instruction::Call {
                                dest: dest_value_id,
                                function: op.to_string(),
                                args: arg_ids,
                                return_type,
                            });
                        }
                    }

                    // Handle new memory operations
                    if *op == "usize.add" {
                        // usize.add ptr_val, offset_val
                        if args.len() != 2 {
                            ctx.error(SemanticError::InvalidOperation {
                                operation: format!(
                                    "usize.add with {} arguments (expected 2)",
                                    args.len()
                                ),
                                ty: dest.ty,
                                location: "usize.add operation".to_string(),
                            });
                            return Err(());
                        }

                        if dest.ty != Type::Usize {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: Type::Usize,
                                found: dest.ty,
                                location: "usize.add result".to_string(),
                            });
                            return Err(());
                        }

                        let (ptr_id, ptr_type) =
                            lower_value_with_func(ctx, func, &args[0], Type::Usize)?;
                        let (offset_id, offset_type) =
                            lower_value_with_func(ctx, func, &args[1], Type::Usize)?;

                        if ptr_type != Type::Usize {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: Type::Usize,
                                found: ptr_type,
                                location: "usize.add first operand".to_string(),
                            });
                            return Err(());
                        }
                        if offset_type != Type::Usize {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: Type::Usize,
                                found: offset_type,
                                location: "usize.add second operand".to_string(),
                            });
                            return Err(());
                        }

                        return Ok(Instruction::PtrAdd {
                            dest: dest_value_id,
                            ptr: ptr_id,
                            offset: offset_id,
                        });
                    } else if op.starts_with("sizeof.") {
                        // sizeof.i32, sizeof.i64, etc.
                        if !args.is_empty() {
                            ctx.error(SemanticError::InvalidOperation {
                                operation: format!(
                                    "sizeof with {} arguments (expected 0)",
                                    args.len()
                                ),
                                ty: dest.ty,
                                location: "sizeof operation".to_string(),
                            });
                            return Err(());
                        }

                        if dest.ty != Type::Usize {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: Type::Usize,
                                found: dest.ty,
                                location: "sizeof result".to_string(),
                            });
                            return Err(());
                        }

                        let type_part = &op[7..]; // Skip "sizeof."
                        let target_type = match type_part {
                            "i32" => Type::I32,
                            "i64" => Type::I64,
                            "f32" => Type::F32,
                            "f64" => Type::F64,
                            "ptr" => Type::Usize,
                            "void" => Type::Void,
                            _ => {
                                ctx.error(SemanticError::InvalidOperation {
                                    operation: format!("sizeof.{} with unknown type", type_part),
                                    ty: dest.ty,
                                    location: "sizeof operation".to_string(),
                                });
                                return Err(());
                            }
                        };

                        return Ok(Instruction::SizeOf {
                            dest: dest_value_id,
                            ty: target_type,
                        });
                    } else if *op == "alloc" {
                        // alloc size_val
                        if args.len() != 1 {
                            ctx.error(SemanticError::InvalidOperation {
                                operation: format!(
                                    "alloc with {} arguments (expected 1)",
                                    args.len()
                                ),
                                ty: dest.ty,
                                location: "alloc operation".to_string(),
                            });
                            return Err(());
                        }

                        if dest.ty != Type::Usize {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: Type::Usize,
                                found: dest.ty,
                                location: "alloc result".to_string(),
                            });
                            return Err(());
                        }

                        let (size_id, size_type) =
                            lower_value_with_func(ctx, func, &args[0], Type::Usize)?;

                        if size_type != Type::Usize {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: Type::Usize,
                                found: size_type,
                                location: "alloc size operand".to_string(),
                            });
                            return Err(());
                        }

                        return Ok(Instruction::Alloc {
                            dest: dest_value_id,
                            size: size_id,
                        });
                    }

                    // Parse operation (e.g., "i32.add" -> BinaryOperator::Add)
                    if let Some(dot_pos) = op.find('.') {
                        let type_part = &op[..dot_pos];
                        let op_part = &op[dot_pos + 1..];

                        // Handle conversion operations first (these don't follow normal type rules)
                        if op_part.starts_with("to_") {
                            return handle_conversion_operation(
                                ctx,
                                func,
                                dest,
                                dest_value_id,
                                op,
                                type_part,
                                op_part,
                                &args,
                            );
                        }

                        let ty = match type_part {
                            "i32" => Type::I32,
                            "i64" => Type::I64,
                            "f32" => Type::F32,
                            "f64" => Type::F64,
                            "ptr" => Type::Usize,
                            "usize" => Type::Usize,
                            _ => {
                                ctx.error(SemanticError::InvalidOperation {
                                    operation: op.to_string(),
                                    ty: dest.ty,
                                    location: "operation".to_string(),
                                });
                                return Err(());
                            }
                        };

                        // Handle memory load operations
                        if op_part == "load" {
                            if args.len() != 1 {
                                ctx.error(SemanticError::InvalidOperation {
                                    operation: format!(
                                        "{}.load with {} arguments (expected 1)",
                                        type_part,
                                        args.len()
                                    ),
                                    ty: dest.ty,
                                    location: "load operation".to_string(),
                                });
                                return Err(());
                            }

                            if ty != dest.ty {
                                ctx.error(SemanticError::TypeMismatch {
                                    expected: dest.ty,
                                    found: ty,
                                    location: "load result".to_string(),
                                });
                                return Err(());
                            }

                            let (addr_id, addr_type) =
                                lower_value_with_func(ctx, func, &args[0], Type::Usize)?;

                            if addr_type != Type::Usize {
                                ctx.error(SemanticError::TypeMismatch {
                                    expected: Type::Usize,
                                    found: addr_type,
                                    location: "load address operand".to_string(),
                                });
                                return Err(());
                            }

                            return Ok(Instruction::Load {
                                dest: dest_value_id,
                                ty,
                                address: addr_id,
                            });
                        }

                        // Check that destination type matches operation type
                        if ty != dest.ty {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: dest.ty,
                                found: ty,
                                location: format!(
                                    "operation '{}': destination expects {:?} but operation '{}' produces {:?}",
                                    op, dest.ty, op, ty
                                ),
                            });
                            return Err(());
                        }

                        if args.len() == 2 {
                            // Binary operation
                            let binary_op =
                                BinaryOperator::from_str(op_part, ty).map_err(|e| ctx.error(e))?;

                            let (lhs_id, lhs_type) =
                                lower_value_with_func(ctx, func, &args[0], ty)?;
                            let (rhs_id, rhs_type) =
                                lower_value_with_func(ctx, func, &args[1], ty)?;

                            // Type check operands
                            if lhs_type != ty {
                                ctx.error(SemanticError::TypeMismatch {
                                    expected: ty,
                                    found: lhs_type,
                                    location: format!(
                                        "operation '{}': left operand expected {:?} but got {:?}",
                                        op, ty, lhs_type
                                    ),
                                });
                                return Err(());
                            }
                            if rhs_type != ty {
                                ctx.error(SemanticError::TypeMismatch {
                                    expected: ty,
                                    found: rhs_type,
                                    location: format!(
                                        "operation '{}': right operand expected {:?} but got {:?}",
                                        op, ty, rhs_type
                                    ),
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
                                        value: *val as i64,
                                        ty,
                                    })
                                } else {
                                    ctx.error(SemanticError::InvalidOperation {
                                        operation: format!(
                                            "{}.const requires a constant value",
                                            type_part
                                        ),
                                        ty,
                                        location: "constant operation".to_string(),
                                    });
                                    Err(())
                                }
                            } else if op_part == "extend" && type_part == "i32" {
                                // Handle i32.extend to convert i32 to i64 or usize
                                if dest.ty != Type::I64 && dest.ty != Type::Usize {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I64,
                                        found: dest.ty,
                                        location: "i32.extend result must be i64 or usize"
                                            .to_string(),
                                    });
                                    return Err(());
                                }

                                let (operand_id, operand_type) =
                                    lower_value_with_func(ctx, func, &args[0], Type::I32)?;

                                if operand_type != Type::I32 {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I32,
                                        found: operand_type,
                                        location: "i32.extend operand".to_string(),
                                    });
                                    return Err(());
                                }

                                Ok(Instruction::Convert {
                                    dest: dest_value_id,
                                    src: operand_id,
                                    from_ty: Type::I32,
                                    to_ty: dest.ty,
                                })
                            } else if op_part == "extend" && type_part == "usize" {
                                // Handle usize.extend to convert usize to i64
                                if dest.ty != Type::I64 {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I64,
                                        found: dest.ty,
                                        location: "usize.extend result must be i64".to_string(),
                                    });
                                    return Err(());
                                }

                                let (operand_id, operand_type) =
                                    lower_value_with_func(ctx, func, &args[0], Type::Usize)?;

                                if operand_type != Type::Usize {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::Usize,
                                        found: operand_type,
                                        location: "usize.extend operand".to_string(),
                                    });
                                    return Err(());
                                }

                                Ok(Instruction::Convert {
                                    dest: dest_value_id,
                                    src: operand_id,
                                    from_ty: Type::Usize,
                                    to_ty: Type::I64,
                                })
                            } else if op_part == "trunc" && type_part == "i64" {
                                // Handle i64.trunc to convert i64 to i32 or usize
                                if dest.ty != Type::I32 && dest.ty != Type::Usize {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I32,
                                        found: dest.ty,
                                        location: "i64.trunc result must be i32 or usize"
                                            .to_string(),
                                    });
                                    return Err(());
                                }

                                let (operand_id, operand_type) =
                                    lower_value_with_func(ctx, func, &args[0], Type::I64)?;

                                if operand_type != Type::I64 {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I64,
                                        found: operand_type,
                                        location: "i64.trunc operand".to_string(),
                                    });
                                    return Err(());
                                }

                                Ok(Instruction::Convert {
                                    dest: dest_value_id,
                                    src: operand_id,
                                    from_ty: Type::I64,
                                    to_ty: dest.ty,
                                })
                            } else if op_part == "to_i64" && type_part == "i32" {
                                // Handle i32.to_i64 conversion
                                if dest.ty != Type::I64 {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I64,
                                        found: dest.ty,
                                        location: "i32.to_i64 result must be i64".to_string(),
                                    });
                                    return Err(());
                                }

                                let (operand_id, operand_type) =
                                    lower_value_with_func(ctx, func, &args[0], Type::I32)?;

                                if operand_type != Type::I32 {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I32,
                                        found: operand_type,
                                        location: "i32.to_i64 operand".to_string(),
                                    });
                                    return Err(());
                                }

                                Ok(Instruction::Convert {
                                    dest: dest_value_id,
                                    src: operand_id,
                                    from_ty: Type::I32,
                                    to_ty: Type::I64,
                                })
                            } else if op_part == "to_usize" && type_part == "i32" {
                                // Handle i32.to_usize conversion
                                if dest.ty != Type::Usize {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::Usize,
                                        found: dest.ty,
                                        location: "i32.to_usize result must be usize".to_string(),
                                    });
                                    return Err(());
                                }

                                let (operand_id, operand_type) =
                                    lower_value_with_func(ctx, func, &args[0], Type::I32)?;

                                if operand_type != Type::I32 {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I32,
                                        found: operand_type,
                                        location: "i32.to_usize operand".to_string(),
                                    });
                                    return Err(());
                                }

                                Ok(Instruction::Convert {
                                    dest: dest_value_id,
                                    src: operand_id,
                                    from_ty: Type::I32,
                                    to_ty: Type::Usize,
                                })
                            } else if op_part == "to_i32" && type_part == "i64" {
                                // Handle i64.to_i32 conversion
                                if dest.ty != Type::I32 {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I32,
                                        found: dest.ty,
                                        location: "i64.to_i32 result must be i32".to_string(),
                                    });
                                    return Err(());
                                }

                                let (operand_id, operand_type) =
                                    lower_value_with_func(ctx, func, &args[0], Type::I64)?;

                                if operand_type != Type::I64 {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I64,
                                        found: operand_type,
                                        location: "i64.to_i32 operand".to_string(),
                                    });
                                    return Err(());
                                }

                                Ok(Instruction::Convert {
                                    dest: dest_value_id,
                                    src: operand_id,
                                    from_ty: Type::I64,
                                    to_ty: Type::I32,
                                })
                            } else if op_part == "to_usize" && type_part == "i64" {
                                // Handle i64.to_usize conversion
                                if dest.ty != Type::Usize {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::Usize,
                                        found: dest.ty,
                                        location: "i64.to_usize result must be usize".to_string(),
                                    });
                                    return Err(());
                                }

                                let (operand_id, operand_type) =
                                    lower_value_with_func(ctx, func, &args[0], Type::I64)?;

                                if operand_type != Type::I64 {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I64,
                                        found: operand_type,
                                        location: "i64.to_usize operand".to_string(),
                                    });
                                    return Err(());
                                }

                                Ok(Instruction::Convert {
                                    dest: dest_value_id,
                                    src: operand_id,
                                    from_ty: Type::I64,
                                    to_ty: Type::Usize,
                                })
                            } else if op_part == "to_i64" && type_part == "usize" {
                                // Handle usize.to_i64 conversion
                                if dest.ty != Type::I64 {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::I64,
                                        found: dest.ty,
                                        location: "usize.to_i64 result must be i64".to_string(),
                                    });
                                    return Err(());
                                }

                                let (operand_id, operand_type) =
                                    lower_value_with_func(ctx, func, &args[0], Type::Usize)?;

                                if operand_type != Type::Usize {
                                    ctx.error(SemanticError::TypeMismatch {
                                        expected: Type::Usize,
                                        found: operand_type,
                                        location: "usize.to_i64 operand".to_string(),
                                    });
                                    return Err(());
                                }

                                Ok(Instruction::Convert {
                                    dest: dest_value_id,
                                    src: operand_id,
                                    from_ty: Type::Usize,
                                    to_ty: Type::I64,
                                })
                            } else {
                                // Unary operation
                                let unary_op = UnaryOperator::from_str(op_part, ty)
                                    .map_err(|e| ctx.error(e))?;

                                let (operand_id, operand_type) =
                                    lower_value_with_func(ctx, func, &args[0], ty)?;

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
                tilt_ast::Expression::Constant(value) => {
                    // Direct constant assignment
                    Ok(Instruction::Const {
                        dest: dest_value_id,
                        value: *value as i64,
                        ty: dest.ty,
                    })
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
        tilt_ast::Instruction::ExpressionStatement { expr } => {
            // Handle expressions used as statements (void expressions)
            match expr {
                tilt_ast::Expression::Call { name, args } => {
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
                                    if let Some((value_id, actual_type)) =
                                        ctx.lookup_variable(var_name)
                                    {
                                        if actual_type != *expected_type {
                                            ctx.error(SemanticError::TypeMismatch {
                                                expected: *expected_type,
                                                found: actual_type,
                                                location: format!(
                                                    "argument to function '{}'",
                                                    name
                                                ),
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
                                    func.constants.insert(
                                        const_value_id,
                                        (*const_val as i64, *expected_type),
                                    );
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
                tilt_ast::Expression::Operation { op, args } => {
                    // Handle operations that return void (e.g., store, free)
                    // Also handle function calls that look like operations

                    // First check if this is a function call disguised as an operation
                    if let Some((param_types, return_type)) = ctx.lookup_function(op).cloned() {
                        // This is a function call - handle it like the Call branch above
                        if return_type != Type::Void {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: Type::Void,
                                found: return_type,
                                location: format!("void call to function '{}'", op),
                            });
                            return Err(());
                        }

                        // Check argument count
                        if args.len() != param_types.len() {
                            ctx.error(SemanticError::ArgumentMismatch {
                                function: op.to_string(),
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
                                    if let Some((value_id, actual_type)) =
                                        ctx.lookup_variable(var_name)
                                    {
                                        if actual_type != *expected_type {
                                            ctx.error(SemanticError::TypeMismatch {
                                                expected: *expected_type,
                                                found: actual_type,
                                                location: format!("argument to function '{}'", op),
                                            });
                                            return Err(());
                                        }
                                        ir_args.push(value_id);
                                    } else {
                                        ctx.error(SemanticError::UndefinedIdentifier {
                                            name: var_name.to_string(),
                                            location: format!("argument to function '{}'", op),
                                        });
                                        return Err(());
                                    }
                                }
                                tilt_ast::Value::Constant(const_val) => {
                                    // Create a constant instruction for this argument
                                    let const_value_id = func.next_value();
                                    func.constants.insert(
                                        const_value_id,
                                        (*const_val as i64, *expected_type),
                                    );
                                    ir_args.push(const_value_id);
                                }
                            }
                        }

                        return Ok(Instruction::CallVoid {
                            function: op.to_string(),
                            args: ir_args,
                        });
                    }

                    // Handle operations that return void (e.g., store, free)
                    if op.ends_with(".store") {
                        if args.len() != 2 {
                            ctx.error(SemanticError::ArgumentMismatch {
                                function: op.to_string(),
                                expected: 2,
                                found: args.len(),
                                location: "store operation".to_string(),
                            });
                            return Err(());
                        }

                        let (ptr_value, ptr_type) =
                            lower_value_with_func(ctx, func, &args[0], Type::Usize)?;

                        if ptr_type != Type::Usize {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: Type::Usize,
                                found: ptr_type,
                                location: format!("first argument to '{}'", op),
                            });
                            return Err(());
                        }

                        let store_type = if *op == "i32.store" {
                            Type::I32
                        } else if *op == "i64.store" {
                            Type::I64
                        } else if *op == "f32.store" {
                            Type::F32
                        } else if *op == "f64.store" {
                            Type::F64
                        } else if *op == "usize.store" {
                            Type::Usize
                        } else {
                            ctx.error(SemanticError::InvalidOperation {
                                operation: op.to_string(),
                                ty: Type::Void,
                                location: "store operation".to_string(),
                            });
                            return Err(());
                        };

                        let (value_id, value_type) =
                            lower_value_with_func(ctx, func, &args[1], store_type)?;

                        if value_type != store_type {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: store_type,
                                found: value_type,
                                location: format!("second argument to '{}'", op),
                            });
                            return Err(());
                        }

                        Ok(Instruction::Store {
                            address: ptr_value,
                            value: value_id,
                            ty: store_type,
                        })
                    } else if *op == "free" {
                        if args.len() != 1 {
                            ctx.error(SemanticError::ArgumentMismatch {
                                function: op.to_string(),
                                expected: 1,
                                found: args.len(),
                                location: "free operation".to_string(),
                            });
                            return Err(());
                        }

                        let (ptr_value, ptr_type) =
                            lower_value_with_func(ctx, func, &args[0], Type::Usize)?;

                        if ptr_type != Type::Usize {
                            ctx.error(SemanticError::TypeMismatch {
                                expected: Type::Usize,
                                found: ptr_type,
                                location: "argument to 'free'".to_string(),
                            });
                            return Err(());
                        }

                        Ok(Instruction::Free { ptr: ptr_value })
                    } else {
                        ctx.error(SemanticError::InvalidOperation {
                            operation: op.to_string(),
                            ty: Type::Void,
                            location: "void operation".to_string(),
                        });
                        Err(())
                    }
                }
                _ => {
                    ctx.error(SemanticError::InvalidOperation {
                        operation: "unknown".to_string(),
                        ty: Type::Void,
                        location: "expression statement".to_string(),
                    });
                    Err(())
                }
            }
        }
    }
}

/// Lower a terminator
fn lower_terminator(
    ctx: &mut LoweringContext,
    func: &mut Function,
    terminator: &tilt_ast::Terminator,
) -> Result<Terminator, ()> {
    match terminator {
        tilt_ast::Terminator::Ret(value_opt) => {
            if let Some(value) = value_opt {
                // Get the expected return type from the current function
                let expected_type = if let Some(current_func) = &ctx.current_function {
                    current_func.return_type
                } else {
                    Type::I32 // Default fallback
                };

                let (value_id, value_type) =
                    lower_value_with_func(ctx, func, value, expected_type)?;

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
        tilt_ast::Terminator::Br { label, args } => {
            if let Some(&target_id) = ctx.block_map.get(*label) {
                // Lower block arguments
                let mut lowered_args = Vec::new();
                for arg in args {
                    let (arg_id, _) = lower_value_with_func(ctx, func, arg, Type::I32)?; // TODO: infer proper type
                    lowered_args.push(arg_id);
                }
                Ok(Terminator::Br {
                    target: target_id,
                    args: lowered_args,
                })
            } else {
                ctx.error(SemanticError::UndefinedBlock {
                    label: label.to_string(),
                    location: "branch target".to_string(),
                });
                Err(())
            }
        }
        tilt_ast::Terminator::BrIf {
            cond,
            true_label,
            true_args,
            false_label,
            false_args,
        } => {
            let (cond_id, cond_type) = lower_value_with_func(ctx, func, cond, Type::I32)?;

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

            // Lower true branch arguments
            let mut lowered_true_args = Vec::new();
            for arg in true_args {
                let (arg_id, _) = lower_value_with_func(ctx, func, arg, Type::I32)?; // TODO: infer proper type
                lowered_true_args.push(arg_id);
            }

            // Lower false branch arguments
            let mut lowered_false_args = Vec::new();
            for arg in false_args {
                let (arg_id, _) = lower_value_with_func(ctx, func, arg, Type::I32)?; // TODO: infer proper type
                lowered_false_args.push(arg_id);
            }

            Ok(Terminator::BrIf {
                cond: cond_id,
                true_target,
                true_args: lowered_true_args,
                false_target,
                false_args: lowered_false_args,
            })
        }
    }
}

/// Lower a value (variable reference or constant)
#[allow(dead_code)]
fn lower_value(ctx: &mut LoweringContext, value: &tilt_ast::Value) -> Result<(ValueId, Type), ()> {
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

fn lower_value_with_func(
    ctx: &mut LoweringContext,
    func: &mut Function,
    value: &tilt_ast::Value,
    expected_type: Type,
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
        tilt_ast::Value::Constant(const_val) => {
            // Create a constant instruction for this value
            let const_value_id = func.next_value();
            func.constants
                .insert(const_value_id, (*const_val as i64, expected_type));
            Ok((const_value_id, expected_type))
        }
    }
}

/// Handle conversion operations like i32.to_usize, i64.to_i32, etc.
fn handle_conversion_operation(
    ctx: &mut LoweringContext,
    func: &mut Function,
    dest: &tilt_ast::TypedIdentifier,
    dest_value_id: ValueId,
    op: &str,
    type_part: &str,
    op_part: &str,
    args: &[tilt_ast::Value],
) -> Result<Instruction, ()> {
    if args.len() != 1 {
        ctx.error(SemanticError::InvalidOperation {
            operation: format!("{} with {} arguments (expected 1)", op, args.len()),
            ty: dest.ty,
            location: "conversion operation".to_string(),
        });
        return Err(());
    }

    // Parse the conversion: type_part.to_TARGET -> (source_type, target_type)
    let (source_type, target_type) = match (type_part, op_part) {
        ("i32", "to_i64") => (Type::I32, Type::I64),
        ("i32", "to_usize") => (Type::I32, Type::Usize),
        ("i64", "to_i32") => (Type::I64, Type::I32),
        ("i64", "to_usize") => (Type::I64, Type::Usize),
        ("usize", "to_i64") => (Type::Usize, Type::I64),
        ("usize", "to_i32") => (Type::Usize, Type::I32),
        _ => {
            ctx.error(SemanticError::InvalidOperation {
                operation: op.to_string(),
                ty: dest.ty,
                location: "unsupported conversion".to_string(),
            });
            return Err(());
        }
    };

    // Check that destination type matches the target type
    if dest.ty != target_type {
        ctx.error(SemanticError::TypeMismatch {
            expected: target_type,
            found: dest.ty,
            location: format!(
                "conversion '{}': result type should be {:?} to match destination",
                op, target_type
            ),
        });
        return Err(());
    }

    // Lower the operand with the expected source type
    let (operand_id, operand_type) = lower_value_with_func(ctx, func, &args[0], source_type)?;

    // Check that operand type matches the source type
    if operand_type != source_type {
        ctx.error(SemanticError::TypeMismatch {
            expected: source_type,
            found: operand_type,
            location: format!(
                "conversion '{}': operand expected {:?} but got {:?}",
                op, source_type, operand_type
            ),
        });
        return Err(());
    }

    Ok(Instruction::Convert {
        dest: dest_value_id,
        src: operand_id,
        from_ty: source_type,
        to_ty: target_type,
    })
}
