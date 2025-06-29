use cranelift::codegen::ir::BlockArg;
use cranelift::prelude::*;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use std::collections::HashMap;
use tilt_ast::Type as IRType;
use tilt_host_abi::{HostABI, JITMemoryHostABI};
use tilt_ir::{
    BinaryOperator, BlockId, Function as IRFunction, Instruction, Program, Terminator,
    UnaryOperator, ValueId,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod memory_jit_tests;

pub struct JIT {
    /// The main JIT module, which manages function compilation and linking.
    module: JITModule,
    /// Function IDs for imports and declared functions
    function_ids: HashMap<String, FuncId>,
    /// Whether to show Cranelift IR during compilation
    show_cranelift_ir: bool,
    /// Host ABI for handling host function calls
    host_abi: Box<dyn HostABI + Send + Sync>,
}

impl JIT {
    pub fn new() -> Result<Self, String> {
        Self::new_with_abi(Box::new(JITMemoryHostABI::new()))
    }

    pub fn new_with_abi(host_abi: Box<dyn HostABI + Send + Sync>) -> Result<Self, String> {
        // Create a JIT builder. We register dynamic host functions here.
        let mut builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .map_err(|e| format!("Failed to create JIT builder: {}", e))?;

        // Register host functions that will dynamically dispatch to the Host ABI
        builder.symbol("print_hello", host_print_hello as *const u8);
        builder.symbol("print_char", host_print_char as *const u8);
        builder.symbol("print_i32", host_print_i32 as *const u8);
        builder.symbol("print_i64", host_print_i64 as *const u8);
        builder.symbol("println", host_println as *const u8);
        builder.symbol("read_i32", host_read_i32 as *const u8);
        builder.symbol("alloc", host_alloc as *const u8);
        builder.symbol("free", host_free as *const u8);

        // Create the JIT module.
        let module = JITModule::new(builder);

        Ok(Self {
            module,
            function_ids: HashMap::new(),
            show_cranelift_ir: false,
            host_abi,
        })
    }

    /// Compile a TILT IR program into executable code in memory.
    pub fn compile(&mut self, program: &Program) -> Result<(), String> {
        // First pass: Declare all functions (both imports and local functions)
        for import in &program.imports {
            let mut sig = self.module.make_signature();

            // Add parameters
            for param_type in &import.params {
                sig.params.push(AbiParam::new(translate_type(param_type)));
            }

            // Add return type
            if import.return_type != IRType::Void {
                sig.returns
                    .push(AbiParam::new(translate_type(&import.return_type)));
            }

            let func_id = self
                .module
                .declare_function(&import.name, Linkage::Import, &sig)
                .map_err(|e| format!("Failed to declare import '{}': {}", import.name, e))?;

            self.function_ids.insert(import.name.clone(), func_id);
        }

        for function in &program.functions {
            let mut sig = self.module.make_signature();

            // Add parameters
            for param_type in &function.params {
                sig.params.push(AbiParam::new(translate_type(param_type)));
            }

            // Add return type
            if function.return_type != IRType::Void {
                sig.returns
                    .push(AbiParam::new(translate_type(&function.return_type)));
            }

            let func_id = self
                .module
                .declare_function(&function.name, Linkage::Export, &sig)
                .map_err(|e| format!("Failed to declare function '{}': {}", function.name, e))?;

            self.function_ids.insert(function.name.clone(), func_id);
        }

        // Second pass: Compile function bodies
        for function in &program.functions {
            self.translate_function(function)?;
        }

        // Finalize all functions, which resolves any forward-declared calls.
        self.module
            .finalize_definitions()
            .map_err(|e| format!("Failed to finalize definitions: {}", e))?;

        Ok(())
    }

    /// Get a raw pointer to a compiled function.
    pub fn get_func_ptr(&mut self, func_name: &str) -> Option<*const u8> {
        let func_id = *self.function_ids.get(func_name)?;
        Some(self.module.get_finalized_function(func_id))
    }

    /// Enable or disable Cranelift IR output during compilation
    pub fn set_show_cranelift_ir(&mut self, show: bool) {
        self.show_cranelift_ir = show;
    }

    fn translate_function(&mut self, func: &IRFunction) -> Result<(), String> {
        let func_id = self
            .function_ids
            .get(&func.name)
            .ok_or_else(|| format!("Function '{}' not declared", func.name))?;

        // The context holds information about the current function being compiled.
        let mut ctx = self.module.make_context();

        // Get the function signature that was already declared
        ctx.func.signature = self
            .module
            .declarations()
            .get_function_decl(*func_id)
            .signature
            .clone();

        // Create a FunctionBuilder context.
        let mut builder_ctx = FunctionBuilderContext::new();
        let builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

        // Create and run the translator.
        let mut translator = Translator {
            builder,
            tilt_func: func,
            module: &mut self.module,
            function_ids: &self.function_ids,
            block_map: HashMap::new(),
            value_map: HashMap::new(),
        };
        translator.translate()?;

        // Show Cranelift IR if requested
        if self.show_cranelift_ir {
            println!("🔧 Cranelift IR for function '{}':", func.name);
            println!("{}", ctx.func.display());
            println!();
        }

        // Define the function body.
        self.module
            .define_function(*func_id, &mut ctx)
            .map_err(|e| format!("Failed to define function '{}': {}", func.name, e))?;

        // Clear the context for the next function.
        self.module.clear_context(&mut ctx);

        Ok(())
    }
}

struct Translator<'a> {
    // The Cranelift function builder.
    builder: FunctionBuilder<'a>,
    // The TILT function we are translating.
    tilt_func: &'a IRFunction,
    // The Cranelift module, needed to declare functions.
    module: &'a mut JITModule,
    // Function IDs for calling other functions
    function_ids: &'a HashMap<String, FuncId>,

    // MAPPINGS: The key to the whole process!
    // Maps our block IDs to Cranelift's block objects.
    block_map: HashMap<BlockId, Block>,
    // Maps our SSA value IDs to Cranelift's SSA value objects.
    value_map: HashMap<ValueId, Value>,
}

