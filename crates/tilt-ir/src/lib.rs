// ===================================================================
// FILE: lib.rs (tilt-ir crate)
//
// DESC: Defines the Intermediate Representation (IR) for the TILT 
//       language. This is a graph-based representation optimized for
//       analysis, optimization, and code generation.
// ===================================================================

use tilt_ast::Type;

pub mod lowering;

#[cfg(test)]
mod tests;

// Re-export main lowering function
pub use lowering::lower_program;

/// Program-level IR containing all functions and imports
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub imports: Vec<ImportDecl>,
    pub functions: Vec<Function>,
}

/// Import declaration in IR form
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub module: String,
    pub name: String,
    pub calling_convention: Option<String>,  // e.g., "c" for C calling convention
    pub params: Vec<Type>,
    pub return_type: Type,
}

/// A function in IR form with resolved references
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Type>,
    pub return_type: Type,
    pub blocks: Vec<BasicBlock>,
    pub entry_block: BlockId,
    pub next_value_id: ValueId, // For generating unique value IDs
    /// Map of constant values (value_id -> (constant_value, type))
    pub constants: std::collections::HashMap<ValueId, (i64, Type)>,
}

/// Opaque identifier for a basic block
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub usize);

/// Opaque identifier for an SSA value
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId(pub usize);

/// A basic block in the IR
#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub id: BlockId,
    pub label: String, // Keep original label for debugging
    pub params: Vec<(ValueId, Type)>, // Phi nodes represented as block parameters
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

/// Instructions in the IR with resolved references
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    /// Binary arithmetic operation
    BinaryOp {
        dest: ValueId,
        op: BinaryOperator,
        ty: Type,
        lhs: ValueId,
        rhs: ValueId,
    },
    /// Unary operation
    UnaryOp {
        dest: ValueId,
        op: UnaryOperator,
        ty: Type,
        operand: ValueId,
    },
    /// Function call with assignment
    Call {
        dest: ValueId,
        function: String,
        args: Vec<ValueId>,
        return_type: Type,
    },
    /// Function call without assignment (void functions)
    CallVoid {
        function: String,
        args: Vec<ValueId>,
    },
    /// Load from memory
    Load {
        dest: ValueId,
        ty: Type,
        address: ValueId,
    },
    /// Store to memory
    Store {
        address: ValueId,
        value: ValueId,
        ty: Type,
    },
    /// Constant assignment
    Const {
        dest: ValueId,
        value: i64,
        ty: Type,
    },
    /// Pointer arithmetic - add offset to pointer
    PtrAdd {
        dest: ValueId,
        ptr: ValueId,
        offset: ValueId,
    },
    /// Get size of type in bytes
    SizeOf {
        dest: ValueId,
        ty: Type,
    },
    /// Host ABI allocation
    Alloc {
        dest: ValueId,
        size: ValueId,
    },
    /// Host ABI deallocation
    Free {
        ptr: ValueId,
    },
}

/// Terminator instructions that end basic blocks
#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    /// Return from function
    Ret {
        value: Option<ValueId>,
    },
    /// Unconditional branch
    Br {
        target: BlockId,
    },
    /// Conditional branch
    BrIf {
        cond: ValueId,
        true_target: BlockId,
        false_target: BlockId,
    },
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Neg,
    Not,
}

