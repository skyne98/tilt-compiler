// ===================================================================
// FILE: main.rs
//
// DESC: Advanced TILT compiler and REPL with comprehensive debugging
//       features including AST, IR, and Cranelift IR visualization.
// ===================================================================

use clap::{Arg, Command};
use colored::*;
use logos::Logos;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::collections::HashMap;
use std::fs;
use std::mem;

use tilt_ast::Type;
use tilt_codegen_cranelift::JIT;
use tilt_host_abi::{MemoryHostABI, RuntimeValue};
use tilt_ir::{lowering::lower_program, Program};
use tilt_parser::{lexer::Token, tilt::ProgramParser};
use tilt_vm::VM;

#[derive(Debug, Clone)]
struct CompilerOptions {
    show_tokens: bool,
    show_ast: bool,
    show_ir: bool,
    show_cranelift_ir: bool,
    use_vm: bool,
    use_jit: bool,
    verbose: bool,
    measure_time: bool,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            show_tokens: false,
            show_ast: false,
            show_ir: false,
            show_cranelift_ir: false,
            use_vm: true,
            use_jit: false,
            verbose: false,
            measure_time: false,
        }
    }
}

fn main() {
    let matches = Command::new("tiltc")
        .version("1.0.0")
        .author("TILT Language Team")
        .about("The TILT Language Compiler and REPL")
        .arg(
            Arg::new("file")
                .help("TILT source file to compile and execute")
                .value_name("FILE")
                .index(1),
        )
        .arg(
            Arg::new("repl")
                .short('r')
                .long("repl")
                .help("Start interactive REPL mode")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("show-tokens")
                .long("show-tokens")
                .help("Display lexer token stream")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("show-ast")
                .long("show-ast")
                .help("Display parser AST")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("show-ir")
                .long("show-ir")
                .help("Display TILT IR")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("show-cranelift-ir")
                .long("show-cranelift-ir")
                .help("Display Cranelift IR")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("show-all")
                .long("show-all")
                .help("Display all compiler stages")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("vm")
                .long("vm")
                .help("Use VM backend (default)")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("jit")
                .long("jit")
                .help("Use JIT backend")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("both")
                .long("both")
                .help("Use both VM and JIT backends for comparison")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("time")
                .short('t')
                .long("time")
                .help("Measure compilation and execution time")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let mut options = CompilerOptions::default();

    // Parse command line options
    options.show_tokens = matches.get_flag("show-tokens") || matches.get_flag("show-all");
    options.show_ast = matches.get_flag("show-ast") || matches.get_flag("show-all");
    options.show_ir = matches.get_flag("show-ir") || matches.get_flag("show-all");
    options.show_cranelift_ir =
        matches.get_flag("show-cranelift-ir") || matches.get_flag("show-all");
    options.verbose = matches.get_flag("verbose");
    options.measure_time = matches.get_flag("time");

    // Determine execution backend
    if matches.get_flag("both") {
        options.use_vm = true;
        options.use_jit = true;
    } else if matches.get_flag("jit") {
        options.use_vm = false;
        options.use_jit = true;
    } else {
        options.use_vm = true;
        options.use_jit = false;
    }

    print_banner();

    if matches.get_flag("repl") || matches.get_one::<String>("file").is_none() {
        start_repl(options);
    } else {
        let filename = matches.get_one::<String>("file").unwrap();
        compile_and_run_file(filename, options);
    }
}

fn print_banner() {
    println!(
        "{}",
        "╔═══════════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║                    🚀 TILT COMPILER 1.0 🚀                    ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║          Typed Intermediate Language with Tensors             ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║                                                               ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║  Features: Memory Management | Pointer Arithmetic | C FFI     ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║  Backends: VM Interpreter | JIT Compiler (Cranelift)          ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();
}

fn start_repl(mut options: CompilerOptions) {
    println!(
        "{}",
        "🔥 Welcome to the TILT Interactive REPL!".green().bold()
    );
    println!("{}", "Type 'help' for commands, 'quit' to exit.".green());
    println!();

    let mut rl = DefaultEditor::new().unwrap();
    let mut session_vars: HashMap<String, RuntimeValue> = HashMap::new();
    let mut line_count = 0;

    loop {
        line_count += 1;
        // Use a simple prompt without colors to avoid rustyline issues
        let prompt = format!("tilt[{}]> ", line_count);

        let readline = rl.readline(&prompt);
        match readline {
            Ok(line) => {
                let line = line.trim();

                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(line).unwrap();

                // Handle REPL commands
                if handle_repl_command(line, &mut options, &session_vars) {
                    continue;
                }

                // Handle quit command
                if line == "quit" || line == "exit" {
                    println!("{}", "👋 Goodbye!".green().bold());
                    break;
                }

                // Try to execute as TILT code
                execute_repl_line(line, &options, &mut session_vars);
            }
            Err(ReadlineError::Interrupted) => {
                println!("{}", "^C".yellow());
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("{}", "👋 Goodbye!".green().bold());
                break;
            }
            Err(err) => {
                println!("{} {:?}", "Error:".red().bold(), err);
                break;
            }
        }
    }
}

fn handle_repl_command(
    line: &str,
    options: &mut CompilerOptions,
    vars: &HashMap<String, RuntimeValue>,
) -> bool {
    match line {
        "help" => {
            print_help();
            true
        }
        "options" => {
            print_current_options(options);
            true
        }
        "vars" => {
            print_session_vars(vars);
            true
        }
        "clear" => {
            // Clear screen
            print!("\x1B[2J\x1B[1;1H");
            print_banner();
            true
        }
        line if line.starts_with("set ") => {
            handle_set_command(line, options);
            true
        }
        "example" => {
            show_example_code();
            true
        }
        _ => false,
    }
}

fn print_help() {
    println!("{}", "📚 TILT REPL Commands:".blue().bold());
    println!("  {}           - Show this help message", "help".green());
    println!(
        "  {}           - Show current compiler options",
        "options".green()
    );
    println!("  {}           - Show session variables", "vars".green());
    println!("  {}          - Clear the screen", "clear".green());
    println!("  {}        - Show example TILT code", "example".green());
    println!("  {}      - Exit the REPL", "quit/exit".green());
    println!();
    println!(
        "{}",
        "🔧 Settings (use 'set <option> <value>'):".blue().bold()
    );
    println!(
        "  {} tokens <true|false>     - Show/hide token stream",
        "set".green()
    );
    println!(
        "  {} ast <true|false>        - Show/hide AST",
        "set".green()
    );
    println!("  {} ir <true|false>         - Show/hide IR", "set".green());
    println!(
        "  {} cranelift <true|false>  - Show/hide Cranelift IR",
        "set".green()
    );
    println!(
        "  {} backend <vm|jit|both>   - Choose execution backend",
        "set".green()
    );
    println!(
        "  {} verbose <true|false>    - Enable/disable verbose output",
        "set".green()
    );
    println!(
        "  {} time <true|false>       - Enable/disable timing",
        "set".green()
    );
    println!();
    println!("{}", "💡 TILT Code Examples:".blue().bold());
    println!("  Type a complete function definition or single expression");
    println!("  Use 'example' to see sample code");
    println!();
}

fn print_current_options(options: &CompilerOptions) {
    println!("{}", "⚙️  Current Compiler Options:".blue().bold());
    println!("  Show tokens:      {}", format_bool(options.show_tokens));
    println!("  Show AST:         {}", format_bool(options.show_ast));
    println!("  Show IR:          {}", format_bool(options.show_ir));
    println!(
        "  Show Cranelift:   {}",
        format_bool(options.show_cranelift_ir)
    );
    println!("  Use VM:           {}", format_bool(options.use_vm));
    println!("  Use JIT:          {}", format_bool(options.use_jit));
    println!("  Verbose:          {}", format_bool(options.verbose));
    println!("  Measure time:     {}", format_bool(options.measure_time));
    println!();
}

fn print_session_vars(vars: &HashMap<String, RuntimeValue>) {
    if vars.is_empty() {
        println!("{}", "📝 No session variables defined yet.".yellow());
    } else {
        println!("{}", "📝 Session Variables:".blue().bold());
        for (name, value) in vars {
            println!("  {}: {:?}", name.green(), value);
        }
    }
    println!();
}

fn format_bool(b: bool) -> colored::ColoredString {
    if b {
        "true".green()
    } else {
        "false".red()
    }
}

fn handle_set_command(line: &str, options: &mut CompilerOptions) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() != 3 {
        println!("{} Usage: set <option> <value>", "Error:".red().bold());
        return;
    }

    let option = parts[1];
    let value = parts[2];

    match option {
        "tokens" => {
            options.show_tokens = parse_bool(value);
            println!(
                "{} Show tokens: {}",
                "✓".green(),
                format_bool(options.show_tokens)
            );
        }
        "ast" => {
            options.show_ast = parse_bool(value);
            println!(
                "{} Show AST: {}",
                "✓".green(),
                format_bool(options.show_ast)
            );
        }
        "ir" => {
            options.show_ir = parse_bool(value);
            println!("{} Show IR: {}", "✓".green(), format_bool(options.show_ir));
        }
        "cranelift" => {
            options.show_cranelift_ir = parse_bool(value);
            println!(
                "{} Show Cranelift IR: {}",
                "✓".green(),
                format_bool(options.show_cranelift_ir)
            );
        }
        "backend" => match value {
            "vm" => {
                options.use_vm = true;
                options.use_jit = false;
                println!("{} Backend: VM", "✓".green());
            }
            "jit" => {
                options.use_vm = false;
                options.use_jit = true;
                println!("{} Backend: JIT", "✓".green());
            }
            "both" => {
                options.use_vm = true;
                options.use_jit = true;
                println!("{} Backend: Both (VM and JIT)", "✓".green());
            }
            _ => {
                println!(
                    "{} Invalid backend. Use: vm, jit, or both",
                    "Error:".red().bold()
                );
            }
        },
        "verbose" => {
            options.verbose = parse_bool(value);
            println!("{} Verbose: {}", "✓".green(), format_bool(options.verbose));
        }
        "time" => {
            options.measure_time = parse_bool(value);
            println!(
                "{} Measure time: {}",
                "✓".green(),
                format_bool(options.measure_time)
            );
        }
        _ => {
            println!("{} Unknown option: {}", "Error:".red().bold(), option);
        }
    }
}

fn parse_bool(s: &str) -> bool {
    matches!(s.to_lowercase().as_str(), "true" | "yes" | "1" | "on")
}

fn show_example_code() {
    println!("{}", "💡 Example TILT Code:".blue().bold());
    println!();
    println!("{}", "# Simple arithmetic:".green());
    println!("fn add_numbers() -> i32 {{");
    println!("entry:");
    println!("    a:i32 = i32.const(10)");
    println!("    b:i32 = i32.const(20)");
    println!("    result:i32 = i32.add(a, b)");
    println!("    ret (result)");
    println!("}}");
    println!();
    println!("{}", "# Memory operations:".green());
    println!("import \"host\" \"alloc\" (size: i64) -> ptr");
    println!("import \"host\" \"free\" (p: ptr) -> void");
    println!();
    println!("fn memory_example() -> i32 {{");
    println!("entry:");
    println!("    size:i64 = i64.const(4)");
    println!("    ptr:ptr = call alloc(size)");
    println!("    value:i32 = i32.const(42)");
    println!("    i32.store(ptr, value)");
    println!("    loaded:i32 = i32.load(ptr)");
    println!("    free(ptr)");
    println!("    ret (loaded)");
    println!("}}");
    println!();
}

fn execute_repl_line(
    line: &str,
    options: &CompilerOptions,
    _vars: &mut HashMap<String, RuntimeValue>,
) {
    let start_time = std::time::Instant::now();

    // If it's a single function, wrap it in a complete program
    let source = if line.trim().starts_with("fn ") {
        line.to_string()
    } else {
        // Try to parse as an expression and wrap in a main function
        format!(
            "fn main() -> i32 {{\nentry:\n    result:i32 = {}\n    ret (result)\n}}",
            line
        )
    };

    match compile_and_execute(&source, options) {
        Ok(result) => {
            if let Some(value) = result {
                println!("{} {:?}", "Result:".green().bold(), value);
            } else {
                println!("{}", "✓ Executed successfully".green());
            }

            if options.measure_time {
                let elapsed = start_time.elapsed();
                println!("{} {:?}", "Time:".blue(), elapsed);
            }
        }
        Err(e) => {
            println!("{} {}", "Error:".red().bold(), e);
        }
    }
    println!();
}

fn compile_and_run_file(filename: &str, options: CompilerOptions) {
    if options.verbose {
        println!("{} {}", "📁 Loading file:".blue().bold(), filename.yellow());
    }

    let source = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => {
            eprintln!(
                "{} Failed to read file '{}': {}",
                "Error:".red().bold(),
                filename,
                e
            );
            return;
        }
    };

    if options.verbose {
        println!("{}", "📝 Source code:".blue().bold());
        println!("{}", source.dimmed());
        println!();
    }

    let start_time = std::time::Instant::now();

    match compile_and_execute(&source, &options) {
        Ok(result) => {
            if let Some(value) = result {
                println!("{} {:?}", "Final result:".green().bold(), value);
            } else {
                println!("{}", "✓ Program executed successfully".green().bold());
            }

            if options.measure_time {
                let elapsed = start_time.elapsed();
                println!("{} {:?}", "Total execution time:".blue().bold(), elapsed);
            }
        }
        Err(e) => {
            eprintln!("{} {}", "Compilation/execution failed:".red().bold(), e);
            std::process::exit(1);
        }
    }
}

