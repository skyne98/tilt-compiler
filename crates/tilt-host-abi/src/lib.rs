// ===================================================================
// FILE: lib.rs (tilt-host-abi crate)
//
// DESC: Host ABI trait and portable standard library interface for TILT.
//       This provides a way for TILT programs to interact with the host
//       environment in a portable way, whether running in JIT or interpreter.
// ===================================================================

use tilt_ast::Type;

/// Runtime values that can be passed between TILT and the host
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeValue {
    I32(i32),
    I64(i64),
    Usize(usize), // Platform-native unsigned integer for sizes, indices, and pointers
    Void,
}

impl RuntimeValue {
    /// Get the TILT type of this runtime value
    pub fn get_type(&self) -> Type {
        match self {
            RuntimeValue::I32(_) => Type::I32,
            RuntimeValue::I64(_) => Type::I64,
            RuntimeValue::Usize(_) => Type::Usize,
            RuntimeValue::Void => Type::Void,
        }
    }

    /// Extract an i32 value, panicking if the type doesn't match
    pub fn as_i32(&self) -> i32 {
        match self {
            RuntimeValue::I32(val) => *val,
            _ => panic!("Expected i32, got {:?}", self),
        }
    }

    /// Extract an i64 value, panicking if the type doesn't match
    pub fn as_i64(&self) -> i64 {
        match self {
            RuntimeValue::I64(val) => *val,
            _ => panic!("Expected i64, got {:?}", self),
        }
    }

    /// Extract a pointer value, panicking if the type doesn't match
    pub fn as_ptr(&self) -> u64 {
        match self {
            RuntimeValue::Usize(val) => (*val).try_into().unwrap(),
            _ => panic!("Expected ptr, got {:?}", self),
        }
    }

    /// Try to extract an i32 value, returning None if the type doesn't match
    pub fn try_as_i32(&self) -> Option<i32> {
        match self {
            RuntimeValue::I32(val) => Some(*val),
            _ => None,
        }
    }

    /// Try to extract an i64 value, returning None if the type doesn't match
    pub fn try_as_i64(&self) -> Option<i64> {
        match self {
            RuntimeValue::I64(val) => Some(*val),
            _ => None,
        }
    }

    /// Try to extract a pointer value, returning None if the type doesn't match
    pub fn try_as_ptr(&self) -> Option<u64> {
        match self {
            RuntimeValue::Usize(val) => Some((*val).try_into().unwrap()),
            _ => None,
        }
    }
}

/// Result type for host function calls
pub type HostResult = Result<RuntimeValue, String>;

/// Trait that defines the interface between TILT programs and the host environment.
/// This allows the same TILT code to run portably across different execution environments
/// (JIT, interpreter, etc.) by abstracting the host interaction layer.
pub trait HostABI {
    /// Call a host function by name with the given arguments
    fn call_host_function(&mut self, name: &str, args: &[RuntimeValue]) -> HostResult;

    /// Get a list of all available host functions
    fn available_functions(&self) -> Vec<&str>;

    /// Check if a host function is available
    fn has_function(&self, name: &str) -> bool {
        self.available_functions().contains(&name)
    }

    /// Read a typed value from memory (default implementation returns error)
    fn read_memory_value(&self, _addr: u64, _ty: tilt_ast::Type) -> Result<RuntimeValue, String> {
        Err("Memory operations not supported by this host ABI".to_string())
    }

    /// Write a typed value to memory (default implementation returns error)
    fn write_memory_value(&mut self, _addr: u64, _value: &RuntimeValue) -> Result<(), String> {
        Err("Memory operations not supported by this host ABI".to_string())
    }
}

/// Standard console-based host ABI implementation
/// This provides basic I/O functions for console interaction
pub struct ConsoleHostABI;

