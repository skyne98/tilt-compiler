// ===================================================================
// FILE: lib.rs (tilt-ir-builder crate)
//
// DESC: Programmatic IR Builder API for TILT. Provides a safe, typed,
//       and efficient Rust API to construct TILT IR directly without
//       going through the parser.
// ===================================================================

use tilt_ast::Type;
use tilt_ir::*;

/// Builder for constructing TILT IR programmatically.
/// This provides a much more ergonomic API than manually constructing IR nodes.
pub struct FunctionBuilder<'a> {
    /// The function IR we are building
    func: &'a mut Function,

    /// The current block we are inserting instructions into
    current_block: Option<usize>,
}

impl<'a> FunctionBuilder<'a> {
    /// Create a new FunctionBuilder for the given function
    pub fn new(func: &'a mut Function) -> Self {
        Self {
            func,
            current_block: None,
        }
    }

    /// Switch to inserting instructions into the given block
    pub fn switch_to_block(&mut self, block: BlockId) {
        let block_index = block.index();
        assert!(block_index < self.func.blocks.len(), "Block does not exist");
        self.current_block = Some(block_index);
    }

    /// Get the current block being built
    pub fn current_block(&self) -> Option<BlockId> {
        self.current_block.map(BlockId::new)
    }

    /// Create a new basic block and add it to the function
    pub fn create_block(&mut self, label: &str) -> BlockId {
        let block_id = BlockId::new(self.func.blocks.len());
        let block = BasicBlock::new(block_id, label.to_string());
        self.func.blocks.push(block);
        block_id
    }

    /// Add a parameter to the given block and return its ValueId
    pub fn add_block_param(&mut self, block: BlockId, ty: Type) -> ValueId {
        let value_id = self.func.next_value();

        // Add the parameter to the block
        let block_index = block.index();
        if let Some(block_data) = self.func.blocks.get_mut(block_index) {
            block_data.params.push((value_id, ty));
        }

        value_id
    }

    /// Get an instruction builder for fluent API
    pub fn ins(&mut self) -> InstructionBuilder<'_, 'a> {
        InstructionBuilder { builder: self }
    }

    /// Add an instruction to the current block
    fn add_instruction(&mut self, instr: Instruction) -> ValueId {
        let current_block = self
            .current_block
            .expect("No current block - call switch_to_block first");

        // Extract the destination ValueId from the instruction
        let dest = match &instr {
            Instruction::Call { dest, .. } => *dest,
            Instruction::Const { dest, .. } => *dest,
            Instruction::BinaryOp { dest, .. } => *dest,
            Instruction::UnaryOp { dest, .. } => *dest,
            Instruction::Load { dest, .. } => *dest,
            Instruction::PtrAdd { dest, .. } => *dest,
            Instruction::SizeOf { dest, .. } => *dest,
            Instruction::Alloc { dest, .. } => *dest,
            Instruction::Convert { dest, .. } => *dest,
            Instruction::CallVoid { .. } | Instruction::Store { .. } | Instruction::Free { .. } => {
                // These instructions don't produce values
                ValueId::new(0) // This shouldn't be used
            }
        };

        if let Some(block) = self.func.blocks.get_mut(current_block) {
            block.instructions.push(instr);
        }

        dest
    }

    /// Set the terminator for the current block
    pub fn set_terminator(&mut self, terminator: Terminator) {
        let current_block = self
            .current_block
            .expect("No current block - call switch_to_block first");

        if let Some(block) = self.func.blocks.get_mut(current_block) {
            block.terminator = terminator;
        }
    }
}

/// Fluent API for building instructions
pub struct InstructionBuilder<'a, 'b> {
    builder: &'a mut FunctionBuilder<'b>,
}

impl<'a, 'b> InstructionBuilder<'a, 'b> {
    /// Build a function call instruction
    pub fn call(&mut self, func_name: &str, args: Vec<ValueId>, return_type: Type) -> ValueId {
        let dest = self.builder.func.next_value();
        let instr = Instruction::Call {
            dest,
            function: func_name.to_string(),
            args,
            return_type,
        };
        self.builder.add_instruction(instr);
        dest
    }

    /// Build a void function call instruction
    pub fn call_void(&mut self, func_name: &str, args: Vec<ValueId>) {
        let instr = Instruction::CallVoid {
            function: func_name.to_string(),
            args,
        };
        self.builder.add_instruction(instr);
    }

    /// Build a constant instruction
    pub fn const_i32(&mut self, value: i32) -> ValueId {
        let dest = self.builder.func.next_value();
        let instr = Instruction::Const {
            dest,
            value: value as i64,
            ty: Type::I32,
        };

        // Also add to constants map
        self.builder
            .func
            .constants
            .insert(dest, (value as i64, Type::I32));

        self.builder.add_instruction(instr);
        dest
    }

