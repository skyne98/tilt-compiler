// ===================================================================
// FILE: main.rs
//
// DESC: The main driver for the TILT compiler with full JIT compilation
//       pipeline: lexing, parsing, semantic analysis, IR generation,
//       and native code execution.
// ===================================================================

use tilt_parser::{Token, ProgramParser};
use tilt_ir::lower_program;
use tilt_codegen_cranelift::JIT;
use logos::Logos;
use std::mem;

// Our sample TILT program, demonstrating basic features for the minimal parser.
const TILT_CODE: &str = r#"
# Import host functions with parameters
import "env" "print_hello" -> void
import "env" "print_char" (c:i32) -> void

# Function that takes multiple parameters
fn print_between(start:i32, end:i32) -> void {
entry:
    call print_char(start)
    call print_char(45)  # dash character
    call print_char(end)
    ret
}

# Main entry point.
fn main() -> void {
entry:
    call print_between(65, 90)  # Print A-Z
    call print_hello()
    ret
}
"#;

fn main() {
    println!("--- TILT COMPILER with JIT EXECUTION ---");
    println!("Compiling and executing source code:\n{}\n", TILT_CODE);

    match compile_and_run(TILT_CODE) {
        Ok(_) => println!("\n--- EXECUTION COMPLETED SUCCESSFULLY ---"),
        Err(e) => eprintln!("ERROR: {}", e),
    }
}

fn compile_and_run(source: &str) -> Result<(), String> {
    // 1. Lexing Stage (logos)
    let lexer = Token::lexer(source);
    
    // Collect tokens for display and parsing - handle Result from logos
    let tokens: Result<Vec<Token>, _> = lexer.collect();
    
    let tokens = tokens.map_err(|e| format!("Lexing failed: {:?}", e))?;
    
    println!("--- Lexer Output (Token Stream) ---");
    for (i, token) in tokens.iter().enumerate() {
        println!("{}: {:?}", i, token);
    }
    println!();

    // 2. Parsing Stage (LALRPOP)
    let parser = ProgramParser::new();
    
    // For LALRPOP, we need to create an iterator of (usize, Token, usize) 
    // representing (start_pos, token, end_pos)
    let token_iter = tokens.into_iter().enumerate().map(|(i, token)| {
        Ok((i, token, i + 1))
    });
    
    let program_ast = parser.parse(token_iter)
        .map_err(|e| format!("Parsing failed: {:?}", e))?;
    
    println!("--- Parser Output (AST) ---");
    println!("{:#?}", program_ast);
    println!();

    // 3. Semantic Analysis & IR Generation
    let program_ir = lower_program(&program_ast)
        .map_err(|errors| {
            let mut error_msg = String::from("Semantic errors:\n");
            for error in errors {
                error_msg.push_str(&format!("  {}\n", error));
            }
            error_msg
        })?;
    
    println!("--- IR Output (Validated IR) ---");
    println!("{:#?}", program_ir);
    println!();

    // 4. JIT Compilation
    println!("--- JIT Compilation ---");
    let mut jit = JIT::new().map_err(|e| format!("Failed to create JIT: {}", e))?;
    
    jit.compile(&program_ir).map_err(|e| format!("JIT compilation failed: {}", e))?;
    println!("JIT compilation successful!");
    
    // 5. Execute the compiled code
    println!("\n--- Executing Compiled Code ---");
    let main_ptr = jit.get_func_ptr("main")
        .ok_or("Main function not found in compiled code")?;

    // Transmute the raw pointer into a safe Rust function type and call it!
    unsafe {
        let main_fn = mem::transmute::<*const u8, fn()>(main_ptr);
        println!("Calling main()...");
        main_fn();
    }
    
    Ok(())
}
