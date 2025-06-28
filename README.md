# TILT Compiler

A modern, multi-backend compiler for the TILT intermediate language, featuring comprehensive memory management, pointer arithmetic, and C FFI capabilities.

## Overview

TILT (Typed Intermediate Language with Tensors) is a low-level intermediate language designed for systems programming with strong typing and explicit memory management. The compiler provides multiple execution backends including a virtual machine (VM) and a JIT compiler powered by Cranelift.

## Language Design Philosophy

TILT follows a unified expression-based design where **everything is an expression**:

- Operations that return values can be used in assignments
- Operations that return `void` can be used as statements
- No artificial distinction between "statements" and "expressions"
- Consistent parenthesized syntax for all operations
- Strong static typing with explicit type annotations

## Architecture

The project is structured as a Rust workspace with multiple specialized crates:

### Core Language Infrastructure

- **`tilt-ast`** - Abstract Syntax Tree definitions and type system
- **`tilt-parser`** - Lexical analysis (Logos) and parsing (LALRPOP)
- **`tilt-ir`** - Intermediate representation and semantic analysis
- **`tilt-ir-builder`** - Programmatic IR construction API

### Execution Backends

- **`tilt-vm`** - Stack-based virtual machine interpreter
- **`tilt-codegen-cranelift`** - JIT compiler using Cranelift backend
- **`tilt-host-abi`** - Host function interface and memory management

### Tooling

- **`tiltc`** - Command-line compiler interface
- **`tilt-integration-tests`** - Comprehensive end-to-end tests

## Language Features

### Type System

```tilt
# Primitive types
i32    # 32-bit signed integer
i64    # 64-bit signed integer  
f32    # 32-bit floating point
f64    # 64-bit floating point
ptr    # Pointer type (platform word size)
void   # No value (for functions/operations with side effects)
```

### Memory Management

TILT provides explicit, low-level memory operations:

```tilt
# Memory allocation and deallocation
array_ptr:ptr = call alloc(size)  # Allocate memory
free(array_ptr)                   # Free memory

# Memory load/store operations
i32.store(ptr, value)             # Store value to memory
value:i32 = i32.load(ptr)         # Load value from memory

# Pointer arithmetic
new_ptr:ptr = ptr.add(ptr, offset) # Add offset to pointer
size:i64 = i32.sizeof()           # Get type size
```

### Function System

```tilt
# Import declarations with optional calling conventions
import "host" "alloc" (size: i64) -> ptr
import "libc" "printf" "c" (format: ptr, ...) -> i32

# Function definitions
fn calculate_sum(a: i32, b: i32) -> i32 {
entry:
    result:i32 = i32.add(a, b)
    ret (result)
}
```

### Arithmetic and Logic Operations

```tilt
# All operations use consistent parenthesized syntax
sum:i32 = i32.add(a, b)           # Addition
diff:i32 = i32.sub(a, b)          # Subtraction  
product:i32 = i32.mul(a, b)       # Multiplication

# Comparison operations (return i32: 1 for true, 0 for false)
equal:i32 = i32.eq(a, b)          # Equality
less:i32 = i32.lt(a, b)           # Less than

# Constants
value:i32 = i32.const(42)         # Integer constant
```

### Control Flow

```tilt
# Basic blocks with labels
fn conditional_example() -> i32 {
entry:
    condition:i32 = i32.const(1)
    br_if condition, true_block, false_block

true_block:
    result:i32 = i32.const(100)
    br final_block

false_block:
    result:i32 = i32.const(200)
    br final_block

final_block:
    ret (result)
}
```

## Example: Complete Memory Management Program

```tilt
import "host" "alloc" (size: i64) -> ptr
import "host" "free" (p: ptr) -> void

fn array_processing() -> i32 {
entry:
    # Allocate array for 4 i32 values (16 bytes)
    array_size:i64 = i64.const(16)
    array_ptr:ptr = call alloc(array_size)
    
    # Store values at different array positions
    val1:i32 = i32.const(10)
    val2:i32 = i32.const(20)
    
    i32.store(array_ptr, val1)
    
    # Calculate offset for second element
    offset:i64 = i64.const(4)
    ptr2:ptr = ptr.add(array_ptr, offset)
    i32.store(ptr2, val2)
    
    # Load values back and compute sum
    loaded1:i32 = i32.load(array_ptr)
    loaded2:i32 = i32.load(ptr2)
    sum:i32 = i32.add(loaded1, loaded2)
    
    # Clean up memory
    free(array_ptr)
    
    ret (sum)
}
```

## Building and Usage

```bash
# Build the entire workspace
cargo build

# Run all tests (including comprehensive integration tests)
cargo test

# Run with optimizations
cargo build --release

# Run the compiler
cargo run

# Run comprehensive test suite with output
cargo test -- --nocapture
```