impl<'a> Translator<'a> {
    fn translate(&mut self) -> Result<(), String> {
        // 1. Create all Cranelift blocks first. This is crucial for handling
        //    forward branches.
        for block in &self.tilt_func.blocks {
            let cl_block = self.builder.create_block();
            self.block_map.insert(block.id, cl_block);

            // Handle block parameters (phi nodes)
            for (value_id, param_type) in &block.params {
                let cl_type = translate_type(param_type);
                let cl_value = self.builder.append_block_param(cl_block, cl_type);
                self.value_map.insert(*value_id, cl_value);
            }
        }

        // 2. Handle function parameters: check if they're already in entry block, if not add them
        let entry_block = self
            .block_map
            .get(&self.tilt_func.entry_block)
            .ok_or("Entry block not found")?;

        // Check if the entry block already has function parameters
        let entry_block_data = self
            .tilt_func
            .blocks
            .iter()
            .find(|b| b.id == self.tilt_func.entry_block)
            .ok_or("Entry block data not found")?;

        // If the entry block doesn't have parameters matching the function signature,
        // add them as block parameters (for manually constructed IR)
        if entry_block_data.params.len() != self.tilt_func.params.len() {
            for (i, param_type) in self.tilt_func.params.iter().enumerate() {
                let cl_type = translate_type(param_type);
                let cl_value = self.builder.append_block_param(*entry_block, cl_type);
                self.value_map.insert(ValueId::new(i), cl_value);
            }
        }

        self.builder.switch_to_block(*entry_block);

        // 3. Now, iterate and translate the contents of every block.
        for block in &self.tilt_func.blocks {
            let cl_block = self.block_map[&block.id];
            self.builder.switch_to_block(cl_block);

            // Translate each instruction.
            for instr in &block.instructions {
                self.translate_instruction(instr)?;
            }

            // Translate the terminator.
            self.translate_terminator(&block.terminator)?;
        }

        // 4. Finalize the function body.
        self.builder.seal_all_blocks();
        Ok(())
    }

