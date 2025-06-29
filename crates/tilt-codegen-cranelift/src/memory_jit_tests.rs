// ===================================================================
// FILE: memory_jit_tests.rs
//
// DESC: Comprehensive tests for JIT memory operations to identify and
//       fix crashes and ensure correct behavior for all memory primitives.
// ===================================================================

use crate::JIT;
use logos::Logos;
use tilt_host_abi::{JITMemoryHostABI, RuntimeValue};
use tilt_ir::lowering::lower_program;
use tilt_parser::{lexer::Token, tilt::ProgramParser};

fn parse_and_lower(source: &str) -> Result<tilt_ir::Program, String> {
    // Tokenize
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next() {
        let token = token.map_err(|_| "Lexing error")?;
        let span = lexer.span();
        tokens.push((span.start, token, span.end));
    }

    // Debug: print tokens for failed cases
    if source.contains("alloc") {
        println!("Source code:");
        println!("{}", source);
        println!("Tokens for alloc test:");
        for (i, (start, token, end)) in tokens.iter().enumerate() {
            println!("{}: {}..{} {:?}", i, start, end, token);
            if i >= 35 && i <= 42 {
                println!("  -> Text: '{}'", &source[*start..*end]);
            }
        }
    }

    // Parse
    let parser = ProgramParser::new();
    let ast = parser
        .parse(tokens)
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Lower to IR
    lower_program(&ast).map_err(|errors| {
        let mut error_msg = "Semantic analysis failed:\n".to_string();
        for error in &errors {
            error_msg.push_str(&format!("  • {}\n", error));
        }
        error_msg
    })
}