impl ConsoleHostABI {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsoleHostABI {
    fn default() -> Self {
        Self::new()
    }
}

impl HostABI for ConsoleHostABI {
    fn call_host_function(&mut self, name: &str, args: &[RuntimeValue]) -> HostResult {
        match name {
            "print_hello" => {
                if !args.is_empty() {
                    return Err(format!(
                        "print_hello expects 0 arguments, got {}",
                        args.len()
                    ));
                }
                println!("Hello from TILT!");
                Ok(RuntimeValue::Void)
            }

            "print_i32" => {
                if args.len() != 1 {
                    return Err(format!("print_i32 expects 1 argument, got {}", args.len()));
                }
                let value = args[0].as_i32();
                print!("{}", value);
                Ok(RuntimeValue::Void)
            }

            "print_i64" => {
                if args.len() != 1 {
                    return Err(format!("print_i64 expects 1 argument, got {}", args.len()));
                }
                let value = args[0].as_i64();
                print!("{}", value);
                Ok(RuntimeValue::Void)
            }

            "print_char" => {
                if args.len() != 1 {
                    return Err(format!("print_char expects 1 argument, got {}", args.len()));
                }
                let value = args[0].as_i32();
                if let Some(ch) = char::from_u32(value as u32) {
                    print!("{}", ch);
                    Ok(RuntimeValue::Void)
                } else {
                    Err(format!("Invalid character code: {}", value))
                }
            }

            "println" => {
                if !args.is_empty() {
                    return Err(format!("println expects 0 arguments, got {}", args.len()));
                }
                println!();
                Ok(RuntimeValue::Void)
            }

            "read_i32" => {
                use std::io::{self, Write};

                if !args.is_empty() {
                    return Err(format!("read_i32 expects 0 arguments, got {}", args.len()));
                }

                print!("Enter an integer: ");
                io::stdout().flush().unwrap();

                let mut input = String::new();
                match io::stdin().read_line(&mut input) {
                    Ok(_) => match input.trim().parse::<i32>() {
                        Ok(value) => Ok(RuntimeValue::I32(value)),
                        Err(_) => Err("Failed to parse integer".to_string()),
                    },
                    Err(e) => Err(format!("Failed to read input: {}", e)),
                }
            }

            _ => Err(format!("Unknown host function: {}", name)),
        }
    }

    fn available_functions(&self) -> Vec<&str> {
        vec![
            "print_hello",
            "print_i32",
            "print_i64",
            "print_char",
            "println",
            "read_i32",
        ]
    }
}

/// Extended host ABI that includes memory management functions
pub struct MemoryHostABI {
    /// Simple memory allocator using a HashMap to track allocations
    memory: std::collections::HashMap<u64, Vec<u8>>,
    /// Next allocation address
    next_addr: u64,
}

impl MemoryHostABI {
    pub fn new() -> Self {
        Self {
            memory: std::collections::HashMap::new(),
            next_addr: 0x1000, // Start at a non-zero address
        }
    }

    /// Read bytes from memory at the given address
    pub fn read_memory(&self, addr: u64, size: usize) -> Result<Vec<u8>, String> {
        // Find the allocation that contains this address
        for (base_addr, data) in &self.memory {
            if addr >= *base_addr && addr + size as u64 <= *base_addr + data.len() as u64 {
                let offset = (addr - base_addr) as usize;
                return Ok(data[offset..offset + size].to_vec());
            }
        }
        Err(format!("Invalid memory access at address 0x{:x}", addr))
    }

    /// Write bytes to memory at the given address
    pub fn write_memory(&mut self, addr: u64, data: &[u8]) -> Result<(), String> {
        // Find the allocation that contains this address
        for (base_addr, memory_data) in &mut self.memory {
            if addr >= *base_addr
                && addr + data.len() as u64 <= *base_addr + memory_data.len() as u64
            {
                let offset = (addr - base_addr) as usize;
                memory_data[offset..offset + data.len()].copy_from_slice(data);
                return Ok(());
            }
        }
        Err(format!("Invalid memory write at address 0x{:x}", addr))
    }

    /// Read a typed value from memory
    pub fn read_value(&self, addr: u64, ty: tilt_ast::Type) -> Result<RuntimeValue, String> {
        use tilt_ast::Type;
        match ty {
            Type::I32 => {
                let bytes = self.read_memory(addr, 4)?;
                let value = i32::from_le_bytes(bytes.try_into().unwrap());
                Ok(RuntimeValue::I32(value))
            }
            Type::I64 => {
                let bytes = self.read_memory(addr, 8)?;
                let value = i64::from_le_bytes(bytes.try_into().unwrap());
                Ok(RuntimeValue::I64(value))
            }
            Type::F32 => {
                // For now, treat as i32 since we don't have F32 variant
                let bytes = self.read_memory(addr, 4)?;
                let value = i32::from_le_bytes(bytes.try_into().unwrap());
                Ok(RuntimeValue::I32(value))
            }
            Type::F64 => {
                // For now, treat as i64 since we don't have F64 variant
                let bytes = self.read_memory(addr, 8)?;
                let value = i64::from_le_bytes(bytes.try_into().unwrap());
                Ok(RuntimeValue::I64(value))
            }
            Type::Usize => {
                let bytes = self.read_memory(addr, 8)?;
                let value = u64::from_le_bytes(bytes.try_into().unwrap());
                Ok(RuntimeValue::Usize(value.try_into().unwrap()))
            }
            Type::Void => Err("Cannot read void type from memory".to_string()),
        }
    }

