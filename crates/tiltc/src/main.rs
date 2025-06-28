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
use std::env;
use std::fs;

fn main() {
    println!("--- TILT COMPILER with JIT EXECUTION ---");
    
    // Get command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <file.tilt>", args[0]);
        return;
    }
    
    let filename = &args[1];
    
    // Read the source file
    let source = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", filename, e);
            return;
        }
    };
    
    println!("Compiling and executing source code:\n{}\n", source);

    match compile_and_run(&source) {
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