fn compile_and_execute(
    source: &str,
    options: &CompilerOptions,
) -> Result<Option<RuntimeValue>, String> {
    let compilation_start = std::time::Instant::now();

    // Step 1: Lexing
    if options.verbose {
        println!("{}", "🔍 Step 1: Lexical Analysis...".blue().bold());
    }

    let tokens = tokenize_with_positions(source)?;

    if options.show_tokens {
        print_tokens(&tokens);
    }

    // Step 2: Parsing
    if options.verbose {
        println!("{}", "🔍 Step 2: Parsing...".blue().bold());
    }

    let parser = ProgramParser::new();
    let ast = parser
        .parse(tokens)
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    if options.show_ast {
        print_ast(&ast);
    }

    // Step 3: IR Lowering
    if options.verbose {
        println!("{}", "🔍 Step 3: IR Generation...".blue().bold());
    }

    let ir_program = lower_program(&ast).map_err(|errors| {
        let mut error_msg = "Semantic analysis failed:\n".to_string();
        for error in &errors {
            error_msg.push_str(&format!("  • {}\n", error));
        }
        error_msg
    })?;

    if options.show_ir {
        print_ir(&ir_program);
    }

    let compilation_time = compilation_start.elapsed();
    if options.measure_time {
        println!("{} {:?}", "Compilation time:".blue(), compilation_time);
    }

    // Step 4: Execution
    let execution_start = std::time::Instant::now();
    let mut results = Vec::new();

    if options.use_vm {
        if options.verbose {
            println!("{}", "🔍 Step 4a: VM Execution...".blue().bold());
        }

        let vm_result = execute_with_vm(&ir_program)?;

        if options.use_jit {
            results.push(("VM", vm_result.clone()));
        } else {
            let execution_time = execution_start.elapsed();
            if options.measure_time {
                println!("{} {:?}", "VM execution time:".blue(), execution_time);
            }
            return Ok(Some(vm_result));
        }
    }

    if options.use_jit {
        if options.verbose {
            println!(
                "{}",
                "🔍 Step 4b: JIT Compilation and Execution...".blue().bold()
            );
        }

        let jit_result = execute_with_jit(&ir_program, options)?;

        if options.use_vm {
            results.push(("JIT", jit_result));
        } else {
            let execution_time = execution_start.elapsed();
            if options.measure_time {
                println!("{} {:?}", "JIT execution time:".blue(), execution_time);
            }
            return Ok(Some(jit_result));
        }
    }

    // Compare results if both backends were used
    if results.len() == 2 {
        let execution_time = execution_start.elapsed();
        if options.measure_time {
            println!("{} {:?}", "Total execution time:".blue(), execution_time);
        }

        println!("{}", "🔍 Backend Comparison:".blue().bold());
        for (backend, result) in &results {
            println!("  {}: {:?}", backend.yellow(), result);
        }

        if results[0].1 == results[1].1 {
            println!("{} Results match!", "✓".green().bold());
        } else {
            println!("{} Results differ!", "⚠".yellow().bold());
        }

        return Ok(Some(results[0].1.clone()));
    }

    Ok(None)
}

