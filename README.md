# TILT Compiler

A compiler for the TILT intermediate language, built in Rust with a modular architecture.

## Overview

The TILT compiler is designed as a robust, modular compiler front-end that parses TILT intermediate language code and builds an Abstract Syntax Tree (AST). The project demonstrates modern Rust compiler construction techniques using LALRPOP for parsing and Logos for lexical analysis.

## Architecture

The project is structured as a Rust workspace with three main crates:

### Core Crates

- **`tilt-ast`** - Defines the Abstract Syntax Tree structures for the TILT language
- **`tilt-parser`** - Implements lexing (with Logos) and parsing (with LALRPOP)
- **`tiltc`** - The main compiler binary that orchestrates the compilation process

### Key Features

- **Clean separation of concerns**: Lexing, parsing, and AST are separate modules
- **Generated parser**: Uses LALRPOP for robust LR(1) parsing
- **Modern Rust**: Leverages the Rust type system for safety and performance
- **Extensible design**: Easy to add new language features and optimizations

## Building

```bash
# Build the entire workspace
cargo build

# Build with optimizations
cargo build --release

# Run the compiler
cargo run
```

## Project Structure

```
tilt-compiler/
├── Cargo.toml          # Workspace configuration
├── crates/
│   ├── tilt-ast/       # AST definitions
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── tilt-parser/    # Lexer and parser
│   │   ├── Cargo.toml
│   │   ├── build.rs    # LALRPOP build script
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── lexer.rs      # Logos-based lexer
│   │       └── tilt.lalrpop  # LALRPOP grammar
│   └── tiltc/          # Main compiler binary
│       ├── Cargo.toml
│       └── src/main.rs
└── target/             # Build artifacts (gitignored)
```

## TILT Language

The TILT language is an intermediate representation with the following features:

### Supported Constructs

- **Import declarations**: `import "module" "function" -> type`
- **Function definitions**: `fn name() -> type { blocks }`
- **Basic blocks**: `label: instructions terminator`
- **Instructions**: Assignment and operation calls
- **Terminators**: `ret`, `br label`, `br_if condition, true_label, false_label`
- **Types**: `i32`, `i64`, `f32`, `f64`, `void`
- **Values**: Constants and variables

### Example Program

```tilt
# Import a host function
import "env" "putc" -> void

# Main function
fn main() -> void {
entry:
    result:i32 = call putc()
    ret
}
```

## Development

### Adding New Language Features

1. **Update AST**: Add new node types in `tilt-ast/src/lib.rs`
2. **Update Lexer**: Add new tokens in `tilt-parser/src/lexer.rs`
3. **Update Grammar**: Add new rules in `tilt-parser/src/tilt.lalrpop`
4. **Test**: Update examples in `tiltc/src/main.rs`

### Tools Used

- **[LALRPOP](https://github.com/lalrpop/lalrpop)** - Parser generator for Rust
- **[Logos](https://github.com/maciejhirsz/logos)** - Fast lexer generator
- **[Cargo](https://doc.rust-lang.org/cargo/)** - Rust's build system and package manager

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.