    /// Build a constant instruction
    pub fn const_i64(&mut self, value: i64) -> ValueId {
        let dest = self.builder.func.next_value();
        let instr = Instruction::Const {
            dest,
            value,
            ty: Type::I64,
        };

        // Also add to constants map
        self.builder.func.constants.insert(dest, (value, Type::I64));

        self.builder.add_instruction(instr);
        dest
    }

    /// Build a constant instruction for usize
    pub fn const_usize(&mut self, value: usize) -> ValueId {
        let dest = self.builder.func.next_value();
        let instr = Instruction::Const {
            dest,
            value: value as i64,
            ty: Type::Usize,
        };

        // Also add to constants map
        self.builder
            .func
            .constants
            .insert(dest, (value as i64, Type::Usize));

        self.builder.add_instruction(instr);
        dest
    }

    /// Build a binary operation instruction
    pub fn binary_op(
        &mut self,
        op: BinaryOperator,
        ty: Type,
        lhs: ValueId,
        rhs: ValueId,
    ) -> ValueId {
        let dest = self.builder.func.next_value();
        let instr = Instruction::BinaryOp {
            dest,
            op,
            ty,
            lhs,
            rhs,
        };
        self.builder.add_instruction(instr);
        dest
    }

    /// Build an add instruction
    pub fn add(&mut self, ty: Type, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.binary_op(BinaryOperator::Add, ty, lhs, rhs)
    }

    /// Build a subtract instruction
    pub fn sub(&mut self, ty: Type, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.binary_op(BinaryOperator::Sub, ty, lhs, rhs)
    }

    /// Build a multiply instruction
    pub fn mul(&mut self, ty: Type, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.binary_op(BinaryOperator::Mul, ty, lhs, rhs)
    }

    /// Build a comparison instruction
    pub fn cmp_eq(&mut self, ty: Type, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.binary_op(BinaryOperator::Eq, ty, lhs, rhs)
    }

    /// Build a less-than instruction
    pub fn cmp_lt(&mut self, ty: Type, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.binary_op(BinaryOperator::Lt, ty, lhs, rhs)
    }

    /// Build a return instruction
    pub fn ret(&mut self, value: Option<ValueId>) {
        let terminator = Terminator::Ret { value };
        self.builder.set_terminator(terminator);
    }

    /// Build a conditional branch instruction
    pub fn br_if(&mut self, condition: ValueId, then_block: BlockId, else_block: BlockId) {
        let terminator = Terminator::BrIf {
            cond: condition,
            true_target: then_block,
            true_args: vec![],
            false_target: else_block,
            false_args: vec![],
        };
        self.builder.set_terminator(terminator);
    }

    /// Build an unconditional jump instruction
    pub fn jump(&mut self, target: BlockId) {
        let terminator = Terminator::Br {
            target,
            args: vec![],
        };
        self.builder.set_terminator(terminator);
    }

    /// Build a pointer addition instruction
    pub fn ptr_add(&mut self, ptr: ValueId, offset: ValueId) -> ValueId {
        let dest = self.builder.func.next_value();
        let instr = Instruction::PtrAdd { dest, ptr, offset };
        self.builder.add_instruction(instr);
        dest
    }

    /// Build a sizeof instruction
    pub fn size_of(&mut self, ty: Type) -> ValueId {
        let dest = self.builder.func.next_value();
        let instr = Instruction::SizeOf { dest, ty };
        self.builder.add_instruction(instr);
        dest
    }

    /// Build an allocation instruction
    pub fn alloc(&mut self, size: ValueId) -> ValueId {
        let dest = self.builder.func.next_value();
        let instr = Instruction::Alloc { dest, size };
        self.builder.add_instruction(instr);
        dest
    }

    /// Build a free instruction
    pub fn free(&mut self, ptr: ValueId) {
        let instr = Instruction::Free { ptr };
        self.builder.add_instruction(instr);
    }

    /// Build a memory load instruction
    pub fn load(&mut self, ty: Type, address: ValueId) -> ValueId {
        let dest = self.builder.func.next_value();
        let instr = Instruction::Load { dest, ty, address };
        self.builder.add_instruction(instr);
        dest
    }

    /// Build a memory store instruction
    pub fn store(&mut self, address: ValueId, value: ValueId, ty: Type) {
        let instr = Instruction::Store { address, value, ty };
        self.builder.add_instruction(instr);
    }
}

/// Builder for constructing entire programs
pub struct ProgramBuilder {
    /// The program being built
    program: Program,
}

impl ProgramBuilder {
    /// Create a new program builder
    pub fn new() -> Self {
        Self {
            program: Program {
                imports: Vec::new(),
                functions: Vec::new(),
            },
        }
    }

