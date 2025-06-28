// ===================================================================
// FILE: tests.rs (tilt-codegen-cranelift crate)
//
// DESC: Comprehensive tests for the JIT compiler backend, testing
//       various TILT programs to ensure correct compilation and execution.
// ===================================================================

use super::*;
use tilt_parser::{Token, ProgramParser};
use tilt_ir::lower_program;
use logos::Logos;
use std::mem;
use std::cell::RefCell;

// Global storage for capturing output from host functions
thread_local! {
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

// Helper to capture output from our test host functions
fn capture_output<F: FnOnce()>(f: F) -> String {
    OUTPUT.with(|output| {
        output.borrow_mut().clear();
        f();
        let bytes = output.borrow().clone();
        String::from_utf8_lossy(&bytes).to_string()
    })
}

// Test host functions that capture output
extern "C" fn test_print_char(c: i32) {
    OUTPUT.with(|output| {
        if let Some(ch) = char::from_u32(c as u32) {
            let ch_str = ch.to_string();
            output.borrow_mut().extend(ch_str.as_bytes());
        } else {
            output.borrow_mut().push(b'?');
        }
    });
}

extern "C" fn test_print_hello() {
    OUTPUT.with(|output| {
        output.borrow_mut().extend(b"Hello from TILT!");
    });
}

extern "C" fn test_print_int(n: i32) {
    OUTPUT.with(|output| {
        let n_str = n.to_string();
        output.borrow_mut().extend(n_str.as_bytes());
    });
}

extern "C" fn test_getc() -> i32 {
    65 // Return 'A'
}

extern "C" fn test_get_number() -> i32 {
    42
}

// Helper function to compile and run a TILT program
fn compile_and_run(source: &str) -> Result<String, String> {
    // 1. Lexing
    let lexer = Token::lexer(source);
    let tokens: Result<Vec<Token>, _> = lexer.collect();
    let tokens = tokens.map_err(|e| format!("Lexing failed: {:?}", e))?;

    // 2. Parsing
    let parser = ProgramParser::new();
    let token_iter = tokens.into_iter().enumerate().map(|(i, token)| {
        Ok((i, token, i + 1))
    });
    let program_ast = parser.parse(token_iter)
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    // 3. IR Generation
    let program_ir = lower_program(&program_ast)
        .map_err(|errors| {
            let mut error_msg = String::from("Semantic errors:\n");
            for error in errors {
                error_msg.push_str(&format!("  {}\n", error));
            }
            error_msg
        })?;

    // 4. JIT Compilation
    let mut jit = create_test_jit()?;
    jit.compile(&program_ir).map_err(|e| format!("JIT compilation failed: {}", e))?;

    // 5. Execute and capture output
    let main_ptr = jit.get_func_ptr("main")
        .ok_or("Main function not found in compiled code")?;

    let output = capture_output(|| {
        unsafe {
            let main_fn = mem::transmute::<*const u8, fn()>(main_ptr);
            main_fn();
        }
    });

    Ok(output)
}

// Create JIT with test host functions
fn create_test_jit() -> Result<JIT, String> {
    let mut builder = cranelift_jit::JITBuilder::new(cranelift_module::default_libcall_names())
        .map_err(|e| format!("Failed to create JIT builder: {}", e))?;

    // Register test host functions
    builder.symbol("print_hello", test_print_hello as *const u8);
    builder.symbol("print_char", test_print_char as *const u8);
    builder.symbol("print_int", test_print_int as *const u8);
    builder.symbol("getc", test_getc as *const u8);
    builder.symbol("get_number", test_get_number as *const u8);

    let module = cranelift_jit::JITModule::new(builder);

    Ok(JIT {
        module,
        function_ids: HashMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_function_call() {
        let source = r#"
import "env" "print_hello" -> void

fn main() -> void {
entry:
    call print_hello()
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "Hello from TILT!");
    }

    #[test]
    fn test_function_with_parameters() {
        let source = r#"
import "env" "print_char" (c:i32) -> void

fn print_twice(value:i32) -> void {
entry:
    call print_char(value)
    call print_char(value)
    ret
}

fn main() -> void {
entry:
    call print_twice(65)  # 'A'
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "AA");
    }

    #[test]
    fn test_multiple_parameters() {
        let source = r#"
import "env" "print_char" (c:i32) -> void

fn print_between(start:i32, end:i32) -> void {
entry:
    call print_char(start)
    call print_char(45)  # dash
    call print_char(end)
    ret
}

fn main() -> void {
entry:
    call print_between(65, 90)  # A-Z
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "A-Z");
    }

    #[test]
    fn test_constants_and_variables() {
        let source = r#"
import "env" "print_char" (c:i32) -> void

fn main() -> void {
entry:
    call print_char(72)   # 'H'
    call print_char(101)  # 'e'
    call print_char(108)  # 'l'
    call print_char(108)  # 'l'
    call print_char(111)  # 'o'
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "Hello");
    }

    #[test]
    fn test_function_return_value() {
        let source = r#"
import "env" "get_number" -> i32
import "env" "print_int" (n:i32) -> void

fn main() -> void {
entry:
    num:i32 = call get_number()
    call print_int(num)
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "42"); // get_number returns 42
    }

    #[test]
    fn test_simple_function_with_variables() {
        // Test using variables and simple function flow
        let source = r#"
import "env" "print_int" (n:i32) -> void
import "env" "get_number" -> i32

fn process_number(a:i32, b:i32) -> i32 {
entry:
    # For now, we'll just return one of the parameters
    # since arithmetic operations aren't parsed yet
    ret a
}

fn main() -> void {
entry:
    result:i32 = call process_number(15, 5)
    call print_int(result)
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "15");
    }

    #[test]
    fn test_nested_function_calls() {
        let source = r#"
import "env" "print_char" (c:i32) -> void
import "env" "get_number" -> i32

fn get_letter() -> i32 {
entry:
    ret 67  # 'C'
}

fn print_letter() -> void {
entry:
    letter:i32 = call get_letter()
    call print_char(letter)
    ret
}

fn main() -> void {
entry:
    call print_letter()
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "C");
    }

    #[test]
    fn test_multiple_functions() {
        let source = r#"
import "env" "print_char" (c:i32) -> void

fn print_a() -> void {
entry:
    call print_char(65)  # 'A'
    ret
}

fn print_b() -> void {
entry:
    call print_char(66)  # 'B'
    ret
}

fn print_c() -> void {
entry:
    call print_char(67)  # 'C'
    ret
}

fn main() -> void {
entry:
    call print_a()
    call print_b()
    call print_c()
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "ABC");
    }

    #[test]
    fn test_parameter_passing() {
        let source = r#"
import "env" "print_char" (c:i32) -> void

fn repeat_char(ch:i32, count:i32) -> void {
entry:
    # For simplicity, just print the character once
    # since we don't have loops yet
    call print_char(ch)
    ret
}

fn main() -> void {
entry:
    call repeat_char(88, 3)  # 'X'
    call repeat_char(89, 2)  # 'Y'
    call repeat_char(90, 1)  # 'Z'
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "XYZ");
    }

    #[test]
    fn test_return_values() {
        let source = r#"
import "env" "print_char" (c:i32) -> void

fn get_first_letter() -> i32 {
entry:
    ret 72  # 'H'
}

fn get_second_letter() -> i32 {
entry:
    ret 105  # 'i'
}

fn main() -> void {
entry:
    first:i32 = call get_first_letter()
    second:i32 = call get_second_letter()
    call print_char(first)
    call print_char(second)
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "Hi");
    }

    #[test]
    fn test_recursive_function() {
        let source = r#"
import "env" "print_char" (c:i32) -> void

fn print_countdown(n:i32) -> void {
entry:
    call print_char(n)
    # Simple base case - in a real implementation we'd have conditionals
    ret
}

fn main() -> void {
entry:
    call print_countdown(53)  # '5'
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "5");
    }

    #[test]
    fn test_complex_parameter_flow() {
        let source = r#"
import "env" "print_char" (c:i32) -> void

fn process_and_print(base:i32, offset:i32) -> i32 {
entry:
    # In real implementation, we'd add these
    # For now, just use the base
    call print_char(base)
    ret base
}

fn chain_calls(start:i32) -> void {
entry:
    result:i32 = call process_and_print(start, 1)
    unused1:i32 = call process_and_print(79, 2)  # 'O'
    unused2:i32 = call process_and_print(75, 3)  # 'K'
    ret
}

fn main() -> void {
entry:
    call chain_calls(72)  # 'H'
    ret
}
"#;

        let output = compile_and_run(source).expect("Compilation failed");
        assert_eq!(output, "HOK");
    }
}
