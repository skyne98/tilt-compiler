// ===================================================================
// FILE: compatibility_tests.rs
//
// DESC: Integration tests for VM <-> JIT compatibility
// ===================================================================

use tilt_ast::Type;
use tilt_codegen_cranelift::JIT;
use tilt_host_abi::{NullHostABI, RuntimeValue};
use tilt_ir_builder::ProgramBuilder;
use tilt_vm::VM;

/// Run a program on both VM and JIT and compare return values
fn test_vm_jit_compatibility(
    program: tilt_ir::Program,
    function_name: &str,
    args: Vec<RuntimeValue>,
) -> Result<(), String> {
    // Test with VM first
    let vm_host_abi = NullHostABI::new();
    let mut vm = VM::new(program.clone(), vm_host_abi);

    let vm_result = vm
        .call_function(function_name, args.clone())
        .map_err(|e| format!("VM execution failed: {:?}", e))?;

    // Test with JIT
    let jit_host_abi = Box::new(NullHostABI::new());
    let mut jit =
        JIT::new_with_abi(jit_host_abi).map_err(|e| format!("Failed to create JIT: {:?}", e))?;

    jit.compile(&program)
        .map_err(|e| format!("JIT compilation failed: {:?}", e))?;

    // Get function pointer and call it with proper signature
    let func_ptr = jit
        .get_func_ptr(function_name)
        .ok_or_else(|| format!("Function '{}' not found in JIT", function_name))?;

    // Execute the function based on its signature and return type
    let jit_result = unsafe {
        match args.len() {
            0 => {
                // Check if this is a void or returning function by examining the program
                let func = program
                    .functions
                    .iter()
                    .find(|f| f.name == function_name)
                    .ok_or_else(|| format!("Function {} not found in program", function_name))?;

                if func.return_type == Type::Void {
                    let func_fn = std::mem::transmute::<*const u8, fn()>(func_ptr);
                    func_fn();
                    RuntimeValue::Void
                } else {
                    let func_fn = std::mem::transmute::<*const u8, fn() -> i32>(func_ptr);
                    RuntimeValue::I32(func_fn())
                }
            }
            1 => {
                let arg0 = match &args[0] {
                    RuntimeValue::I32(val) => *val,
                    RuntimeValue::I64(val) => *val as i32,
                    RuntimeValue::Void => 0,
                };

                let func = program
                    .functions
                    .iter()
                    .find(|f| f.name == function_name)
                    .ok_or_else(|| format!("Function {} not found in program", function_name))?;

                if func.return_type == Type::Void {
                    let func_fn = std::mem::transmute::<*const u8, fn(i32)>(func_ptr);
                    func_fn(arg0);
                    RuntimeValue::Void
                } else {
                    let func_fn = std::mem::transmute::<*const u8, fn(i32) -> i32>(func_ptr);
                    RuntimeValue::I32(func_fn(arg0))
                }
            }
            2 => {
                let arg0 = match &args[0] {
                    RuntimeValue::I32(val) => *val,
                    RuntimeValue::I64(val) => *val as i32,
                    RuntimeValue::Void => 0,
                };
                let arg1 = match &args[1] {
                    RuntimeValue::I32(val) => *val,
                    RuntimeValue::I64(val) => *val as i32,
                    RuntimeValue::Void => 0,
                };

                let func = program
                    .functions
                    .iter()
                    .find(|f| f.name == function_name)
                    .ok_or_else(|| format!("Function {} not found in program", function_name))?;

                if func.return_type == Type::Void {
                    let func_fn = std::mem::transmute::<*const u8, fn(i32, i32)>(func_ptr);
                    func_fn(arg0, arg1);
                    RuntimeValue::Void
                } else {
                    let func_fn = std::mem::transmute::<*const u8, fn(i32, i32) -> i32>(func_ptr);
                    RuntimeValue::I32(func_fn(arg0, arg1))
                }
            }
            3 => {
                let arg0 = match &args[0] {
                    RuntimeValue::I32(val) => *val,
                    RuntimeValue::I64(val) => *val as i32,
                    RuntimeValue::Void => 0,
                };
                let arg1 = match &args[1] {
                    RuntimeValue::I32(val) => *val,
                    RuntimeValue::I64(val) => *val as i32,
                    RuntimeValue::Void => 0,
                };
                let arg2 = match &args[2] {
                    RuntimeValue::I32(val) => *val,
                    RuntimeValue::I64(val) => *val as i32,
                    RuntimeValue::Void => 0,
                };

                let func = program
                    .functions
                    .iter()
                    .find(|f| f.name == function_name)
                    .ok_or_else(|| format!("Function {} not found in program", function_name))?;

                if func.return_type == Type::Void {
                    let func_fn = std::mem::transmute::<*const u8, fn(i32, i32, i32)>(func_ptr);
                    func_fn(arg0, arg1, arg2);
                    RuntimeValue::Void
                } else {
                    let func_fn =
                        std::mem::transmute::<*const u8, fn(i32, i32, i32) -> i32>(func_ptr);
                    RuntimeValue::I32(func_fn(arg0, arg1, arg2))
                }
            }
            _ => return Err("Too many arguments for JIT function call".to_string()),
        }
    };

    // Compare results
    if vm_result != jit_result {
        return Err(format!(
            "VM and JIT results differ!\nVM: {:?}\nJIT: {:?}",
            vm_result, jit_result
        ));
    }

    Ok(())
}

