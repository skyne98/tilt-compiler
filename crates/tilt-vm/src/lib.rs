// ===================================================================
// FILE: lib.rs (tilt-vm crate)
//
// DESC: Virtual Machine (interpreter) for TILT IR. This provides a
//       portable reference implementation that can execute TILT IR
//       directly without compilation to native code.
// ===================================================================

use std::collections::HashMap;
use tilt_ast::Type;
use tilt_host_abi::{HostABI, RuntimeValue};
use tilt_ir::*;

/// Error types that can occur during VM execution
#[derive(Debug, Clone, PartialEq)]
pub enum VMError {
    /// Function not found
    FunctionNotFound(String),
    /// Block not found
    BlockNotFound(BlockId),
    /// Value not found in the current scope
    ValueNotFound(ValueId),
    /// Type mismatch during operation
    TypeMismatch {
        expected: Type,
        actual: Type,
        context: String,
    },
    /// Division by zero
    DivisionByZero,
    /// Host function call failed
    HostCallError(String),
    /// Stack overflow
    StackOverflow,
    /// Invalid instruction
    InvalidInstruction(String),
}

impl std::fmt::Display for VMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VMError::FunctionNotFound(name) => write!(f, "Function not found: {}", name),
            VMError::BlockNotFound(id) => write!(f, "Block not found: {:?}", id),
            VMError::ValueNotFound(id) => write!(f, "Value not found: {:?}", id),
            VMError::TypeMismatch {
                expected,
                actual,
                context,
            } => {
                write!(
                    f,
                    "Type mismatch in {}: expected {:?}, got {:?}",
                    context, expected, actual
                )
            }
            VMError::DivisionByZero => write!(f, "Division by zero"),
            VMError::HostCallError(msg) => write!(f, "Host call error: {}", msg),
            VMError::StackOverflow => write!(f, "Stack overflow"),
            VMError::InvalidInstruction(msg) => write!(f, "Invalid instruction: {}", msg),
        }
    }
}

impl std::error::Error for VMError {}

/// Result type for VM operations
pub type VMResult<T> = Result<T, VMError>;

/// A stack frame for function calls
#[derive(Debug, Clone)]
struct StackFrame {
    /// The function being executed
    function_name: String,
    /// Values in the current frame (ValueId -> RuntimeValue)
    values: HashMap<ValueId, RuntimeValue>,
    /// Current block being executed
    current_block: BlockId,
    /// Instruction pointer within the current block
    instruction_pointer: usize,
}

impl StackFrame {
    fn new(function_name: String, entry_block: BlockId) -> Self {
        Self {
            function_name,
            values: HashMap::new(),
            current_block: entry_block,
            instruction_pointer: 0,
        }
    }

    fn get_value(&self, value_id: ValueId) -> VMResult<&RuntimeValue> {
        self.values
            .get(&value_id)
            .ok_or(VMError::ValueNotFound(value_id))
    }

    fn set_value(&mut self, value_id: ValueId, value: RuntimeValue) {
        self.values.insert(value_id, value);
    }
}

/// The TILT Virtual Machine
pub struct VM<H: HostABI> {
    /// The program being executed
    program: Program,
    /// Call stack
    call_stack: Vec<StackFrame>,
    /// Host ABI implementation
    host_abi: H,
    /// Maximum call stack depth (to prevent infinite recursion)
    max_stack_depth: usize,
}

impl<H: HostABI> VM<H> {
    /// Create a new VM with the given program and host ABI
    pub fn new(program: Program, host_abi: H) -> Self {
        Self {
            program,
            call_stack: Vec::new(),
            host_abi,
            max_stack_depth: 1000, // Reasonable default
        }
    }

    /// Set the maximum call stack depth
    pub fn set_max_stack_depth(&mut self, depth: usize) {
        self.max_stack_depth = depth;
    }

