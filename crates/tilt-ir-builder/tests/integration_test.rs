use tilt_ir_builder::ProgramBuilder;
use tilt_ast::Type;

#[test]
fn test_end_to_end_program_construction() {
    let mut builder = ProgramBuilder::new();
    
    // Add some imports
    builder.add_import("env", "print_i32", vec![Type::I32], Type::Void);
    
    // Create a simple function that adds two numbers
    let func_idx = builder.create_function("add", vec![Type::I32, Type::I32], Type::I32);
    
    {
        let mut func_builder = builder.function_builder(func_idx);
        
        // Create entry block
        let entry = func_builder.create_block("entry");
        func_builder.switch_to_block(entry);
        
        // Add parameters
        let param1 = func_builder.add_block_param(entry, Type::I32);
        let param2 = func_builder.add_block_param(entry, Type::I32);
        
        // Add them together
        let result = func_builder.ins().add(Type::I32, param1, param2);
        
        // Return the result
        func_builder.ins().ret(Some(result));
    }
    
    // Build the program
    let program = builder.build();
    
    // Validate the program structure
    assert_eq!(program.imports.len(), 1);
    assert_eq!(program.functions.len(), 1);
    assert_eq!(program.functions[0].name, "add");
    assert_eq!(program.functions[0].blocks.len(), 1);
}