/// Create a simple arithmetic program
fn create_arithmetic_program() -> tilt_ir::Program {
    let mut builder = ProgramBuilder::new();

    // Add function: fn add_mul(a: i32, b: i32, c: i32) -> i32 { return (a + b) * c; }
    let func_idx =
        builder.create_function("add_mul", vec![Type::I32, Type::I32, Type::I32], Type::I32);

    {
        let mut func_builder = builder.function_builder(func_idx);
        let entry = func_builder.create_block("entry");
        func_builder.switch_to_block(entry);

        let a = func_builder.add_param(Type::I32);
        let b = func_builder.add_param(Type::I32);
        let c = func_builder.add_param(Type::I32);

        let sum = func_builder
            .ins()
            .binary_op(tilt_ir::BinaryOperator::Add, Type::I32, a, b);
        let result = func_builder
            .ins()
            .binary_op(tilt_ir::BinaryOperator::Mul, Type::I32, sum, c);

        func_builder.ins().ret(Some(result));
    }

    builder.build()
}

/// Create a program with conditional logic
fn create_conditional_program() -> tilt_ir::Program {
    let mut builder = ProgramBuilder::new();

    // Add function: fn max(a: i32, b: i32) -> i32 { if a > b { return a; } else { return b; } }
    let func_idx = builder.create_function("max", vec![Type::I32, Type::I32], Type::I32);

    {
        let mut func_builder = builder.function_builder(func_idx);

        let entry = func_builder.create_block("entry");
        let then_block = func_builder.create_block("then");
        let else_block = func_builder.create_block("else");

        func_builder.switch_to_block(entry);
        let a = func_builder.add_param(Type::I32);
        let b = func_builder.add_param(Type::I32);

        // Compare a > b (implemented as b < a)
        let cmp = func_builder
            .ins()
            .binary_op(tilt_ir::BinaryOperator::Lt, Type::I32, b, a);
        func_builder.ins().br_if(cmp, then_block, else_block);

        // Then block: return a
        func_builder.switch_to_block(then_block);
        func_builder.ins().ret(Some(a));

        // Else block: return b
        func_builder.switch_to_block(else_block);
        func_builder.ins().ret(Some(b));
    }

    builder.build()
}

/// Create a program with recursion
fn create_recursive_program() -> tilt_ir::Program {
    let mut builder = ProgramBuilder::new();

    // Add function: fn factorial(n: i32) -> i32 { if n <= 1 { return 1; } else { return n * factorial(n - 1); } }
    let func_idx = builder.create_function("factorial", vec![Type::I32], Type::I32);

    {
        let mut func_builder = builder.function_builder(func_idx);

        let entry = func_builder.create_block("entry");
        let base_case = func_builder.create_block("base_case");
        let recursive_case = func_builder.create_block("recursive_case");

        func_builder.switch_to_block(entry);
        let n = func_builder.add_param(Type::I32);

        // Check if n <= 1
        let one = func_builder.ins().const_i32(1);
        let cmp = func_builder
            .ins()
            .binary_op(tilt_ir::BinaryOperator::Lt, Type::I32, one, n);
        func_builder.ins().br_if(cmp, recursive_case, base_case);

        // Base case: return 1
        func_builder.switch_to_block(base_case);
        func_builder.ins().ret(Some(one));

        // Recursive case: return n * factorial(n - 1)
        func_builder.switch_to_block(recursive_case);
        let n_minus_1 =
            func_builder
                .ins()
                .binary_op(tilt_ir::BinaryOperator::Sub, Type::I32, n, one);
        let factorial_result = func_builder
            .ins()
            .call("factorial", vec![n_minus_1], Type::I32);
        let result = func_builder.ins().binary_op(
            tilt_ir::BinaryOperator::Mul,
            Type::I32,
            n,
            factorial_result,
        );
        func_builder.ins().ret(Some(result));
    }

    builder.build()
}