    fn translate_instruction(&mut self, instr: &Instruction) -> Result<(), String> {
        match instr {
            Instruction::Call {
                dest,
                function,
                args,
                return_type: _,
            } => {
                let func_id = self
                    .function_ids
                    .get(function)
                    .ok_or_else(|| format!("Function '{}' not found", function))?;

                let func_ref = self
                    .module
                    .declare_func_in_func(*func_id, self.builder.func);

                let cl_args: Vec<Value> = args
                    .iter()
                    .map(|arg| self.get_value_or_constant(*arg))
                    .collect::<Result<Vec<_>, _>>()?;

                let call_result = self.builder.ins().call(func_ref, &cl_args);
                let results = self.builder.inst_results(call_result);

                if !results.is_empty() {
                    self.value_map.insert(*dest, results[0]);
                }

                Ok(())
            }
            Instruction::CallVoid { function, args } => {
                let func_id = self
                    .function_ids
                    .get(function)
                    .ok_or_else(|| format!("Function '{}' not found", function))?;

                let func_ref = self
                    .module
                    .declare_func_in_func(*func_id, self.builder.func);

                let cl_args: Vec<Value> = args
                    .iter()
                    .map(|arg| self.get_value_or_constant(*arg))
                    .collect::<Result<Vec<_>, _>>()?;

                self.builder.ins().call(func_ref, &cl_args);
                Ok(())
            }
            Instruction::BinaryOp {
                dest,
                op,
                ty,
                lhs,
                rhs,
            } => {
                let lhs_val = self.get_value_or_constant(*lhs)?;
                let rhs_val = self.get_value_or_constant(*rhs)?;

                let result = match op {
                    BinaryOperator::Add => self.builder.ins().iadd(lhs_val, rhs_val),
                    BinaryOperator::Sub => self.builder.ins().isub(lhs_val, rhs_val),
                    BinaryOperator::Mul => self.builder.ins().imul(lhs_val, rhs_val),
                    BinaryOperator::Div => match ty {
                        IRType::I32 | IRType::I64 => self.builder.ins().sdiv(lhs_val, rhs_val),
                        IRType::F32 | IRType::F64 => self.builder.ins().fdiv(lhs_val, rhs_val),
                        _ => return Err(format!("Division not supported for type {:?}", ty)),
                    },
                    BinaryOperator::Eq => {
                        let cmp_result = self.builder.ins().icmp(IntCC::Equal, lhs_val, rhs_val);
                        self.builder.ins().uextend(types::I32, cmp_result)
                    }
                    BinaryOperator::Ne => {
                        let cmp_result = self.builder.ins().icmp(IntCC::NotEqual, lhs_val, rhs_val);
                        self.builder.ins().uextend(types::I32, cmp_result)
                    }
                    BinaryOperator::Lt => {
                        let cmp_result =
                            self.builder
                                .ins()
                                .icmp(IntCC::SignedLessThan, lhs_val, rhs_val);
                        self.builder.ins().uextend(types::I32, cmp_result)
                    }
                    BinaryOperator::Le => {
                        let cmp_result =
                            self.builder
                                .ins()
                                .icmp(IntCC::SignedLessThanOrEqual, lhs_val, rhs_val);
                        self.builder.ins().uextend(types::I32, cmp_result)
                    }
                    BinaryOperator::Gt => {
                        let cmp_result =
                            self.builder
                                .ins()
                                .icmp(IntCC::SignedGreaterThan, lhs_val, rhs_val);
                        self.builder.ins().uextend(types::I32, cmp_result)
                    }
                    BinaryOperator::Ge => {
                        let cmp_result = self.builder.ins().icmp(
                            IntCC::SignedGreaterThanOrEqual,
                            lhs_val,
                            rhs_val,
                        );
                        self.builder.ins().uextend(types::I32, cmp_result)
                    }
                    _ => return Err(format!("Binary operator {:?} not implemented", op)),
                };

                self.value_map.insert(*dest, result);
                Ok(())
            }
            Instruction::UnaryOp {
                dest,
                op,
                ty: _,
                operand,
            } => {
                let operand_val = self.get_value_or_constant(*operand)?;

                let result = match op {
                    UnaryOperator::Neg => self.builder.ins().ineg(operand_val),
                    UnaryOperator::Not => self.builder.ins().bnot(operand_val),
                };

                self.value_map.insert(*dest, result);
                Ok(())
            }
            Instruction::Const { dest, value, ty } => {
                let cl_value = match ty {
                    IRType::I32 => self.builder.ins().iconst(types::I32, *value),
                    IRType::I64 => self.builder.ins().iconst(types::I64, *value),
                    IRType::Usize => {
                        if cfg!(target_pointer_width = "64") {
                            self.builder.ins().iconst(types::I64, *value)
                        } else {
                            self.builder.ins().iconst(types::I32, *value)
                        }
                    }
                    IRType::F32 => self.builder.ins().f32const(*value as f32),
                    IRType::F64 => self.builder.ins().f64const(*value as f64),
                    _ => return Err(format!("Constant type {:?} not supported", ty)),
                };

                self.value_map.insert(*dest, cl_value);
                Ok(())
            }
            Instruction::Store {
                address,
                value,
                ty: _,
            } => {
                let addr_val = self.get_value_or_constant(*address)?;
                let val = self.get_value_or_constant(*value)?;

                // Debug: Print the address being stored to
                // Note: This would require runtime printing which we can't do here
                // Let's add a comment for now and fix the actual issue

                self.builder.ins().store(MemFlags::new(), val, addr_val, 0);
                Ok(())
            }
            Instruction::Load { dest, ty, address } => {
                let addr_val = self.get_value_or_constant(*address)?;

                let cl_type = translate_type(ty);
                let result = self
                    .builder
                    .ins()
                    .load(cl_type, MemFlags::new(), addr_val, 0);
                self.value_map.insert(*dest, result);
                Ok(())
            }

            Instruction::PtrAdd { dest, ptr, offset } => {
                let ptr_val = self.get_value_or_constant(*ptr)?;
                let offset_val = self.get_value_or_constant(*offset)?;

                let result = self.builder.ins().iadd(ptr_val, offset_val);
                self.value_map.insert(*dest, result);
                Ok(())
            }

            Instruction::SizeOf { dest, ty } => {
                let size = match ty {
                    IRType::I32 => 4,
                    IRType::I64 => 8,
                    IRType::F32 => 4,
                    IRType::F64 => 8,
                    IRType::Usize => std::mem::size_of::<usize>() as i64, // Platform-dependent
                    IRType::Void => 0,
                };

                // SizeOf returns usize, so we use the appropriate type based on platform
                let size_val = if cfg!(target_pointer_width = "64") {
                    self.builder.ins().iconst(types::I64, size)
                } else {
                    self.builder.ins().iconst(types::I32, size)
                };
                self.value_map.insert(*dest, size_val);
                Ok(())
            }

            Instruction::Alloc { dest, size } => {
                // Call the host ABI alloc function
                let size_val = self.get_value_or_constant(*size)?;

                // For now, we'll call a host function - this requires the alloc function to be registered
                let alloc_func_id = self
                    .function_ids
                    .get("alloc")
                    .copied()
                    .ok_or_else(|| "alloc function not found".to_string())?;

                let alloc_func_ref = self
                    .module
                    .declare_func_in_func(alloc_func_id, &mut self.builder.func);
                let call_result = self.builder.ins().call(alloc_func_ref, &[size_val]);
                let result_val = self.builder.inst_results(call_result)[0];

                self.value_map.insert(*dest, result_val);
                Ok(())
            }

            Instruction::Free { ptr } => {
                // Call the host ABI free function
                let ptr_val = self.get_value_or_constant(*ptr)?;

                let free_func_id = self
                    .function_ids
                    .get("free")
                    .copied()
                    .ok_or_else(|| "free function not found".to_string())?;

                let free_func_ref = self
                    .module
                    .declare_func_in_func(free_func_id, &mut self.builder.func);
                self.builder.ins().call(free_func_ref, &[ptr_val]);
                Ok(())
            }

            Instruction::Convert {
                dest,
                src,
                from_ty,
                to_ty,
            } => {
                let src_val = self.get_value_or_constant(*src)?;

                // Perform type conversion using Cranelift instructions
                let result = match (from_ty, to_ty) {
                    (IRType::I32, IRType::I64) => {
                        // Sign-extend i32 to i64
                        self.builder.ins().sextend(types::I64, src_val)
                    }
                    (IRType::I32, IRType::Usize) => {
                        // Sign-extend i32 to usize
                        if cfg!(target_pointer_width = "64") {
                            self.builder.ins().sextend(types::I64, src_val)
                        } else {
                            // On 32-bit platforms, i32 is already usize
                            src_val
                        }
                    }
                    (IRType::I64, IRType::I32) => {
                        // Truncate i64 to i32
                        self.builder.ins().ireduce(types::I32, src_val)
                    }
                    (IRType::Usize, IRType::I64) => {
                        // Convert usize to i64
                        if cfg!(target_pointer_width = "64") {
                            // On 64-bit platforms, usize is already i64, so just return as-is
                            src_val
                        } else {
                            // On 32-bit platforms, sign-extend usize (i32) to i64
                            self.builder.ins().sextend(types::I64, src_val)
                        }
                    }
                    (IRType::Usize, IRType::I32) => {
                        // Convert usize to i32
                        if cfg!(target_pointer_width = "64") {
                            // On 64-bit platforms, truncate usize (i64) to i32
                            self.builder.ins().ireduce(types::I32, src_val)
                        } else {
                            // On 32-bit platforms, usize is already i32, so just return as-is
                            src_val
                        }
                    }
                    (IRType::I64, IRType::Usize) => {
                        // Convert i64 to usize
                        if cfg!(target_pointer_width = "64") {
                            // On 64-bit platforms, usize is i64, so just return as-is
                            src_val
                        } else {
                            // On 32-bit platforms, truncate i64 to usize (i32)
                            self.builder.ins().ireduce(types::I32, src_val)
                        }
                    }
                    _ => {
                        return Err(format!(
                            "Unsupported type conversion from {:?} to {:?}",
                            from_ty, to_ty
                        ));
                    }
                };

                self.value_map.insert(*dest, result);
                Ok(())
            }
        }
    }