    /// Write a typed value to memory
    pub fn write_value(&mut self, addr: u64, value: &RuntimeValue) -> Result<(), String> {
        match value {
            RuntimeValue::I32(v) => self.write_memory(addr, &v.to_le_bytes()),
            RuntimeValue::I64(v) => self.write_memory(addr, &v.to_le_bytes()),
            RuntimeValue::Usize(v) => self.write_memory(addr, &v.to_le_bytes()),
            RuntimeValue::Void => Err("Cannot write void type to memory".to_string()),
        }
    }

    fn allocate(&mut self, size: u64) -> u64 {
        if size == 0 {
            return 0; // Null pointer for zero-sized allocation
        }

        let addr = self.next_addr;
        self.next_addr += size + 8; // Add some padding between allocations
        self.memory.insert(addr, vec![0; size as usize]);
        addr
    }

    fn deallocate(&mut self, addr: u64) -> Result<(), String> {
        if addr == 0 {
            return Ok(()); // Freeing null pointer is a no-op
        }

        if self.memory.remove(&addr).is_some() {
            Ok(())
        } else {
            Err(format!("Attempt to free invalid address: 0x{:x}", addr))
        }
    }
}

impl HostABI for MemoryHostABI {
    fn call_host_function(&mut self, name: &str, args: &[RuntimeValue]) -> HostResult {
        match name {
            "alloc" => {
                if args.len() != 1 {
                    return Err(format!("alloc expects 1 argument, got {}", args.len()));
                }
                let size = args[0].as_ptr();
                let addr = self.allocate(size);
                Ok(RuntimeValue::Usize(addr.try_into().unwrap()))
            }

            "free" => {
                if args.len() != 1 {
                    return Err(format!("free expects 1 argument, got {}", args.len()));
                }
                let addr = args[0].as_ptr();
                self.deallocate(addr)?;
                Ok(RuntimeValue::Void)
            }

            // Delegate other functions to a console ABI
            _ => {
                let mut console_abi = ConsoleHostABI::new();
                console_abi.call_host_function(name, args)
            }
        }
    }

    fn available_functions(&self) -> Vec<&str> {
        vec![
            "alloc",
            "free",
            "print_hello",
            "print_i32",
            "print_i64",
            "print_char",
            "println",
            "read_i32",
        ]
    }

    fn read_memory_value(&self, addr: u64, ty: tilt_ast::Type) -> Result<RuntimeValue, String> {
        self.read_value(addr, ty)
    }

    fn write_memory_value(&mut self, addr: u64, value: &RuntimeValue) -> Result<(), String> {
        self.write_value(addr, value)
    }
}

/// A no-op host ABI for testing or isolated execution
pub struct NullHostABI;

impl NullHostABI {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullHostABI {
    fn default() -> Self {
        Self::new()
    }
}

impl HostABI for NullHostABI {
    fn call_host_function(&mut self, name: &str, _args: &[RuntimeValue]) -> HostResult {
        Err(format!("Null ABI: function '{}' not implemented", name))
    }

    fn available_functions(&self) -> Vec<&str> {
        vec![]
    }
}

/// Helper trait for converting Rust values to RuntimeValue
pub trait IntoRuntimeValue {
    fn into_runtime_value(self) -> RuntimeValue;
}

impl IntoRuntimeValue for i32 {
    fn into_runtime_value(self) -> RuntimeValue {
        RuntimeValue::I32(self)
    }
}

impl IntoRuntimeValue for i64 {
    fn into_runtime_value(self) -> RuntimeValue {
        RuntimeValue::I64(self)
    }
}

impl IntoRuntimeValue for () {
    fn into_runtime_value(self) -> RuntimeValue {
        RuntimeValue::Void
    }
}

/// Real memory allocation Host ABI for JIT
/// This allocates actual system memory that the JIT can directly access
pub struct JITMemoryHostABI {
    /// Track allocations for proper cleanup using addresses
    allocations: std::collections::HashMap<u64, usize>, // addr -> size
}

impl JITMemoryHostABI {
    pub fn new() -> Self {
        Self {
            allocations: std::collections::HashMap::new(),
        }
    }

    /// Allocate real memory and return the raw pointer address
    fn allocate_real_memory(&mut self, size: u64) -> u64 {
        if size == 0 {
            return 0;
        }

        use std::alloc::{Layout, alloc};

        let layout = Layout::from_size_align(size as usize, 8).unwrap();
        let ptr = unsafe { alloc(layout) };

        if ptr.is_null() {
            return 0; // Allocation failed
        }

        let addr = ptr as u64;
        self.allocations.insert(addr, size as usize);

        addr
    }