fn tokenize_with_positions(input: &str) -> Result<Vec<(usize, Token, usize)>, String> {
    let mut lexer = Token::lexer(input);
    let mut tokens = Vec::new();

    while let Some(token) = lexer.next() {
        let token = token.map_err(|_| "Lexing error")?;
        let span = lexer.span();
        tokens.push((span.start, token, span.end));
    }
    Ok(tokens)
}

fn print_tokens(tokens: &[(usize, Token, usize)]) {
    println!("{}", "🔤 Token Stream:".blue().bold());
    println!(
        "{}",
        "┌─────┬──────────────┬──────────────────────────────┐".blue()
    );
    println!(
        "{}",
        "│ Pos │     Span     │            Token             │".blue()
    );
    println!(
        "{}",
        "├─────┼──────────────┼──────────────────────────────┤".blue()
    );

    for (i, (start, token, end)) in tokens.iter().enumerate() {
        println!(
            "│{:4} │ {:4}..{:<6} │ {:<28} │",
            i.to_string().yellow(),
            start.to_string().cyan(),
            end.to_string().cyan(),
            format!("{:?}", token).green()
        );
    }

    println!(
        "{}",
        "└─────┴──────────────┴──────────────────────────────┘".blue()
    );
    println!();
}

fn print_ast(ast: &tilt_ast::Program) {
    println!("{}", "🌳 Abstract Syntax Tree:".blue().bold());
    println!("{}", format!("{:#?}", ast).green());
    println!();
}