    /// Execute a function by name with the given arguments
    pub fn call_function(&mut self, name: &str, args: Vec<RuntimeValue>) -> VMResult<RuntimeValue> {
        // Find the function
        let function = self
            .program
            .functions
            .iter()
            .find(|f| f.name == name)
            .ok_or_else(|| VMError::FunctionNotFound(name.to_string()))?;

        // Check argument count
        if args.len() != function.params.len() {
            return Err(VMError::InvalidInstruction(format!(
                "Function {} expects {} arguments, got {}",
                name,
                function.params.len(),
                args.len()
            )));
        }

        // Check stack depth
        if self.call_stack.len() >= self.max_stack_depth {
            return Err(VMError::StackOverflow);
        }

        // Get the entry block (first block)
        let entry_block = function
            .blocks
            .get(0)
            .map(|b| b.id)
            .ok_or_else(|| VMError::InvalidInstruction("Function has no blocks".to_string()))?;

        // Create a new stack frame
        let mut frame = StackFrame::new(name.to_string(), entry_block);

        // Set up function parameters as the first values in the function scope
        // Function parameters become ValueId(0), ValueId(1), etc.
        for (i, (arg_value, param_type)) in args.iter().zip(function.params.iter()).enumerate() {
            // Type check
            if arg_value.get_type() != *param_type {
                return Err(VMError::TypeMismatch {
                    expected: *param_type,
                    actual: arg_value.get_type(),
                    context: format!("function parameter {} in function '{}'", i, name),
                });
            }
            // Parameters start from ValueId(0)
            let param_id = ValueId(i);
            frame.set_value(param_id, arg_value.clone());
        }

        // Set up constants
        for (value_id, (const_value, const_type)) in &function.constants {
            let runtime_value = match const_type {
                Type::I32 => RuntimeValue::I32(*const_value as i32),
                Type::I64 => RuntimeValue::I64(*const_value),
                Type::Usize => RuntimeValue::Usize((*const_value as u64).try_into().unwrap()),
                Type::Void => RuntimeValue::Void,
                Type::F32 | Type::F64 => {
                    return Err(VMError::InvalidInstruction(
                        "Float types not yet supported".to_string(),
                    ));
                }
            };
            frame.set_value(*value_id, runtime_value);
        }

        // Push the frame and execute
        self.call_stack.push(frame);
        let result = self.execute_function();
        self.call_stack.pop();

        result
    }