    fn translate_terminator(&mut self, term: &Terminator) -> Result<(), String> {
        match term {
            Terminator::Ret { value } => {
                if let Some(val_id) = value {
                    let cl_value = self.get_value_or_constant(*val_id)?;
                    self.builder.ins().return_(&[cl_value]);
                } else {
                    self.builder.ins().return_(&[]);
                }
                Ok(())
            }
            Terminator::Br { target, args } => {
                let cl_target = self
                    .block_map
                    .get(target)
                    .copied()
                    .ok_or_else(|| format!("Branch target {:?} not found", target))?;

                if args.is_empty() {
                    // Simple jump without arguments
                    self.builder.ins().jump(cl_target, &[]);
                } else {
                    // Resolve argument values for block parameters
                    let cl_args: Result<Vec<Value>, String> = args
                        .iter()
                        .map(|arg| self.get_value_or_constant(*arg))
                        .collect();
                    let cl_args = cl_args?;

                    // Convert Values to BlockArgs and jump with block arguments
                    let block_args: Vec<BlockArg> =
                        cl_args.into_iter().map(BlockArg::Value).collect();
                    self.builder.ins().jump(cl_target, block_args.iter());
                }
                Ok(())
            }
            Terminator::BrIf {
                cond,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                let cond_val = self.get_value_or_constant(*cond)?;
                let true_block = self
                    .block_map
                    .get(true_target)
                    .copied()
                    .ok_or_else(|| format!("True target {:?} not found", true_target))?;
                let false_block = self
                    .block_map
                    .get(false_target)
                    .copied()
                    .ok_or_else(|| format!("False target {:?} not found", false_target))?;

                if true_args.is_empty() && false_args.is_empty() {
                    self.builder
                        .ins()
                        .brif(cond_val, true_block, &[], false_block, &[]);
                } else {
                    // Resolve arguments for both branches
                    let true_cl_args: Result<Vec<Value>, String> = true_args
                        .iter()
                        .map(|arg| self.get_value_or_constant(*arg))
                        .collect();
                    let true_cl_args = true_cl_args?;

                    let false_cl_args: Result<Vec<Value>, String> = false_args
                        .iter()
                        .map(|arg| self.get_value_or_constant(*arg))
                        .collect();
                    let false_cl_args = false_cl_args?;

                    // Convert Values to BlockArgs
                    let true_block_args: Vec<BlockArg> =
                        true_cl_args.into_iter().map(BlockArg::Value).collect();
                    let false_block_args: Vec<BlockArg> =
                        false_cl_args.into_iter().map(BlockArg::Value).collect();

                    // Conditional branch with block arguments
                    self.builder.ins().brif(
                        cond_val,
                        true_block,
                        true_block_args.iter(),
                        false_block,
                        false_block_args.iter(),
                    );
                }
                Ok(())
            }
        }
    }