    /// Free real memory
    fn free_real_memory(&mut self, addr: u64) -> Result<(), String> {
        if addr == 0 {
            return Ok(());
        }

        if let Some(size) = self.allocations.remove(&addr) {
            use std::alloc::{Layout, dealloc};
            let layout = Layout::from_size_align(size, 8).unwrap();
            let ptr = addr as *mut u8;
            unsafe { dealloc(ptr, layout) };
            Ok(())
        } else {
            Err(format!("Attempt to free invalid address: 0x{:x}", addr))
        }
    }
}

impl HostABI for JITMemoryHostABI {
    fn call_host_function(&mut self, name: &str, args: &[RuntimeValue]) -> HostResult {
        match name {
            "alloc" => {
                if args.len() != 1 {
                    return Err(format!("alloc expects 1 argument, got {}", args.len()));
                }
                let size = args[0].as_ptr();
                let addr = self.allocate_real_memory(size);
                Ok(RuntimeValue::Usize(addr.try_into().unwrap()))
            }

            "free" => {
                if args.len() != 1 {
                    return Err(format!("free expects 1 argument, got {}", args.len()));
                }
                let addr = args[0].as_ptr();
                self.free_real_memory(addr)?;
                Ok(RuntimeValue::Void)
            }

            // Delegate other functions to a console ABI
            _ => {
                let mut console_abi = ConsoleHostABI::new();
                console_abi.call_host_function(name, args)
            }
        }
    }

    fn available_functions(&self) -> Vec<&str> {
        vec![
            "alloc",
            "free",
            "print_hello",
            "print_i32",
            "print_i64",
            "print_char",
            "println",
            "read_i32",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_value_types() {
        let val_i32 = RuntimeValue::I32(42);
        let val_i64 = RuntimeValue::I64(123);
        let val_void = RuntimeValue::Void;

        assert_eq!(val_i32.get_type(), Type::I32);
        assert_eq!(val_i64.get_type(), Type::I64);
        assert_eq!(val_void.get_type(), Type::Void);

        assert_eq!(val_i32.as_i32(), 42);
        assert_eq!(val_i64.as_i64(), 123);
    }

    #[test]
    fn test_runtime_value_try_cast() {
        let val = RuntimeValue::I32(42);

        assert_eq!(val.try_as_i32(), Some(42));
        assert_eq!(val.try_as_i64(), None);
    }

    #[test]
    fn test_console_host_abi_available_functions() {
        let abi = ConsoleHostABI::new();
        let functions = abi.available_functions();

        assert!(functions.contains(&"print_hello"));
        assert!(functions.contains(&"print_i32"));
        assert!(functions.contains(&"print_i64"));
        assert!(functions.contains(&"print_char"));
        assert!(functions.contains(&"println"));
        assert!(functions.contains(&"read_i32"));
    }

    #[test]
    fn test_console_host_abi_print_i32() {
        let mut abi = ConsoleHostABI::new();
        let args = vec![RuntimeValue::I32(42)];

        let result = abi.call_host_function("print_i32", &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RuntimeValue::Void);
    }

    #[test]
    fn test_console_host_abi_print_char() {
        let mut abi = ConsoleHostABI::new();
        let args = vec![RuntimeValue::I32(65)]; // ASCII 'A'

        let result = abi.call_host_function("print_char", &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RuntimeValue::Void);
    }

    #[test]
    fn test_console_host_abi_wrong_args() {
        let mut abi = ConsoleHostABI::new();
        let args = vec![RuntimeValue::I32(42), RuntimeValue::I32(43)]; // too many args

        let result = abi.call_host_function("print_i32", &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_console_host_abi_unknown_function() {
        let mut abi = ConsoleHostABI::new();
        let args = vec![];

        let result = abi.call_host_function("unknown_function", &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_null_host_abi() {
        let mut abi = NullHostABI::new();
        let args = vec![RuntimeValue::I32(42)];

        let result = abi.call_host_function("print_i32", &args);
        assert!(result.is_err());

        assert_eq!(abi.available_functions().len(), 0);
    }

    #[test]
    fn test_into_runtime_value() {
        assert_eq!(42i32.into_runtime_value(), RuntimeValue::I32(42));
        assert_eq!(123i64.into_runtime_value(), RuntimeValue::I64(123));
        assert_eq!(().into_runtime_value(), RuntimeValue::Void);
    }
}