    /// Execute the function in the current stack frame
    fn execute_function(&mut self) -> VMResult<RuntimeValue> {
        loop {
            // Extract the current state to avoid borrowing issues
            let (function_name, current_block_id, instruction_pointer) = {
                let current_frame = self.call_stack.last().unwrap();
                (
                    current_frame.function_name.clone(),
                    current_frame.current_block,
                    current_frame.instruction_pointer,
                )
            };

            // Find the function and block
            let function = self
                .program
                .functions
                .iter()
                .find(|f| f.name == function_name)
                .unwrap(); // We know it exists

            let block = function
                .blocks
                .iter()
                .find(|b| b.id == current_block_id)
                .ok_or(VMError::BlockNotFound(current_block_id))?;

            // Check if we're at the end of the block (need to execute terminator)
            if instruction_pointer >= block.instructions.len() {
                match &block.terminator {
                    Terminator::Ret { value } => {
                        return if let Some(val_id) = value {
                            let frame = self.call_stack.last().unwrap();
                            Ok(frame.get_value(*val_id)?.clone())
                        } else {
                            Ok(RuntimeValue::Void)
                        };
                    }
                    Terminator::Br { target, args } => {
                        // Handle block arguments by setting up the parameters in the target block
                        let frame = self.call_stack.last_mut().unwrap();

                        // If there are arguments, we need to pass them to the target block's parameters
                        if !args.is_empty() {
                            // Find the target block to get its parameters
                            let target_block = function
                                .blocks
                                .iter()
                                .find(|b| b.id == *target)
                                .ok_or(VMError::BlockNotFound(*target))?;

                            // Check that the number of arguments matches the number of parameters
                            if args.len() != target_block.params.len() {
                                return Err(VMError::InvalidInstruction(format!(
                                    "Block parameter count mismatch: expected {}, got {}",
                                    target_block.params.len(),
                                    args.len()
                                )));
                            }

                            // Get the argument values from the current frame
                            let mut arg_values = Vec::new();
                            for arg_id in args {
                                arg_values.push(frame.get_value(*arg_id)?.clone());
                            }

                            // Set up the target block with the new parameter values
                            frame.current_block = *target;
                            frame.instruction_pointer = 0;

                            // Map the block parameters to the argument values
                            for (i, (param_id, _param_type)) in
                                target_block.params.iter().enumerate()
                            {
                                frame.values.insert(*param_id, arg_values[i].clone());
                            }
                        } else {
                            frame.current_block = *target;
                            frame.instruction_pointer = 0;
                        }
                        continue;
                    }
                    Terminator::BrIf {
                        cond,
                        true_target,
                        true_args,
                        false_target,
                        false_args,
                    } => {
                        let frame = self.call_stack.last().unwrap();
                        let cond_value = frame.get_value(*cond)?;

                        let is_true = match cond_value {
                            RuntimeValue::I32(val) => *val != 0,
                            RuntimeValue::I64(val) => *val != 0,
                            _ => {
                                return Err(VMError::TypeMismatch {
                                    expected: Type::I32,
                                    actual: cond_value.get_type(),
                                    context: "conditional branch condition".to_string(),
                                });
                            }
                        };

                        let (target_block_id, target_args) = if is_true {
                            (*true_target, true_args)
                        } else {
                            (*false_target, false_args)
                        };

                        // Handle block arguments similar to Br
                        let frame = self.call_stack.last_mut().unwrap();

                        if !target_args.is_empty() {
                            // Find the target block to get its parameters
                            let target_block = function
                                .blocks
                                .iter()
                                .find(|b| b.id == target_block_id)
                                .ok_or(VMError::BlockNotFound(target_block_id))?;

                            // Check that the number of arguments matches the number of parameters
                            if target_args.len() != target_block.params.len() {
                                return Err(VMError::InvalidInstruction(format!(
                                    "Block parameter count mismatch: expected {}, got {}",
                                    target_block.params.len(),
                                    target_args.len()
                                )));
                            }

                            // Get the argument values from the current frame
                            let mut arg_values = Vec::new();
                            for arg_id in target_args {
                                arg_values.push(frame.get_value(*arg_id)?.clone());
                            }

                            // Set up the target block with the new parameter values
                            frame.current_block = target_block_id;
                            frame.instruction_pointer = 0;

                            // Map the block parameters to the argument values
                            for (i, (param_id, _param_type)) in
                                target_block.params.iter().enumerate()
                            {
                                frame.values.insert(*param_id, arg_values[i].clone());
                            }
                        } else {
                            frame.current_block = target_block_id;
                            frame.instruction_pointer = 0;
                        }
                        continue;
                    }
                }
            }

            // Execute the current instruction
            let instruction = block.instructions[instruction_pointer].clone();
            self.execute_instruction(&instruction)?;

            // Advance instruction pointer
            let frame = self.call_stack.last_mut().unwrap();
            frame.instruction_pointer += 1;
        }
    }