    /// Get or create a constant value (always create fresh to respect SSA)
    fn get_constant(&mut self, value: i64, ty: &IRType) -> Value {
        // Always create a fresh constant in the current block to respect SSA form
        match ty {
            IRType::I32 => self.builder.ins().iconst(types::I32, value),
            IRType::I64 => self.builder.ins().iconst(types::I64, value),
            IRType::Usize => {
                if cfg!(target_pointer_width = "64") {
                    self.builder.ins().iconst(types::I64, value)
                } else {
                    self.builder.ins().iconst(types::I32, value)
                }
            }
            IRType::F32 => self.builder.ins().f32const(value as f32),
            IRType::F64 => self.builder.ins().f64const(value as f64),
            IRType::Void => self.builder.ins().iconst(types::I8, 0), // Placeholder
        }
    }

    /// Get a value, either from the value map or create a constant
    fn get_value_or_constant(&mut self, value_id: ValueId) -> Result<Value, String> {
        // First check if it's a regular value
        if let Some(&cl_value) = self.value_map.get(&value_id) {
            return Ok(cl_value);
        }

        // Check if it's a constant
        if let Some(&(const_val, ref const_type)) = self.tilt_func.constants.get(&value_id) {
            return Ok(self.get_constant(const_val, const_type));
        }

        Err(format!("Value {:?} not found", value_id))
    }
}