## Project Structure

```
tilt-compiler/
├── Cargo.toml                    # Workspace configuration
├── examples/
│   ├── example_memory.tilt       # Example TILT program
│   └── README.md                 # Example documentation
├── crates/
│   ├── tilt-ast/                 # AST and type definitions
│   ├── tilt-parser/              # Lexer and parser (LALRPOP + Logos)
│   ├── tilt-ir/                  # IR and semantic analysis
│   ├── tilt-ir-builder/          # Programmatic IR construction
│   ├── tilt-vm/                  # Virtual machine interpreter
│   ├── tilt-codegen-cranelift/   # JIT compiler backend
│   ├── tilt-host-abi/            # Host function interface
│   ├── tilt-integration-tests/   # End-to-end integration tests
│   └── tiltc/                    # Main compiler binary
└── target/                       # Build artifacts
```

## Testing

The project includes comprehensive test coverage:

- **Unit tests** in each crate for individual components
- **Integration tests** for cross-crate functionality  
- **End-to-end tests** exercising the complete compilation pipeline
- **VM/JIT compatibility tests** ensuring backend consistency
- **Memory operation tests** validating low-level operations

Key test files:
- `crates/tilt-integration-tests/src/memory_test.rs` - Comprehensive feature tests
- `crates/tilt-integration-tests/src/vm_jit_compatibility.rs` - Backend compatibility

## Implementation Highlights

### Unified Expression System

TILT implements a clean expression-based design:

- **Before**: `i32.store ptr, value` (special syntax)
- **After**: `i32.store(ptr, value)` (consistent with all operations)

- **Before**: Separate AST nodes for calls vs. statements
- **After**: Single `ExpressionStatement` for void expressions

### Memory Safety

- Explicit allocation/deallocation with host ABI
- Pointer arithmetic with proper type checking
- Memory operations validated at compile time
- Host ABI manages actual memory operations safely

### Multi-Backend Architecture

- **VM Backend**: Stack-based interpreter for debugging and testing
- **JIT Backend**: Cranelift-powered native code generation
- **Unified IR**: Both backends consume the same intermediate representation

## Development

### Adding New Language Features

1. **AST**: Update type definitions in `tilt-ast/src/lib.rs`
2. **Lexer**: Add tokens in `tilt-parser/src/lexer.rs`  
3. **Parser**: Update grammar in `tilt-parser/src/tilt.lalrpop`
4. **IR**: Add instruction types in `tilt-ir/src/lib.rs`
5. **Lowering**: Update AST→IR translation in `tilt-ir/src/lowering.rs`
6. **VM**: Implement instruction in `tilt-vm/src/lib.rs`
7. **JIT**: Add Cranelift codegen in `tilt-codegen-cranelift/src/lib.rs`
8. **Tests**: Add comprehensive tests in `tilt-integration-tests/`

### Tools and Dependencies

- **[LALRPOP](https://github.com/lalrpop/lalrpop)** - LR(1) parser generator
- **[Logos](https://github.com/maciejhirsz/logos)** - Fast lexer generator  
- **[Cranelift](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift)** - JIT compiler backend
- **[Cargo](https://doc.rust-lang.org/cargo/)** - Build system and package manager

## Performance

The compiler supports multiple execution strategies:

- **VM Interpretation**: ~100-1000x slower than native, excellent for debugging
- **JIT Compilation**: Near-native performance with fast compilation
- **AOT Compilation**: (Future) Full optimization for production use

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes following the established patterns
4. Add comprehensive tests for new functionality
5. Ensure all tests pass (`cargo test`)
6. Submit a pull request

### Code Organization Principles

- **Separation of concerns**: Each crate has a single responsibility
- **Type safety**: Leverage Rust's type system for correctness
- **Comprehensive testing**: Every feature should have integration tests
- **Documentation**: Code should be self-documenting with clear examples

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Status

**Current Status**: ✅ Feature Complete

The TILT compiler currently supports:
- ✅ Complete type system with pointer types
- ✅ Memory allocation/deallocation operations  
- ✅ Pointer arithmetic and memory load/store
- ✅ Full arithmetic and comparison operations
- ✅ Control flow (branches, conditionals)
- ✅ Function definitions and imports
- ✅ Host ABI integration
- ✅ VM and JIT execution backends
- ✅ Comprehensive test suite
- ✅ Text format parsing and compilation pipeline

**Future Enhancements**:
- 🔄 Advanced optimizations in JIT backend
- 🔄 Static analysis and optimization passes
- 🔄 Enhanced debugging and profiling tools
- 🔄 Advanced type features (structs, arrays)