    /// Execute a single instruction
    fn execute_instruction(&mut self, instruction: &Instruction) -> VMResult<()> {
        match instruction {
            Instruction::Call {
                dest,
                function,
                args,
                return_type: _,
            } => {
                // Collect argument values
                let frame = self.call_stack.last().unwrap();
                let arg_values: Result<Vec<_>, _> = args
                    .iter()
                    .map(|arg_id| frame.get_value(*arg_id).map(|v| v.clone()))
                    .collect();
                let arg_values = arg_values?;

                // Try host function first
                if self.host_abi.has_function(function) {
                    let result = self
                        .host_abi
                        .call_host_function(function, &arg_values)
                        .map_err(VMError::HostCallError)?;

                    let frame = self.call_stack.last_mut().unwrap();
                    frame.set_value(*dest, result);
                } else {
                    // Recursive call to TILT function
                    let result = self.call_function(function, arg_values)?;
                    let frame = self.call_stack.last_mut().unwrap();
                    frame.set_value(*dest, result);
                }
            }

            Instruction::CallVoid { function, args } => {
                // Collect argument values
                let frame = self.call_stack.last().unwrap();
                let arg_values: Result<Vec<_>, _> = args
                    .iter()
                    .map(|arg_id| frame.get_value(*arg_id).map(|v| v.clone()))
                    .collect();
                let arg_values = arg_values?;

                // Try host function first
                if self.host_abi.has_function(function) {
                    self.host_abi
                        .call_host_function(function, &arg_values)
                        .map_err(VMError::HostCallError)?;
                } else {
                    // Recursive call to TILT function
                    self.call_function(function, arg_values)?;
                }
            }

            Instruction::Const { dest, value, ty } => {
                let runtime_value = match ty {
                    Type::I32 => RuntimeValue::I32(*value as i32),
                    Type::I64 => RuntimeValue::I64(*value),
                    Type::Usize => RuntimeValue::Usize((*value as u64).try_into().unwrap()),
                    Type::Void => RuntimeValue::Void,
                    Type::F32 | Type::F64 => {
                        return Err(VMError::InvalidInstruction(
                            "Float types not yet supported".to_string(),
                        ));
                    }
                };

                let frame = self.call_stack.last_mut().unwrap();
                frame.set_value(*dest, runtime_value);
            }

            Instruction::BinaryOp {
                dest,
                op,
                ty: _,
                lhs,
                rhs,
            } => {
                let frame = self.call_stack.last().unwrap();
                let lhs_val = frame.get_value(*lhs)?;
                let rhs_val = frame.get_value(*rhs)?;

                let result = match op {
                    BinaryOperator::Add => match (lhs_val, rhs_val) {
                        (RuntimeValue::I32(a), RuntimeValue::I32(b)) => RuntimeValue::I32(a + b),
                        (RuntimeValue::I64(a), RuntimeValue::I64(b)) => RuntimeValue::I64(a + b),
                        (RuntimeValue::Usize(a), RuntimeValue::Usize(b)) => {
                            RuntimeValue::Usize(a.wrapping_add(*b))
                        }
                        _ => {
                            return Err(VMError::TypeMismatch {
                                expected: lhs_val.get_type(),
                                actual: rhs_val.get_type(),
                                context: format!(
                                    "binary add operation (lhs: {:?}, rhs: {:?})",
                                    lhs_val.get_type(),
                                    rhs_val.get_type()
                                ),
                            });
                        }
                    },
                    BinaryOperator::Sub => match (lhs_val, rhs_val) {
                        (RuntimeValue::I32(a), RuntimeValue::I32(b)) => RuntimeValue::I32(a - b),
                        (RuntimeValue::I64(a), RuntimeValue::I64(b)) => RuntimeValue::I64(a - b),
                        (RuntimeValue::Usize(a), RuntimeValue::Usize(b)) => {
                            RuntimeValue::Usize(a.wrapping_sub(*b))
                        }
                        _ => {
                            return Err(VMError::TypeMismatch {
                                expected: lhs_val.get_type(),
                                actual: rhs_val.get_type(),
                                context: format!(
                                    "binary sub operation (lhs: {:?}, rhs: {:?})",
                                    lhs_val.get_type(),
                                    rhs_val.get_type()
                                ),
                            });
                        }
                    },
                    BinaryOperator::Mul => match (lhs_val, rhs_val) {
                        (RuntimeValue::I32(a), RuntimeValue::I32(b)) => RuntimeValue::I32(a * b),
                        (RuntimeValue::I64(a), RuntimeValue::I64(b)) => RuntimeValue::I64(a * b),
                        (RuntimeValue::Usize(a), RuntimeValue::Usize(b)) => {
                            RuntimeValue::Usize(a.wrapping_mul(*b))
                        }
                        _ => {
                            return Err(VMError::TypeMismatch {
                                expected: lhs_val.get_type(),
                                actual: rhs_val.get_type(),
                                context: format!(
                                    "binary mul operation (lhs: {:?}, rhs: {:?})",
                                    lhs_val.get_type(),
                                    rhs_val.get_type()
                                ),
                            });
                        }
                    },
                    BinaryOperator::Div => match (lhs_val, rhs_val) {
                        (RuntimeValue::I32(a), RuntimeValue::I32(b)) => {
                            if *b == 0 {
                                return Err(VMError::DivisionByZero);
                            }
                            RuntimeValue::I32(a / b)
                        }
                        (RuntimeValue::I64(a), RuntimeValue::I64(b)) => {
                            if *b == 0 {
                                return Err(VMError::DivisionByZero);
                            }
                            RuntimeValue::I64(a / b)
                        }
                        (RuntimeValue::Usize(a), RuntimeValue::Usize(b)) => {
                            if *b == 0 {
                                return Err(VMError::DivisionByZero);
                            }
                            RuntimeValue::Usize(a / b)
                        }
                        _ => {
                            return Err(VMError::TypeMismatch {
                                expected: lhs_val.get_type(),
                                actual: rhs_val.get_type(),
                                context: format!(
                                    "binary div operation (lhs: {:?}, rhs: {:?})",
                                    lhs_val.get_type(),
                                    rhs_val.get_type()
                                ),
                            });
                        }
                    },
                    BinaryOperator::Eq => match (lhs_val, rhs_val) {
                        (RuntimeValue::I32(a), RuntimeValue::I32(b)) => {
                            RuntimeValue::I32(if a == b { 1 } else { 0 })
                        }
                        (RuntimeValue::I64(a), RuntimeValue::I64(b)) => {
                            RuntimeValue::I32(if a == b { 1 } else { 0 })
                        }
                        (RuntimeValue::Usize(a), RuntimeValue::Usize(b)) => {
                            RuntimeValue::I32(if a == b { 1 } else { 0 })
                        }
                        _ => {
                            return Err(VMError::TypeMismatch {
                                expected: lhs_val.get_type(),
                                actual: rhs_val.get_type(),
                                context: format!(
                                    "binary eq operation (lhs: {:?}, rhs: {:?})",
                                    lhs_val.get_type(),
                                    rhs_val.get_type()
                                ),
                            });
                        }
                    },
                    BinaryOperator::Lt => match (lhs_val, rhs_val) {
                        (RuntimeValue::I32(a), RuntimeValue::I32(b)) => {
                            RuntimeValue::I32(if a < b { 1 } else { 0 })
                        }
                        (RuntimeValue::I64(a), RuntimeValue::I64(b)) => {
                            RuntimeValue::I32(if a < b { 1 } else { 0 })
                        }
                        (RuntimeValue::Usize(a), RuntimeValue::Usize(b)) => {
                            RuntimeValue::I32(if a < b { 1 } else { 0 })
                        }
                        _ => {
                            return Err(VMError::TypeMismatch {
                                expected: lhs_val.get_type(),
                                actual: rhs_val.get_type(),
                                context: format!(
                                    "binary lt operation (lhs: {:?}, rhs: {:?})",
                                    lhs_val.get_type(),
                                    rhs_val.get_type()
                                ),
                            });
                        }
                    },
                    // For now, return an error for unimplemented operators
                    _ => {
                        return Err(VMError::InvalidInstruction(format!(
                            "Binary operator {:?} not yet implemented",
                            op
                        )));
                    }
                };

                let frame = self.call_stack.last_mut().unwrap();
                frame.set_value(*dest, result);
            }

            Instruction::UnaryOp { .. } => {
                return Err(VMError::InvalidInstruction(
                    "Unary operations not yet implemented".to_string(),
                ));
            }

            Instruction::Load { dest, ty, address } => {
                let frame = self.call_stack.last().unwrap();
                let addr_val = frame.get_value(*address)?;

                let addr = match addr_val {
                    RuntimeValue::Usize(addr) => *addr,
                    _ => {
                        return Err(VMError::TypeMismatch {
                            expected: Type::Usize,
                            actual: addr_val.get_type(),
                            context: "load instruction address".to_string(),
                        });
                    }
                };

                // Use the host ABI to read the value from memory
                let result = self
                    .host_abi
                    .read_memory_value(addr.try_into().unwrap(), *ty)
                    .map_err(|e| {
                        VMError::InvalidInstruction(format!("Memory read error: {}", e))
                    })?;

                let frame = self.call_stack.last_mut().unwrap();
                frame.set_value(*dest, result);
            }

            Instruction::Store {
                address,
                value,
                ty: _,
            } => {
                let frame = self.call_stack.last().unwrap();
                let addr_val = frame.get_value(*address)?;
                let val = frame.get_value(*value)?;

                let addr = match addr_val {
                    RuntimeValue::Usize(addr) => *addr,
                    _ => {
                        return Err(VMError::TypeMismatch {
                            expected: Type::Usize,
                            actual: addr_val.get_type(),
                            context: "store instruction address".to_string(),
                        });
                    }
                };

                // Use the host ABI to write the value to memory
                self.host_abi
                    .write_memory_value(addr.try_into().unwrap(), val)
                    .map_err(|e| {
                        VMError::InvalidInstruction(format!("Memory write error: {}", e))
                    })?;
            }

            Instruction::PtrAdd { dest, ptr, offset } => {
                let frame = self.call_stack.last().unwrap();
                let ptr_val = frame.get_value(*ptr)?;
                let offset_val = frame.get_value(*offset)?;

                let result = match (ptr_val, offset_val) {
                    (RuntimeValue::Usize(ptr_addr), RuntimeValue::Usize(offset_bytes)) => {
                        RuntimeValue::Usize(ptr_addr.wrapping_add(*offset_bytes))
                    }
                    _ => {
                        return Err(VMError::TypeMismatch {
                            expected: Type::Usize,
                            actual: offset_val.get_type(),
                            context: format!(
                                "pointer arithmetic (ptr: {:?}, offset: {:?})",
                                ptr_val.get_type(),
                                offset_val.get_type()
                            ),
                        });
                    }
                };

                let frame = self.call_stack.last_mut().unwrap();
                frame.set_value(*dest, result);
            }

            Instruction::SizeOf { dest, ty } => {
                let size = match ty {
                    Type::I32 => 4,
                    Type::I64 => 8,
                    Type::F32 => 4,
                    Type::F64 => 8,
                    Type::Usize => std::mem::size_of::<usize>(), // Platform-dependent
                    Type::Void => 0,
                };

                let frame = self.call_stack.last_mut().unwrap();
                frame.set_value(*dest, RuntimeValue::Usize(size));
            }

            Instruction::Alloc { dest, size } => {
                let frame = self.call_stack.last().unwrap();
                let size_val = frame.get_value(*size)?;

                let result = if let RuntimeValue::Usize(_size_bytes) = size_val {
                    let result = self
                        .host_abi
                        .call_host_function("alloc", &[size_val.clone()])
                        .map_err(VMError::HostCallError)?;
                    result
                } else {
                    return Err(VMError::TypeMismatch {
                        expected: Type::Usize,
                        actual: size_val.get_type(),
                        context: "alloc instruction size parameter".to_string(),
                    });
                };

                let frame = self.call_stack.last_mut().unwrap();
                frame.set_value(*dest, result);
            }

            Instruction::Free { ptr } => {
                let frame = self.call_stack.last().unwrap();
                let ptr_val = frame.get_value(*ptr)?;

                if let RuntimeValue::Usize(_) = ptr_val {
                    self.host_abi
                        .call_host_function("free", &[ptr_val.clone()])
                        .map_err(VMError::HostCallError)?;
                } else {
                    return Err(VMError::TypeMismatch {
                        expected: Type::Usize,
                        actual: ptr_val.get_type(),
                        context: "free instruction pointer parameter".to_string(),
                    });
                }
            }

            Instruction::Convert {
                dest,
                src,
                from_ty,
                to_ty,
            } => {
                let frame = self.call_stack.last().unwrap();
                let src_val = frame.get_value(*src)?;

                // Verify source type matches expected type
                if src_val.get_type() != *from_ty {
                    return Err(VMError::TypeMismatch {
                        expected: *from_ty,
                        actual: src_val.get_type(),
                        context: format!(
                            "convert instruction source type (converting {:?} to {:?})",
                            from_ty, to_ty
                        ),
                    });
                }

                // Perform type conversion
                let result = match (from_ty, to_ty, src_val) {
                    (Type::I32, Type::I64, RuntimeValue::I32(val)) => {
                        RuntimeValue::I64(*val as i64)
                    }
                    (Type::I32, Type::Usize, RuntimeValue::I32(val)) => {
                        RuntimeValue::Usize(*val as usize)
                    }
                    (Type::I64, Type::I32, RuntimeValue::I64(val)) => {
                        RuntimeValue::I32(*val as i32)
                    }
                    (Type::Usize, Type::I64, RuntimeValue::Usize(val)) => {
                        RuntimeValue::I64(*val as i64)
                    }
                    (Type::Usize, Type::I32, RuntimeValue::Usize(val)) => {
                        RuntimeValue::I32(*val as i32)
                    }
                    (Type::I64, Type::Usize, RuntimeValue::I64(val)) => {
                        RuntimeValue::Usize(*val as usize)
                    }
                    _ => {
                        return Err(VMError::InvalidInstruction(format!(
                            "Unsupported type conversion from {:?} to {:?}",
                            from_ty, to_ty
                        )));
                    }
                };

                let frame = self.call_stack.last_mut().unwrap();
                frame.set_value(*dest, result);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tilt_host_abi::ConsoleHostABI;

    fn create_simple_add_program() -> Program {
        // Create a simple program that adds two numbers
        let mut program = Program {
            imports: vec![],
            functions: vec![],
        };

        // Create the add function: fn add(a: i32, b: i32) -> i32 { return a + b; }
        let mut func = Function::new("add".to_string(), vec![Type::I32, Type::I32], Type::I32);

        // Create entry block
        let entry_block = BasicBlock::new(BlockId::new(0), "entry".to_string());
        func.blocks.push(entry_block);

        // Add parameters to the entry block
        let param_a = ValueId::new(0);
        let param_b = ValueId::new(1);
        func.blocks[0].params.push((param_a, Type::I32));
        func.blocks[0].params.push((param_b, Type::I32));

        // Create add instruction: result = add a, b
        let result = ValueId::new(2);
        let add_instr = Instruction::BinaryOp {
            dest: result,
            op: BinaryOperator::Add,
            ty: Type::I32,
            lhs: param_a,
            rhs: param_b,
        };
        func.blocks[0].instructions.push(add_instr);

        // Set terminator: return result
        func.blocks[0].terminator = Terminator::Ret {
            value: Some(result),
        };

        // Update value counter
        func.next_value_id = ValueId::new(3);

        program.functions.push(func);
        program
    }

    #[test]
    fn test_simple_add_function() {
        let program = create_simple_add_program();
        let host_abi = ConsoleHostABI::new();
        let mut vm = VM::new(program, host_abi);

        let args = vec![RuntimeValue::I32(5), RuntimeValue::I32(3)];
        let result = vm.call_function("add", args).unwrap();

        assert_eq!(result, RuntimeValue::I32(8));
    }

    #[test]
    fn test_function_not_found() {
        let program = create_simple_add_program();
        let host_abi = ConsoleHostABI::new();
        let mut vm = VM::new(program, host_abi);

        let args = vec![RuntimeValue::I32(5), RuntimeValue::I32(3)];
        let result = vm.call_function("nonexistent", args);

        assert!(matches!(result, Err(VMError::FunctionNotFound(_))));
    }

    #[test]
    fn test_wrong_argument_count() {
        let program = create_simple_add_program();
        let host_abi = ConsoleHostABI::new();
        let mut vm = VM::new(program, host_abi);

        let args = vec![RuntimeValue::I32(5)]; // Missing one argument
        let result = vm.call_function("add", args);

        assert!(matches!(result, Err(VMError::InvalidInstruction(_))));
    }

    #[test]
    fn test_host_function_call() {
        let mut program = Program {
            imports: vec![],
            functions: vec![],
        };

        // Create a function that calls a host function
        let mut func = Function::new("test_print".to_string(), vec![Type::I32], Type::Void);

        // Create entry block
        let entry_block = BasicBlock::new(BlockId::new(0), "entry".to_string());
        func.blocks.push(entry_block);

        // Add parameter
        let param = ValueId::new(0);
        func.blocks[0].params.push((param, Type::I32));

        // Call host function print_i32
        let call_instr = Instruction::CallVoid {
            function: "print_i32".to_string(),
            args: vec![param],
        };
        func.blocks[0].instructions.push(call_instr);

        // Return void
        func.blocks[0].terminator = Terminator::Ret { value: None };

        func.next_value_id = ValueId::new(1);
        program.functions.push(func);

        let host_abi = ConsoleHostABI::new();
        let mut vm = VM::new(program, host_abi);

        let args = vec![RuntimeValue::I32(42)];
        let result = vm.call_function("test_print", args).unwrap();

        assert_eq!(result, RuntimeValue::Void);
    }
}