fn print_ir(ir: &Program) {
    println!("{}", "⚙️  TILT Intermediate Representation:".blue().bold());
    println!("{}", format!("{:#?}", ir).green());
    println!();
}

fn execute_with_vm(program: &Program) -> Result<RuntimeValue, String> {
    let host_abi = MemoryHostABI::new();
    let mut vm = VM::new(program.clone(), host_abi);

    // Try to find and execute main function
    let result = vm
        .call_function("main", vec![])
        .map_err(|e| format!("VM execution failed: {:?}", e))?;

    Ok(result)
}

fn execute_with_jit(program: &Program, options: &CompilerOptions) -> Result<RuntimeValue, String> {
    let host_abi = Box::new(tilt_host_abi::JITMemoryHostABI::new());
    let mut jit =
        JIT::new_with_abi(host_abi).map_err(|e| format!("Failed to create JIT: {}", e))?;

    // Enable Cranelift IR display if requested
    if options.show_cranelift_ir {
        jit.set_show_cranelift_ir(true);
    }

    jit.compile(program)
        .map_err(|e| format!("JIT compilation failed: {}", e))?;

    // Get the main function pointer
    let main_ptr = jit
        .get_func_ptr("main")
        .ok_or("Main function not found in JIT compiled code")?;

    // Find the main function to check its return type
    let main_function = program
        .functions
        .iter()
        .find(|f| f.name == "main")
        .ok_or("Main function not found in program")?;

    // Execute the function based on its return type
    unsafe {
        match main_function.return_type {
            Type::I32 => {
                let main_fn = mem::transmute::<*const u8, fn() -> i32>(main_ptr);
                let result = main_fn();
                Ok(RuntimeValue::I32(result))
            }
            Type::I64 => {
                let main_fn = mem::transmute::<*const u8, fn() -> i64>(main_ptr);
                let result = main_fn();
                Ok(RuntimeValue::I64(result))
            }
            Type::Void => {
                let main_fn = mem::transmute::<*const u8, fn()>(main_ptr);
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
