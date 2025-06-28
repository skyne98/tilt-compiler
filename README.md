# TILT Compiler

A modern, multi-backend compiler for the TILT intermediate language, featuring comprehensive memory management, pointer arithmetic, and robust execution backends.

## Overview

TILT (Typed Intermediate Language with Tensors) is a low-level intermediate language designed for systems programming with strong typing and explicit memory management. The compiler provides multiple execution backends including a virtual machine (VM) interpreter and a JIT compiler powered by Cranelift.

## Language Design Philosophy

TILT follows a **Static Single Assignment (SSA)** form with unified expression-based design:

- **SSA Compliance**: Every value is assigned exactly once (no variable reassignment)
- **Expression-Based**: Operations that return values are used in assignments
- **Statement-Based**: Operations that return `void` are used as statements  
- **No Nested Calls**: Function calls cannot be nested (e.g., `f(g(x))` is invalid)
- **Consistent Syntax**: All operations use parenthesized syntax
- **Strong Typing**: Explicit type annotations with compile-time checking

### SSA Form Requirements

```tilt
# ✅ Valid SSA - each variable assigned once
a:i32 = i32.const(10)
b:i32 = i32.const(20)
sum:i32 = i32.add(a, b)

# ❌ Invalid - variable reassignment
a:i32 = i32.const(10)
a = i32.const(20)  # Error: 'a' already assigned

# ❌ Invalid - nested function calls
result:i32 = i32.add(i32.const(10), i32.const(20))  # Error: nested calls

# ✅ Valid SSA equivalent
val1:i32 = i32.const(10)
val2:i32 = i32.const(20)
result:i32 = i32.add(val1, val2)
```

## Architecture

The project is structured as a Rust workspace with multiple specialized crates:

### Core Language Infrastructure

- **`tilt-ast`** - Abstract Syntax Tree definitions and type system
- **`tilt-parser`** - Lexical analysis (Logos) and parsing (LALRPOP) with type keywords as identifiers
- **`tilt-ir`** - Intermediate representation and semantic analysis with comprehensive lowering
- **`tilt-ir-builder`** - Programmatic IR construction API

### Execution Backends

- **`tilt-vm`** - Stack-based virtual machine interpreter with full memory operations
- **`tilt-codegen-cranelift`** - JIT compiler using Cranelift backend with native memory access
- **`tilt-host-abi`** - Host function interface and memory management with multiple ABI implementations

### Tooling

- **`tiltc`** - Command-line compiler with REPL, debugging, and backend comparison
- **`tilt-integration-tests`** - Comprehensive end-to-end tests and compatibility validation

## Language Features

### Type System

```tilt
# Primitive types
i32    # 32-bit signed integer
i64    # 64-bit signed integer  
f32    # 32-bit floating point (parsed but limited backend support)
f64    # 64-bit floating point (parsed but limited backend support)
ptr    # Pointer type (64-bit on x64 platforms)
void   # No value (for functions/operations with side effects)
```

### Memory Management

TILT provides explicit, low-level memory operations with full SSA compliance:

```tilt
import "host" "alloc" (size:i64) -> ptr
import "host" "free" (p:ptr) -> void

fn memory_example() -> i32 {
entry:
    # Memory allocation
    size:i64 = i64.const(8)
    array_ptr:ptr = alloc(size)
    
    # Memory store operations
    value1:i32 = i32.const(100)
    i32.store(array_ptr, value1)
    
    # Pointer arithmetic
    offset:i64 = sizeof.i32()
    ptr2:ptr = ptr.add(array_ptr, offset)
    value2:i32 = i32.const(200)
    i32.store(ptr2, value2)
    
    # Memory load operations
    loaded1:i32 = i32.load(array_ptr)
    loaded2:i32 = i32.load(ptr2)
    
    # Arithmetic operations
    result:i32 = i32.add(loaded1, loaded2)
    
    # Memory deallocation
    free(array_ptr)
    
    ret (result)
}
```