    /// Add an import to the program
    pub fn add_import(&mut self, module: &str, name: &str, params: Vec<Type>, return_type: Type) {
        self.program.imports.push(ImportDecl {
            module: module.to_string(),
            name: name.to_string(),
            calling_convention: None,
            params,
            return_type,
        });
    }

    /// Add an import with calling convention to the program
    pub fn add_import_with_cc(
        &mut self,
        module: &str,
        name: &str,
        calling_convention: Option<String>,
        params: Vec<Type>,
        return_type: Type,
    ) {
        self.program.imports.push(ImportDecl {
            module: module.to_string(),
            name: name.to_string(),
            calling_convention,
            params,
            return_type,
        });
    }

    /// Create a new function and return a builder for it
    pub fn create_function(&mut self, name: &str, params: Vec<Type>, return_type: Type) -> usize {
        let func = Function::new(name.to_string(), params, return_type);
        self.program.functions.push(func);
        self.program.functions.len() - 1 // Return index for later use
    }

    /// Get a builder for an existing function
    pub fn function_builder(&mut self, function_index: usize) -> FunctionBuilder {
        let func = &mut self.program.functions[function_index];
        FunctionBuilder::new(func)
    }

    /// Finalize and return the built program
    pub fn build(self) -> Program {
        self.program
    }
}

impl Default for ProgramBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_function_construction() {
        let mut builder = ProgramBuilder::new();

        // Add imports
        builder.add_import("env", "print_char", vec![Type::I32], Type::Void);

        assert_eq!(builder.program.imports.len(), 1);
        assert_eq!(builder.program.imports[0].name, "print_char");
    }

    #[test]
    fn test_value_id_generation() {
        let mut func = Function::new("test".to_string(), vec![], Type::Void);
        let mut builder = FunctionBuilder::new(&mut func);

        // Create a block first
        let block = builder.create_block("entry");
        builder.switch_to_block(block);

        // Test that we can generate unique value IDs through constant creation
        let val1 = builder.ins().const_i32(42);
        let val2 = builder.ins().const_i32(43);

        assert_ne!(val1, val2);
    }

    #[test]
    fn test_block_creation() {
        let mut func = Function::new("test".to_string(), vec![], Type::Void);
        let mut builder = FunctionBuilder::new(&mut func);

        // Test that we can create blocks
        let block1 = builder.create_block("entry");
        let block2 = builder.create_block("exit");

        assert_ne!(block1, block2);
        assert_eq!(builder.func.blocks.len(), 2);
    }

    #[test]
    fn test_simple_arithmetic() {
        let mut func = Function::new(
            "add_test".to_string(),
            vec![Type::I32, Type::I32],
            Type::I32,
        );
        let mut builder = FunctionBuilder::new(&mut func);

        // Create entry block
        let entry = builder.create_block("entry");
        builder.switch_to_block(entry);

        // Add block parameters for function arguments
        let param1 = builder.add_block_param(entry, Type::I32);
        let param2 = builder.add_block_param(entry, Type::I32);

        // Build: result = add param1, param2
        let result = builder.ins().add(Type::I32, param1, param2);

        // Return the result
        builder.ins().ret(Some(result));

        // Check that everything was built correctly
        assert_eq!(builder.func.blocks.len(), 1);
        assert_eq!(builder.func.blocks[0].instructions.len(), 1); // add instruction
        assert_eq!(builder.func.blocks[0].params.len(), 2); // two parameters
    }

    #[test]
    fn test_conditional_logic() {
        let mut func = Function::new("max".to_string(), vec![Type::I32, Type::I32], Type::I32);
        let mut builder = FunctionBuilder::new(&mut func);

        // Create blocks
        let entry = builder.create_block("entry");
        let then_block = builder.create_block("then");
        let else_block = builder.create_block("else");
        let exit = builder.create_block("exit");

        // Entry block: compare the arguments
        builder.switch_to_block(entry);
        let param1 = builder.add_block_param(entry, Type::I32);
        let param2 = builder.add_block_param(entry, Type::I32);
        let cmp = builder.ins().cmp_lt(Type::I32, param1, param2);
        builder.ins().br_if(cmp, then_block, else_block);

        // Then block: return param2
        builder.switch_to_block(then_block);
        builder.ins().jump(exit);

        // Else block: return param1
        builder.switch_to_block(else_block);
        builder.ins().jump(exit);

        // Exit block: return (for now, just return a constant)
        builder.switch_to_block(exit);
        let result = builder.ins().const_i32(0);
        builder.ins().ret(Some(result));

        // Validate the structure
        assert_eq!(builder.func.blocks.len(), 4);
    }
}