/// Errors that can occur during semantic analysis and IR generation
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticError {
    /// Undefined variable or function
    UndefinedIdentifier {
        name: String,
        location: String,
    },
    /// Duplicate definition
    DuplicateDefinition {
        name: String,
        location: String,
    },
    /// Type mismatch
    TypeMismatch {
        expected: Type,
        found: Type,
        location: String,
    },
    /// Invalid operation for type
    InvalidOperation {
        operation: String,
        ty: Type,
        location: String,
    },
    /// Undefined block label
    UndefinedBlock {
        label: String,
        location: String,
    },
    /// Missing terminator in block
    MissingTerminator {
        block: String,
    },
    /// Invalid phi node reference
    InvalidPhiReference {
        block: String,
        referenced_block: String,
    },
    /// Function not found
    FunctionNotFound {
        name: String,
        location: String,
    },
    /// Wrong number of arguments
    ArgumentMismatch {
        function: String,
        expected: usize,
        found: usize,
        location: String,
    },
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticError::UndefinedIdentifier { name, location } => {
                write!(f, "Undefined identifier '{}' at {}", name, location)
            }
            SemanticError::DuplicateDefinition { name, location } => {
                write!(f, "Duplicate definition of '{}' at {}", name, location)
            }
            SemanticError::TypeMismatch { expected, found, location } => {
                write!(f, "Type mismatch at {}: expected {:?}, found {:?}", location, expected, found)
            }
            SemanticError::InvalidOperation { operation, ty, location } => {
                write!(f, "Invalid operation '{}' for type {:?} at {}", operation, ty, location)
            }
            SemanticError::UndefinedBlock { label, location } => {
                write!(f, "Undefined block '{}' referenced at {}", label, location)
            }
            SemanticError::MissingTerminator { block } => {
                write!(f, "Block '{}' is missing a terminator instruction", block)
            }
            SemanticError::InvalidPhiReference { block, referenced_block } => {
                write!(f, "Block '{}' references non-existent predecessor '{}'", block, referenced_block)
            }
            SemanticError::FunctionNotFound { name, location } => {
                write!(f, "Function '{}' not found at {}", name, location)
            }
            SemanticError::ArgumentMismatch { function, expected, found, location } => {
                write!(f, "Function '{}' at {} expects {} arguments, got {}", function, location, expected, found)
            }
        }
    }
}

impl std::error::Error for SemanticError {}

impl BlockId {
    pub fn new(id: usize) -> Self {
        BlockId(id)
    }
    
    pub fn index(self) -> usize {
        self.0
    }
}

impl ValueId {
    pub fn new(id: usize) -> Self {
        ValueId(id)
    }
    
    pub fn index(self) -> usize {
        self.0
    }
}

impl Function {
    /// Create a new function with the given name and types
    pub fn new(name: String, params: Vec<Type>, return_type: Type) -> Self {
        Function {
            name,
            params,
            return_type,
            blocks: Vec::new(),
            entry_block: BlockId(0),
            next_value_id: ValueId(0),
            constants: std::collections::HashMap::new(),
        }
    }
    
    /// Generate the next unique value ID
    pub fn next_value(&mut self) -> ValueId {
        let id = self.next_value_id;
        self.next_value_id = ValueId(id.0 + 1);
        id
    }
}

impl BasicBlock {
    /// Create a new basic block
    pub fn new(id: BlockId, label: String) -> Self {
        BasicBlock {
            id,
            label,
            params: Vec::new(),
            instructions: Vec::new(),
            terminator: Terminator::Ret { value: None }, // Placeholder
        }
    }
}

/// Parse a binary operator from a string
impl BinaryOperator {
    pub fn from_str(s: &str, ty: Type) -> Result<Self, SemanticError> {
        match s {
            "add" => Ok(BinaryOperator::Add),
            "sub" => Ok(BinaryOperator::Sub),
            "mul" => Ok(BinaryOperator::Mul),
            "div" => Ok(BinaryOperator::Div),
            "rem" => Ok(BinaryOperator::Rem),
            "and" => Ok(BinaryOperator::And),
            "or" => Ok(BinaryOperator::Or),
            "xor" => Ok(BinaryOperator::Xor),
            "shl" => Ok(BinaryOperator::Shl),
            "shr" => Ok(BinaryOperator::Shr),
            "eq" => Ok(BinaryOperator::Eq),
            "ne" => Ok(BinaryOperator::Ne),
            "lt" => Ok(BinaryOperator::Lt),
            "le" => Ok(BinaryOperator::Le),
            "gt" => Ok(BinaryOperator::Gt),
            "ge" => Ok(BinaryOperator::Ge),
            _ => Err(SemanticError::InvalidOperation {
                operation: s.to_string(),
                ty,
                location: "unknown".to_string(),
            }),
        }
    }
}

impl UnaryOperator {
    pub fn from_str(s: &str, ty: Type) -> Result<Self, SemanticError> {
        match s {
            "neg" => Ok(UnaryOperator::Neg),
            "not" => Ok(UnaryOperator::Not),
            _ => Err(SemanticError::InvalidOperation {
                operation: s.to_string(),
                ty,
                location: "unknown".to_string(),
            }),
        }
    }
}