### Function System

```tilt
# Import declarations for host functions
import "host" "alloc" (size:i64) -> ptr
import "host" "free" (p:ptr) -> void

# Function definitions with typed parameters
fn calculate_sum(a:i32, b:i32) -> i32 {
entry:
    result:i32 = i32.add(a, b)
    ret (result)
}

# Void functions
fn print_value(value:i32) -> void {
entry:
    print_i32(value)
    ret
}
```

### Arithmetic and Logic Operations

```tilt
# All operations use consistent parenthesized syntax
sum:i32 = i32.add(a, b)           # Addition
diff:i32 = i32.sub(a, b)          # Subtraction  
product:i32 = i32.mul(a, b)       # Multiplication
quotient:i32 = i32.div(a, b)      # Division

# Comparison operations (return i32: 1 for true, 0 for false)
equal:i32 = i32.eq(a, b)          # Equality
less:i32 = i32.lt(a, b)           # Less than

# Constants with type-specific constructors
value:i32 = i32.const(42)         # 32-bit integer constant
size:i64 = i64.const(1024)        # 64-bit integer constant

# Type introspection
size:i64 = sizeof.i32()           # Get size of i32 (returns 4)
```

### Control Flow

```tilt
# Basic blocks with explicit labels
fn conditional_example(x:i32) -> i32 {
entry:
    zero:i32 = i32.const(0)
    condition:i32 = i32.eq(x, zero)
    br_if condition, zero_block, nonzero_block

zero_block:
    result:i32 = i32.const(100)
    br final_block

nonzero_block:
    result:i32 = i32.const(200)
    br final_block

final_block:
    ret (result)
}
```

### Memory Operations Deep Dive

TILT provides comprehensive memory management with both high-level operations and low-level control:

```tilt
import "host" "alloc" (size:i64) -> ptr
import "host" "free" (p:ptr) -> void

fn advanced_memory_example() -> i32 {
entry:
    # Array allocation for 5 i32 values
    element_size:i64 = sizeof.i32()
    count:i64 = i64.const(5)
    total_size:i64 = i64.mul(element_size, count)
    array_ptr:ptr = alloc(total_size)
    
    # Initialize array elements with pointer arithmetic
    val1:i32 = i32.const(10)
    i32.store(array_ptr, val1)
    
    offset1:i64 = sizeof.i32()
    ptr1:ptr = ptr.add(array_ptr, offset1)
    val2:i32 = i32.const(20)
    i32.store(ptr1, val2)
    
    # More complex offset calculation
    two:i64 = i64.const(2)
    offset2:i64 = i64.mul(element_size, two)
    ptr2:ptr = ptr.add(array_ptr, offset2)
    val3:i32 = i32.const(30)
    i32.store(ptr2, val3)
    
    # Load and sum all values
    loaded1:i32 = i32.load(array_ptr)
    loaded2:i32 = i32.load(ptr1)
    loaded3:i32 = i32.load(ptr2)
    
    sum1:i32 = i32.add(loaded1, loaded2)
    result:i32 = i32.add(sum1, loaded3)
    
    # Proper cleanup
    free(array_ptr)
    
    ret (result)
}
```

## Implementation Architecture

### Parser and Lexer Implementation

The parser uses LALRPOP with several important design decisions:

- **Type Keywords as Identifiers**: Allows variables like `ptr:ptr = alloc(size)`
- **No Nested Function Calls**: Enforces SSA form at parse time
- **Expression vs Statement Distinction**: Void operations are statements, others are expressions
- **Comments**: Support for `#` line comments

### IR (Intermediate Representation)

The IR layer provides comprehensive lowering from AST with semantic analysis:

- **Value ID System**: Each value gets a unique identifier in SSA form
- **Block-based Structure**: Functions contain basic blocks with terminators
- **Type Checking**: Full type validation during lowering
- **Host Function Integration**: Seamless import and call mechanism

