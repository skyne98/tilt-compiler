// ===================================================================
// FILE: main.rs
//
// DESC: The main driver for the TILT compiler using LALRPOP parser.
// ===================================================================

use tilt_parser::{Token, ProgramParser};
use logos::Logos;

// Our sample TILT program, demonstrating basic features for the minimal parser.
const TILT_CODE: &str = r#"
# Import the host function for printing a character.
import "env" "putc" -> void

# Main entry point.
fn main() -> void {
entry:
    result:i32 = call putc()
    ret
}
"#;

fn main() {
    println!("--- TILT COMPILER with LALRPOP ---");
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
                    println!("\nSUCCESS: Program parsed successfully with LALRPOP!");
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