fn execute_jit_program(source: &str) -> Result<RuntimeValue, String> {
    let program = parse_and_lower(source)?;

    let host_abi = Box::new(JITMemoryHostABI::new());
    let mut jit =
        JIT::new_with_abi(host_abi).map_err(|e| format!("Failed to create JIT: {}", e))?;

    // Enable Cranelift IR output for debugging
    jit.set_show_cranelift_ir(true);

    jit.compile(&program)
        .map_err(|e| format!("JIT compilation failed: {}", e))?;

    let main_ptr = jit
        .get_func_ptr("main")
        .ok_or("Main function not found in JIT compiled code")?;

    let main_function = program
        .functions
        .iter()
        .find(|f| f.name == "main")
        .ok_or("Main function not found in program")?;

    unsafe {
        match main_function.return_type {
            tilt_ast::Type::I32 => {
                let main_fn = std::mem::transmute::<*const u8, fn() -> i32>(main_ptr);
                let result = main_fn();
                Ok(RuntimeValue::I32(result))
            }
            tilt_ast::Type::I64 => {
                let main_fn = std::mem::transmute::<*const u8, fn() -> i64>(main_ptr);
                let result = main_fn();
                Ok(RuntimeValue::I64(result))
            }
            tilt_ast::Type::Usize => {
                let main_fn = std::mem::transmute::<*const u8, fn() -> usize>(main_ptr);
                let result = main_fn();
                Ok(RuntimeValue::Usize(result))
            }
            tilt_ast::Type::Void => {
                let main_fn = std::mem::transmute::<*const u8, fn()>(main_ptr);
                main_fn();
                Ok(RuntimeValue::Void)
            }
            _ => Err(format!(
                "Unsupported main function return type: {:?}",
                main_function.return_type
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jit_simple_constant() {
        let source = r#"
            fn main() -> i32 {
            entry:
                result:i32 = i32.const(42)
                ret (result)
            }
        "#;

        let result = execute_jit_program(source).expect("JIT execution should succeed");
        assert_eq!(result, RuntimeValue::I32(42));
    }

    #[test]
    fn test_jit_sizeof_operation() {
        let source = r#"
            fn main() -> usize {
            entry:
                size:usize = sizeof.i32()
                ret (size)
            }
        "#;

        let result = execute_jit_program(source).expect("JIT execution should succeed");
        #[cfg(target_pointer_width = "64")]
        assert_eq!(result, RuntimeValue::Usize(4));
        #[cfg(target_pointer_width = "32")]
        assert_eq!(result, RuntimeValue::Usize(4));
    }

    #[test]
    fn test_jit_multiple_sizeof() {
        let source = r#"
            fn main() -> usize {
            entry:
                size1:usize = sizeof.i32()
                size2:usize = sizeof.i64()
                total:usize = usize.add(size1, size2)
                ret (total)
            }
        "#;

        let result = execute_jit_program(source).expect("JIT execution should succeed");
        #[cfg(target_pointer_width = "64")]
        assert_eq!(result, RuntimeValue::Usize(12)); // 4 + 8
        #[cfg(target_pointer_width = "32")]
        assert_eq!(result, RuntimeValue::Usize(12)); // 4 + 8
    }

    #[test]
    fn test_jit_simple_allocation() {
        let source = r#"
            import "host" "alloc" (size:usize) -> usize
            import "host" "free" (p:usize) -> void

            fn main() -> i32 {
            entry:
                size:usize = usize.const(8)
                ptr:usize = alloc(size)
                free(ptr)
                result:i32 = i32.const(1)
                ret (result)
            }
        "#;

        let result = execute_jit_program(source).expect("JIT execution should succeed");
        assert_eq!(result, RuntimeValue::I32(1));
    }

    #[test]
    fn test_jit_alloc_free_basic() {
        let source = r#"
            import "host" "alloc" (size:usize) -> usize
            import "host" "free" (p:usize) -> void

            fn main() -> void {
            entry:
                size:usize = sizeof.i32()
                ptr:usize = alloc(size)
                free(ptr)
                ret
            }
        "#;

        let result = execute_jit_program(source).expect("JIT execution should succeed");
        assert_eq!(result, RuntimeValue::Void);
    }

    #[test]
    fn test_jit_store_load_simple() {
        let source = r#"
            import "host" "alloc" (size:usize) -> usize
            import "host" "free" (p:usize) -> void

            fn main() -> i32 {
            entry:
                size:usize = sizeof.i32()
                ptr:usize = alloc(size)
                value:i32 = i32.const(42)
                i32.store(ptr, value)
                loaded:i32 = i32.load(ptr)
                free(ptr)
                ret (loaded)
            }
        "#;

        let result = execute_jit_program(source).expect("JIT execution should succeed");
        assert_eq!(result, RuntimeValue::I32(42));
    }

    #[test]
    fn test_jit_pointer_arithmetic() {
        let source = r#"
            import "host" "alloc" (size:usize) -> usize
            import "host" "free" (p:usize) -> void

            fn main() -> i32 {
            entry:
                size:usize = usize.const(8)
                ptr:usize = alloc(size)
                
                val1:i32 = i32.const(10)
                i32.store(ptr, val1)
                
                offset:usize = sizeof.i32()
                ptr2:usize = usize.add(ptr, offset)
                val2:i32 = i32.const(20)
                i32.store(ptr2, val2)
                
                loaded1:i32 = i32.load(ptr)
                loaded2:i32 = i32.load(ptr2)
                result:i32 = i32.add(loaded1, loaded2)
                
                free(ptr)
                ret (result)
            }
        "#;

        let result = execute_jit_program(source).expect("JIT execution should succeed");
        assert_eq!(result, RuntimeValue::I32(30)); // 10 + 20
    }

    #[test]
    fn test_jit_function_call_with_memory() {
        let source = r#"
            import "host" "alloc" (size:usize) -> usize
            import "host" "free" (p:usize) -> void

            fn allocate_and_store(value:i32) -> i32 {
            entry:
                size:usize = sizeof.i32()
                ptr:usize = alloc(size)
                i32.store(ptr, value)
                loaded:i32 = i32.load(ptr)
                free(ptr)
                ret (loaded)
            }

            fn main() -> i32 {
            entry:
                value:i32 = i32.const(99)
                result:i32 = allocate_and_store(value)
                ret (result)
            }
        "#;

        let result = execute_jit_program(source).expect("JIT execution should succeed");
        assert_eq!(result, RuntimeValue::I32(99));
    }

    #[test]
    fn test_jit_complex_memory_operations() {
        let source = r#"
            import "host" "alloc" (size:usize) -> usize
            import "host" "free" (p:usize) -> void

            fn main() -> i32 {
            entry:
                # Allocate space for 3 i32 values  
                element_size:usize = sizeof.i32()
                count:usize = usize.const(3)
                total_size:usize = usize.mul(element_size, count)
                ptr:usize = alloc(total_size)
                
                # Store values at different offsets
                val1:i32 = i32.const(100)
                i32.store(ptr, val1)
                
                offset1:usize = sizeof.i32()
                ptr2:usize = usize.add(ptr, offset1)
                val2:i32 = i32.const(200)
                i32.store(ptr2, val2)
                
                # Calculate offset for third element: 2 * sizeof(i32)
                two:usize = usize.const(2)
                element_size_2:usize = sizeof.i32()
                offset2:usize = usize.mul(element_size_2, two)
                ptr3:usize = usize.add(ptr, offset2)
                val3:i32 = i32.const(300)
                i32.store(ptr3, val3)
                
                # Load and sum all values
                loaded1:i32 = i32.load(ptr)
                loaded2:i32 = i32.load(ptr2)
                loaded3:i32 = i32.load(ptr3)
                
                sum1:i32 = i32.add(loaded1, loaded2)
                result:i32 = i32.add(sum1, loaded3)
                
                free(ptr)
                ret (result)
            }
        "#;

        let result = execute_jit_program(source).expect("JIT execution should succeed");
        assert_eq!(result, RuntimeValue::I32(600)); // 100 + 200 + 300
    }
}