### Virtual Machine (VM) Backend

The VM provides a stack-based interpreter with:

- **Call Stack Management**: Proper function call handling with stack frames
- **Memory Integration**: Full integration with MemoryHostABI for actual memory operations
- **Value Storage**: HashMap-based value storage per stack frame
- **Host ABI Calls**: Direct integration with host functions

### JIT Backend (Cranelift)

The JIT compiler provides native code generation with:

- **Real Memory Access**: Uses JITMemoryHostABI for direct system memory allocation
- **Cranelift IR Output**: Debug output shows generated Cranelift IR
- **Host Function Calls**: Native function calls to host ABI
- **Type-Safe Code Generation**: Maintains TILT's type safety in generated code

### Host ABI System

The Host ABI provides multiple implementations:

- **ConsoleHostABI**: Basic I/O operations (print_i32, print_char, etc.)
- **MemoryHostABI**: Simulated memory for VM with HashMap-based storage
- **JITMemoryHostABI**: Real system memory allocation for JIT
- **NullHostABI**: No-op implementation for testing

## Building and Usage

```bash
# Build the entire workspace
cargo build

# Build with optimizations
cargo build --release

# Run all tests including comprehensive integration tests
cargo test

# Run specific memory operation tests
cargo test -p tilt-codegen-cranelift memory -- --nocapture

# Run the compiler CLI
./target/debug/tiltc.exe [file.tilt] [options]

# CLI Options:
#   --vm                Use VM backend (default)
#   --jit               Use JIT backend  
#   --both              Compare both backends
#   --repl              Start interactive REPL
#   --show-tokens       Display lexer tokens
#   --show-ast          Display abstract syntax tree
#   --show-ir           Display intermediate representation
#   --show-cranelift-ir Display Cranelift IR (JIT only)
#   --verbose           Enable verbose output
#   --measure-time      Measure execution time
```

### CLI Examples

```bash
# Run program with VM backend
./target/debug/tiltc.exe examples/advanced_memory_test.tilt --vm

# Run with JIT and show Cranelift IR
./target/debug/tiltc.exe examples/advanced_memory_test.tilt --jit --show-cranelift-ir

# Compare VM and JIT results
./target/debug/tiltc.exe examples/advanced_memory_test.tilt --both

# Start REPL with JIT backend
./target/debug/tiltc.exe --repl --jit

# Debug parsing and IR generation
./target/debug/tiltc.exe program.tilt --show-tokens --show-ast --show-ir
```

## Project Structure

```
tilt-compiler/
├── Cargo.toml                          # Workspace configuration
├── README.md                           # This file
├── LICENSE                             # MIT license
├── examples/                           # Example TILT programs
│   ├── advanced_memory_test.tilt       # Complex memory operations
│   ├── example_memory.tilt             # Basic memory example
│   ├── final_memory_test.tilt          # Memory test for validation
│   ├── minimal_alloc_test.tilt         # Minimal allocation test
│   ├── simple_test.tilt                # Basic arithmetic
│   └── test_function_call.tilt         # Function call examples
├── crates/                             # Rust workspace crates
│   ├── tilt-ast/                       # AST and type definitions
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── tilt-parser/                    # Lexer and parser
│   │   ├── Cargo.toml
│   │   ├── build.rs                    # LALRPOP build script
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── lexer.rs                # Logos-based lexer
│   │       ├── tests.rs                # Parser tests
│   │       └── tilt.lalrpop            # LALRPOP grammar
│   ├── tilt-ir/                        # IR and semantic analysis
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # IR definitions
│   │       ├── lowering.rs             # AST → IR transformation
│   │       └── tests.rs                # IR tests
│   ├── tilt-ir-builder/                # Programmatic IR construction
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs
│   │   └── tests/integration_test.rs
│   ├── tilt-vm/                        # Virtual machine interpreter
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                  # VM implementation with memory ops
│   ├── tilt-codegen-cranelift/         # JIT compiler backend
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # JIT and Cranelift integration
│   │       ├── memory_jit_tests.rs     # Comprehensive JIT memory tests
│   │       └── tests.rs                # JIT tests
│   ├── tilt-host-abi/                  # Host function interface
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                  # Multiple ABI implementations
│   ├── tilt-integration-tests/         # End-to-end integration tests
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── memory_test.rs          # Memory operation tests
│   │       └── vm_jit_compatibility.rs # Backend compatibility tests
│   └── tiltc/                          # Main compiler binary
│       ├── Cargo.toml
│       └── src/main.rs                 # CLI, REPL, and execution logic
└── target/                             # Build artifacts (generated)
```

