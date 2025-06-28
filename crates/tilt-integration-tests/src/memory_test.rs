// ===================================================================
// FILE: memory_test.rs
//
// DESC: Test program demonstrating the new memory operations
// ===================================================================

use tilt_ir_builder::ProgramBuilder;
use tilt_ast::Type;
use tilt_vm::VM;
use tilt_host_abi::{MemoryHostABI, RuntimeValue};

#[test]
fn test_memory_operations() {
    let mut builder = ProgramBuilder::new();

    // Add memory allocation import
    builder.add_import_with_cc("host", "alloc", None, vec![Type::I64], Type::Ptr);
    builder.add_import_with_cc("host", "free", None, vec![Type::Ptr], Type::Void);

    // Create a function that uses memory operations
    let func_idx = builder.create_function("test_alloc", vec![], Type::Void);

    {
        let mut func_builder = builder.function_builder(func_idx);
        let entry = func_builder.create_block("entry");
        func_builder.switch_to_block(entry);

        // Allocate 8 bytes
        let size = func_builder.ins().const_i64(8);
        let ptr = func_builder.ins().alloc(size);

        // Free the allocated memory
        func_builder.ins().free(ptr);

        func_builder.ins().ret(None);
    }

    let program = builder.build();

    // Test with VM
    let host_abi = MemoryHostABI::new();
    let mut vm = VM::new(program, host_abi);
    
    let result = vm.call_function("test_alloc", vec![]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), RuntimeValue::Void);
}
