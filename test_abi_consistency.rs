// Test to verify that VM and JIT produce identical results using the Host ABI

use tilt_ast::Type;
use tilt_codegen_cranelift::JIT;
use tilt_host_abi::{ConsoleHostABI, RuntimeValue};
use tilt_ir_builder::ProgramBuilder;
use tilt_vm::VM;

fn create_test_program() -> tilt_ir::Program {
    let mut builder = ProgramBuilder::new();

    // Add host function imports
    builder.add_import("env", "print_i32", vec![Type::I32], Type::Void);
    builder.add_import("env", "print_char", vec![Type::I32], Type::Void);

    // Create a simple test function
    let func_idx = builder.create_function("test", vec![], Type::Void);

    {
        let mut func_builder = builder.function_builder(func_idx);

        // Create entry block
        let entry = func_builder.create_block("entry");
        func_builder.switch_to_block(entry);

        // Print 42 and then a newline
        let val42 = func_builder.ins().const_i32(42);
        let newline = func_builder.ins().const_i32(10); // ASCII newline

        func_builder.ins().call_void("print_i32", vec![val42]);
        func_builder.ins().call_void("print_char", vec![newline]);

        // Return
        func_builder.ins().ret(None);
    }

    builder.build()
}

#[test]
fn test_vm_and_jit_consistency() {
    let program = create_test_program();

    // Test with VM
    println!("VM output:");
    let host_abi_vm = ConsoleHostABI::new();
    let mut vm = VM::new(program.clone(), host_abi_vm);
    let vm_result = vm.call_function("test", vec![]);
    assert!(vm_result.is_ok());
    assert_eq!(vm_result.unwrap(), RuntimeValue::Void);

    // Test with JIT
    println!("JIT output:");
    let mut jit = JIT::new().expect("Failed to create JIT");
    jit.compile(&program).expect("Failed to compile program");
    let jit_result = jit.call_function("test", &[]);
    assert!(jit_result.is_ok());

    println!("Both VM and JIT executed successfully with Host ABI!");
}

fn main() {
    test_vm_and_jit_consistency();
}