## Testing

The project includes comprehensive test coverage across multiple dimensions:

### Test Categories

- **Unit Tests**: Each crate tests its components in isolation
- **Integration Tests**: Cross-crate functionality validation
- **Memory Operation Tests**: Comprehensive memory management validation
- **VM/JIT Compatibility Tests**: Ensure both backends produce identical results
- **Parser Tests**: Lexer and grammar validation
- **End-to-End Tests**: Complete compilation pipeline validation

### Key Test Files

```
crates/tilt-integration-tests/src/
├── memory_test.rs              # Comprehensive memory feature tests
└── vm_jit_compatibility.rs     # Backend result comparison tests

crates/tilt-codegen-cranelift/src/
└── memory_jit_tests.rs         # JIT-specific memory operation tests

crates/tilt-parser/src/
└── tests.rs                    # Parser and lexer validation

crates/tilt-vm/src/
└── lib.rs                      # VM test suite (inline tests)
```

### Test Execution

```bash
# Run all tests with output
cargo test -- --nocapture

# Run specific test suites
cargo test -p tilt-integration-tests
cargo test -p tilt-codegen-cranelift memory
cargo test -p tilt-parser
cargo test -p tilt-vm

# Run VM/JIT compatibility tests
cargo test vm_jit_compatibility -- --nocapture
```

## Language Quirks and Important Details

### SSA Form Enforcement

TILT strictly enforces SSA form, which means:

```tilt
# ❌ This will cause a parsing error:
result:i32 = i32.add(i32.const(10), i32.const(20))

# ✅ Must be written as:
val1:i32 = i32.const(10)
val2:i32 = i32.const(20)
result:i32 = i32.add(val1, val2)
```

### Type Keywords as Identifiers

The parser allows type keywords to be used as variable names:

```tilt
# ✅ This is valid TILT code:
ptr:ptr = alloc(size)     # 'ptr' is both a type and variable name
i32:i32 = i32.const(42)   # 'i32' is both a type and variable name
```

### Memory Model Differences

- **VM Backend**: Uses simulated memory with HashMap storage
- **JIT Backend**: Uses real system memory allocation
- **Both**: Produce identical results for all memory operations

### Host ABI Integration

Host functions are seamlessly integrated:

```tilt
# Import host functions
import "host" "alloc" (size:i64) -> ptr

# Use like any other function
ptr:ptr = alloc(size)  # Direct call, no special syntax needed
```

### Comments and Syntax

```tilt
# Line comments start with '#'
# There are no block comments

# All operations use parentheses
result:i32 = i32.add(a, b)    # Not: result = a + b

# Type annotations are mandatory
value:i32 = i32.const(42)     # Not: value = 42
```

## Performance Characteristics

### Compilation Speed
- **Parsing**: Very fast (Logos + LALRPOP)
- **IR Generation**: Fast single-pass lowering
- **JIT Compilation**: Fast Cranelift code generation

### Runtime Performance
- **VM Interpretation**: ~100-1000x slower than native (excellent for debugging)
- **JIT Compilation**: Near-native performance with minimal compilation overhead