fn translate_type(ir_type: &IRType) -> types::Type {
    match ir_type {
        IRType::I32 => types::I32,
        IRType::I64 => types::I64,
        IRType::F32 => types::F32,
        IRType::F64 => types::F64,
        IRType::Usize => {
            // Use the native pointer size for the target platform
            if cfg!(target_pointer_width = "64") {
                types::I64
            } else {
                types::I32
            }
        }
        IRType::Void => types::I8, // Placeholder, void functions return nothing
    }
}

// Host function implementations
// For now, these are simple implementations that don't use the dynamic ABI
// In the future, we could implement proper per-instance ABI support

fn host_print_hello() {
    print!("Hello from JIT!");
}

fn host_print_char(c: i32) {
    if let Some(ch) = char::from_u32(c as u32) {
        print!("{}", ch);
    }
}

fn host_print_i32(value: i32) {
    print!("{}", value);
}

fn host_print_i64(value: i64) {
    print!("{}", value);
}

fn host_println() {
    println!();
}

fn host_read_i32() -> i32 {
    use std::io::{self, Write};
    print!("Enter i32: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().parse().unwrap_or(0)
}

#[cfg(target_pointer_width = "64")]
fn host_alloc(size: u64) -> u64 {
    use std::alloc::{alloc, Layout};
    if size == 0 {
        return 0;
    }

    let layout = Layout::from_size_align(size as usize, 8).unwrap();
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        0
    } else {
        ptr as u64
    }
}

#[cfg(target_pointer_width = "32")]
fn host_alloc(size: u32) -> u32 {
    use std::alloc::{alloc, Layout};
    if size == 0 {
        return 0;
    }

    let layout = Layout::from_size_align(size as usize, 4).unwrap();
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        0
    } else {
        ptr as u32
    }
}

#[cfg(target_pointer_width = "64")]
fn host_free(ptr: u64) {
    if ptr != 0 {
        // Note: This is unsafe because we don't know the original size
        // In a real implementation, we'd need to track allocations
        // For now, we'll just leak memory to avoid crashes
        // TODO: Implement proper allocation tracking
    }
}

#[cfg(target_pointer_width = "32")]
fn host_free(ptr: u32) {
    if ptr != 0 {
        // Note: This is unsafe because we don't know the original size
        // In a real implementation, we'd need to track allocations
        // For now, we'll just leak memory to avoid crashes
        // TODO: Implement proper allocation tracking
    }
}
