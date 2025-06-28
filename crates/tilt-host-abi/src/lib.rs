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
    Void,
}

impl RuntimeValue {
    /// Get the TILT type of this runtime value
    pub fn get_type(&self) -> Type {
        match self {
            RuntimeValue::I32(_) => Type::I32,
            RuntimeValue::I64(_) => Type::I64,
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
                    return Err(format!("print_hello expects 0 arguments, got {}", args.len()));
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
                    Ok(_) => {
                        match input.trim().parse::<i32>() {
                            Ok(value) => Ok(RuntimeValue::I32(value)),
                            Err(_) => Err("Failed to parse integer".to_string()),
                        }
                    }
                    Err(e) => Err(format!("Failed to read input: {}", e)),
                }
            }
            
            _ => Err(format!("Unknown host function: {}", name)),
        }
    }

    fn available_functions(&self) -> Vec<&str> {
        vec!["print_hello", "print_i32", "print_i64", "print_char", "println", "read_i32"]
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
