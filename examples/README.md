# TILT Examples

This directory contains example TILT programs demonstrating various language features.

## Memory Operations (`example_memory.tilt`)

Demonstrates the new memory management primitives:

- **Memory allocation**: `alloc(size)` and `free(ptr)`
- **Pointer arithmetic**: `ptr.add(ptr, offset)`
- **Memory operations**: `T.load(ptr)` and `T.store(ptr, value)`
- **Size operations**: `sizeof.T()`

### Key Features Shown:

1. **Heap Allocation**: Allocating memory for multiple values
2. **Pointer Math**: Calculating offsets for array-like access
3. **Type-safe Memory Access**: Loading and storing typed values
4. **Memory Management**: Proper cleanup with `free()`

### Usage:

```bash
cargo run -- examples/example_memory.tilt
```

Note: The example uses proper TILT syntax but may require parser improvements for full compilation. The memory operations are fully functional when used through the IR builder API (as demonstrated in the integration tests).

## Language Features Demonstrated

- Import declarations with typed parameters
- Function definitions with return values
- Variable assignments with type annotations
- Memory allocation and deallocation
- Pointer arithmetic and memory access
- Mathematical operations (`i32.add`, `i64.add`)
- Control flow (`ret` statements)

These examples serve as both documentation and test cases for the TILT language's memory management capabilities.