/// Create a program with division and error handling
fn create_division_program() -> tilt_ir::Program {
    let mut builder = ProgramBuilder::new();

    // Add function: fn safe_divide(a: i32, b: i32) -> i32 { if b == 0 { return -1; } else { return a / b; } }
    let func_idx = builder.create_function("safe_divide", vec![Type::I32, Type::I32], Type::I32);

    {
        let mut func_builder = builder.function_builder(func_idx);

        let entry = func_builder.create_block("entry");
        let div_by_zero = func_builder.create_block("div_by_zero");
        let normal_div = func_builder.create_block("normal_div");

        func_builder.switch_to_block(entry);
        let a = func_builder.add_param(Type::I32);
        let b = func_builder.add_param(Type::I32);

        // Check if b == 0
        let zero = func_builder.ins().const_i32(0);
        let is_zero = func_builder
            .ins()
            .binary_op(tilt_ir::BinaryOperator::Eq, Type::I32, b, zero);
        func_builder.ins().br_if(is_zero, div_by_zero, normal_div);

        // Division by zero case: return -1
        func_builder.switch_to_block(div_by_zero);
        let minus_one = func_builder.ins().const_i32(-1);
        func_builder.ins().ret(Some(minus_one));

        // Normal division case: return a / b
        func_builder.switch_to_block(normal_div);
        let result = func_builder
            .ins()
            .binary_op(tilt_ir::BinaryOperator::Div, Type::I32, a, b);
        func_builder.ins().ret(Some(result));
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arithmetic_compatibility() {
        let program = create_arithmetic_program();
        let args = vec![
            RuntimeValue::I32(5),
            RuntimeValue::I32(3),
            RuntimeValue::I32(2),
        ];

        test_vm_jit_compatibility(program, "add_mul", args).expect("Arithmetic test should pass");
    }

    #[test]
    fn test_conditional_compatibility() {
        let program = create_conditional_program();

        // Test case where a > b
        let args1 = vec![RuntimeValue::I32(10), RuntimeValue::I32(5)];
        test_vm_jit_compatibility(program.clone(), "max", args1)
            .expect("Conditional test (a > b) should pass");

        // Test case where a < b
        let args2 = vec![RuntimeValue::I32(3), RuntimeValue::I32(7)];
        test_vm_jit_compatibility(program.clone(), "max", args2)
            .expect("Conditional test (a < b) should pass");

        // Test case where a == b
        let args3 = vec![RuntimeValue::I32(5), RuntimeValue::I32(5)];
        test_vm_jit_compatibility(program, "max", args3)
            .expect("Conditional test (a == b) should pass");
    }

    #[test]
    fn test_recursion_compatibility() {
        let program = create_recursive_program();

        // Test factorial(0) = 1
        let args1 = vec![RuntimeValue::I32(0)];
        test_vm_jit_compatibility(program.clone(), "factorial", args1)
            .expect("Recursion test (factorial 0) should pass");

        // Test factorial(1) = 1
        let args2 = vec![RuntimeValue::I32(1)];
        test_vm_jit_compatibility(program.clone(), "factorial", args2)
            .expect("Recursion test (factorial 1) should pass");

        // Test factorial(5) = 120
        let args3 = vec![RuntimeValue::I32(5)];
        test_vm_jit_compatibility(program, "factorial", args3)
            .expect("Recursion test (factorial 5) should pass");
    }

    #[test]
    fn test_division_compatibility() {
        let program = create_division_program();

        // Test normal division
        let args1 = vec![RuntimeValue::I32(15), RuntimeValue::I32(3)];
        test_vm_jit_compatibility(program.clone(), "safe_divide", args1)
            .expect("Division test (normal) should pass");

        // Test division by zero
        let args2 = vec![RuntimeValue::I32(10), RuntimeValue::I32(0)];
        test_vm_jit_compatibility(program, "safe_divide", args2)
            .expect("Division test (by zero) should pass");
    }

    #[test]
    fn test_edge_cases() {
        // Test with maximum and minimum i32 values
        let program = create_arithmetic_program();

        // Test with large numbers
        let args1 = vec![
            RuntimeValue::I32(i32::MAX),
            RuntimeValue::I32(0),
            RuntimeValue::I32(1),
        ];
        test_vm_jit_compatibility(program.clone(), "add_mul", args1)
            .expect("Edge case test (large numbers) should pass");

        // Test with negative numbers
        let args2 = vec![
            RuntimeValue::I32(-10),
            RuntimeValue::I32(5),
            RuntimeValue::I32(-2),
        ];
        test_vm_jit_compatibility(program, "add_mul", args2)
            .expect("Edge case test (negative numbers) should pass");
    }

    #[test]
    fn test_zero_operations() {
        let program = create_arithmetic_program();

        // Test with all zeros
        let args = vec![
            RuntimeValue::I32(0),
            RuntimeValue::I32(0),
            RuntimeValue::I32(0),
        ];
        test_vm_jit_compatibility(program, "add_mul", args)
            .expect("Zero operations test should pass");
    }
}