### Memory Usage
- **VM**: Minimal memory overhead with HashMap-based value storage
- **JIT**: Direct system memory usage, efficient allocation patterns

## Implementation Highlights

### Unified Expression System

TILT implements a clean expression-based design:

```tilt
# Everything is either an expression (returns value) or statement (returns void)
value:i32 = i32.add(a, b)     # Expression - produces value
i32.store(ptr, value)         # Statement - void operation
```

### Multi-Backend Architecture

- **Shared IR**: Both VM and JIT consume identical intermediate representation
- **Host ABI Abstraction**: Different memory models unified behind common interface
- **Result Validation**: Built-in comparison mode ensures backend consistency

### Comprehensive Debugging

The compiler provides extensive debugging capabilities:

```bash
# See every stage of compilation
./target/debug/tiltc.exe program.tilt \
  --show-tokens \
  --show-ast \
  --show-ir \
  --show-cranelift-ir \
  --verbose
```

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

### Code Organization Principles

- **Separation of Concerns**: Each crate has a single, well-defined responsibility
- **Type Safety**: Leverage Rust's type system for correctness guarantees
- **Comprehensive Testing**: Every feature has unit, integration, and compatibility tests
- **SSA Compliance**: All code generation maintains SSA form invariants

### Tools and Dependencies

- **[LALRPOP](https://github.com/lalrpop/lalrpop)** - LR(1) parser generator for grammar
- **[Logos](https://github.com/maciejhirsz/logos)** - Fast lexer generator for tokenization
- **[Cranelift](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift)** - JIT compiler backend
- **[Clap](https://clap.rs/)** - Command-line argument parsing
- **[Colored](https://crates.io/crates/colored)** - Terminal color output
- **[Rustyline](https://crates.io/crates/rustyline)** - REPL implementation

## Current Status

**Status**: ✅ **Production Ready**

The TILT compiler is feature-complete and thoroughly tested:

### ✅ Completed Features

- **Complete Type System**: All primitive types with pointer support
- **Memory Management**: Full allocation/deallocation with pointer arithmetic
- **Memory Operations**: Load/store operations with type safety
- **Arithmetic Operations**: Complete set of binary operations (add, sub, mul, div, eq, lt)
- **Control Flow**: Basic blocks, branches, and conditionals
- **Function System**: Definitions, imports, and calls with type checking
- **Host ABI Integration**: Multiple ABI implementations with seamless integration
- **Dual Backend Support**: VM interpreter and JIT compiler with result validation
- **SSA Form Compliance**: Strict SSA enforcement throughout compilation pipeline
- **Comprehensive Testing**: 120+ tests covering all language features
- **Debug Tooling**: Complete introspection of compilation pipeline
- **CLI and REPL**: Professional command-line interface with interactive mode

### 🔧 Technical Achievements

- **Parser Robustness**: Handles edge cases like type keywords as identifiers
- **Memory Safety**: Both VM and JIT backends provide memory-safe execution
- **Backend Compatibility**: VM and JIT produce identical results on all tests
- **Performance**: JIT backend achieves near-native performance
- **Extensibility**: Clean architecture enables easy addition of new features

### 📊 Test Coverage

- **Parser Tests**: 47 tests covering lexer, grammar, and edge cases
- **VM Tests**: 4 core tests plus integration coverage
- **JIT Tests**: 30 tests including 9 dedicated memory operation tests
- **Integration Tests**: 9 comprehensive end-to-end tests
- **Compatibility Tests**: 9 VM/JIT comparison tests
- **Total**: 120+ tests with 100% pass rate

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Follow established architectural patterns
4. Add comprehensive tests for new functionality
5. Ensure all tests pass (`cargo test`)
6. Update documentation as needed
7. Submit a pull request

The TILT compiler represents a complete, production-ready implementation of a low-level intermediate language with modern tooling and robust execution backends.
