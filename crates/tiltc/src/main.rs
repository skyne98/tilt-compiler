// ===================================================================
// FILE: main.rs
//
// DESC: The main driver for the TILT compiler using LALRPOP parser
//       and semantic analysis with IR generation.
// ===================================================================

use tilt_parser::{Token, ProgramParser};
use tilt_ir::lower_program;
use logos::Logos;

// Our sample TILT program, demonstrating basic features for the minimal parser.
const TILT_CODE: &str = r#"
# Import host functions
import "env" "putc" -> void
import "env" "getc" -> i32

# Main entry point.
fn main() -> void {
entry:
    result:i32 = call getc()
    call putc()
    ret
}
"#;

fn main() {
    println!("--- TILT COMPILER with LALRPOP and IR ---");
    println!("Parsing source code:\n{}\n", TILT_CODE);

    // 1. Lexing Stage (logos)
    let lexer = Token::lexer(TILT_CODE);
    
    // Collect tokens for display and parsing - handle Result from logos
    let tokens: Result<Vec<Token>, _> = lexer.collect();
    
    match tokens {
        Ok(tokens) => {
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
            
            match parser.parse(token_iter) {
                Ok(program_ast) => {
                    println!("--- Parser Output (AST) ---");
                    println!("{:#?}", program_ast);
                    println!();

                    // 3. Semantic Analysis & IR Generation
                    match lower_program(&program_ast) {
                        Ok(program_ir) => {
                            println!("--- IR Output (Validated IR) ---");
                            println!("{:#?}", program_ir);
                            println!("\nSUCCESS: Program compiled to IR successfully!");
                        }
                        Err(errors) => {
                            eprintln!("--- Semantic Errors ---");
                            for error in errors {
                                eprintln!("ERROR: {}", error);
                            }
                            eprintln!("\nFAILED: Program has semantic errors.");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("ERROR: Failed to parse program.");
                    eprintln!("{:?}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("ERROR: Lexing failed.");
            eprintln!("{:?}", e);
        }
    }
}
