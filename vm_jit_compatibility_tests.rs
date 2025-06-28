// ===================================================================
// FILE: vm_jit_compatibility_tests.rs
//
// DESC: Comprehensive compatibility tests between the VM interpreter
//       and JIT backend to ensure identical behavior across all
//       supported TILT IR features.
// ===================================================================

use std::cell::RefCell;
use std::rc::Rc;
use tilt_ast::Type;
use tilt_codegen_cranelift::JIT;
use tilt_host_abi::{ConsoleHostABI, HostABI, NullHostABI, RuntimeValue};
use tilt_ir_builder::ProgramBuilder;
use tilt_vm::VM;

/// Test result for comparing VM and JIT behavior
#[derive(Debug, Clone, PartialEq)]
struct TestResult {
    return_value: RuntimeValue,
    // Note: We skip output comparison since JIT and VM handle host functions differently
    // Instead we focus on return value compatibility
}

/// Run a program on both VM and JIT and compare return values
fn test_vm_jit_compatibility(
    program: tilt_ir::Program,
    function_name: &str,
    args: Vec<RuntimeValue>,
) -> Result<(), String> {
    // Test with VM first
    let vm_host_abi = NullHostABI::new(); // Use null ABI for deterministic results
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

/// Create a program with basic arithmetic only (no host calls)
fn create_simple_program() -> tilt_ir::Program {
    let mut builder = ProgramBuilder::new();

    // Add function: fn simple_test(x: i32) -> i32 { return x + 1; }
    let func_idx = builder.create_function("simple_test", vec![Type::I32], Type::I32);

    {
        let mut func_builder = builder.function_builder(func_idx);
        let entry = func_builder.create_block("entry");
        func_builder.switch_to_block(entry);

        let x = func_builder.add_param(Type::I32);
        let one = func_builder.ins().const_i32(1);
        let result = func_builder
            .ins()
            .binary_op(tilt_ir::BinaryOperator::Add, Type::I32, x, one);

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

/// Create a program with division and error handling (no host calls)
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

/// Create a program that computes multiple values (no host calls)
fn create_compute_program() -> tilt_ir::Program {
    let mut builder = ProgramBuilder::new();

    // Add function: fn compute(x: i32) -> i32 { return x * x + x + 1; }
    let func_idx = builder.create_function("compute", vec![Type::I32], Type::I32);

    {
        let mut func_builder = builder.function_builder(func_idx);
        let entry = func_builder.create_block("entry");
        func_builder.switch_to_block(entry);

        let x = func_builder.add_param(Type::I32);
        let one = func_builder.ins().const_i32(1);

        // x * x
        let x_squared = func_builder
            .ins()
            .binary_op(tilt_ir::BinaryOperator::Mul, Type::I32, x, x);

        // x * x + x
        let sum1 =
            func_builder
                .ins()
                .binary_op(tilt_ir::BinaryOperator::Add, Type::I32, x_squared, x);

        // x * x + x + 1
        let result =
            func_builder
                .ins()
                .binary_op(tilt_ir::BinaryOperator::Add, Type::I32, sum1, one);

        func_builder.ins().ret(Some(result));
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_computation() {
        let program = create_simple_program();
        let args = vec![RuntimeValue::I32(5)];

        test_vm_jit_compatibility(program, "simple_test", args)
            .expect("Simple computation test should pass");
    }

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
    fn test_polynomial_computation() {
        let program = create_compute_program();

        // Test with various inputs
        for x in [0, 1, -1, 5, -3] {
            let args = vec![RuntimeValue::I32(x)];
            test_vm_jit_compatibility(program.clone(), "compute", args).expect(&format!(
                "Polynomial computation test (x={}) should pass",
                x
            ));
        }
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
    fn test_input_output_compatibility() {
        let program = create_compute_program();
        let args = vec![RuntimeValue::I32(10)];

        test_vm_jit_compatibility(program, "compute", args).expect("Computation test should pass");
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

    #[test]
    fn test_mixed_operations() {
        // Test a more complex program combining multiple features (no host calls)
        let mut builder = ProgramBuilder::new();

        // Function that performs multiple operations
        let func_idx =
            builder.create_function("complex_func", vec![Type::I32, Type::I32], Type::I32);

        {
            let mut func_builder = builder.function_builder(func_idx);

            let entry = func_builder.create_block("entry");
            let positive_branch = func_builder.create_block("positive");
            let negative_branch = func_builder.create_block("negative");

            func_builder.switch_to_block(entry);
            let a = func_builder.add_param(Type::I32);
            let b = func_builder.add_param(Type::I32);

            // Compute a + b
            let sum = func_builder
                .ins()
                .binary_op(tilt_ir::BinaryOperator::Add, Type::I32, a, b);

            // Check if sum > 0
            let zero = func_builder.ins().const_i32(0);
            let is_positive =
                func_builder
                    .ins()
                    .binary_op(tilt_ir::BinaryOperator::Lt, Type::I32, zero, sum);
            func_builder
                .ins()
                .br_if(is_positive, positive_branch, negative_branch);

            // Positive branch: return sum * 2
            func_builder.switch_to_block(positive_branch);
            let two = func_builder.ins().const_i32(2);
            let result_pos =
                func_builder
                    .ins()
                    .binary_op(tilt_ir::BinaryOperator::Mul, Type::I32, sum, two);
            func_builder.ins().ret(Some(result_pos));

            // Negative branch: return sum - 1
            func_builder.switch_to_block(negative_branch);
            let one = func_builder.ins().const_i32(1);
            let result_neg =
                func_builder
                    .ins()
                    .binary_op(tilt_ir::BinaryOperator::Sub, Type::I32, sum, one);
            func_builder.ins().ret(Some(result_neg));
        }

        let program = builder.build();

        // Test various inputs
        let test_cases = vec![
            (5, 10),  // Positive sum
            (-3, -2), // Negative sum
            (1, -1),  // Zero sum
            (0, 5),   // Positive sum with zero
        ];

        for (a, b) in test_cases {
            let args = vec![RuntimeValue::I32(a), RuntimeValue::I32(b)];
            test_vm_jit_compatibility(program.clone(), "complex_func", args)
                .expect(&format!("Mixed operations test ({}, {}) should pass", a, b));
        }
    }
}

// Integration test function that can be called from main
pub fn run_all_compatibility_tests() -> Result<(), String> {
    println!("Running comprehensive VM <-> JIT compatibility tests...");

    // Test 1: Simple computation
    println!("  Testing simple computation...");
    let program = create_simple_program();
    let args = vec![RuntimeValue::I32(5)];
    test_vm_jit_compatibility(program, "simple_test", args)?;
    println!("    ✓ Simple computation test passed");

    // Test 2: Arithmetic operations
    println!("  Testing arithmetic operations...");
    let program = create_arithmetic_program();
    let args = vec![
        RuntimeValue::I32(5),
        RuntimeValue::I32(3),
        RuntimeValue::I32(2),
    ];
    test_vm_jit_compatibility(program, "add_mul", args)?;
    println!("    ✓ Arithmetic operations test passed");

    // Test 3: Polynomial computation
    println!("  Testing polynomial computation...");
    let program = create_compute_program();
    for x in [0, 1, -1, 5, -3] {
        let args = vec![RuntimeValue::I32(x)];
        test_vm_jit_compatibility(program.clone(), "compute", args)?;
    }
    println!("    ✓ Polynomial computation test passed");

    // Test 4: Conditional logic
    println!("  Testing conditional logic...");
    let program = create_conditional_program();
    let test_cases = vec![
        (10, 5), // a > b
        (3, 7),  // a < b
        (5, 5),  // a == b
    ];
    for (a, b) in test_cases {
        let args = vec![RuntimeValue::I32(a), RuntimeValue::I32(b)];
        test_vm_jit_compatibility(program.clone(), "max", args)?;
    }
    println!("    ✓ Conditional logic test passed");

    // Test 5: Recursion
    println!("  Testing recursion...");
    let program = create_recursive_program();
    for n in [0, 1, 3, 5] {
        let args = vec![RuntimeValue::I32(n)];
        test_vm_jit_compatibility(program.clone(), "factorial", args)?;
    }
    println!("    ✓ Recursion test passed");

    // Test 6: Division and error handling
    println!("  Testing division and error handling...");
    let program = create_division_program();
    let test_cases = vec![
        (15, 3), // Normal division
        (10, 0), // Division by zero
        (-8, 2), // Negative division
    ];
    for (a, b) in test_cases {
        let args = vec![RuntimeValue::I32(a), RuntimeValue::I32(b)];
        test_vm_jit_compatibility(program.clone(), "safe_divide", args)?;
    }
    println!("    ✓ Division and error handling test passed");

    println!("All VM <-> JIT compatibility tests passed! ✓");
    Ok(())
}

fn main() {
    match run_all_compatibility_tests() {
        Ok(()) => {
            println!("🎉 All compatibility tests completed successfully!");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("❌ Compatibility test failed: {}", e);
            std::process::exit(1);
        }
    }
}
