//! Assembly Semantic Intermediate Representation (ASIR).
//!
//! ASIR is an architecture-neutral analysis form derived from assembly and
//! metadata. Lowering from physical instructions is implemented per target in
//! later slices; this crate only defines the core graph shape.

#![forbid(unsafe_code)]

use semasm_core::FunctionId;

/// Identifier of a basic block within a function body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(u32);

impl BlockId {
    /// Create a block id.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Raw id value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Identifier of a value (register abstraction, temporary, or constant handle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueId(u32);

impl ValueId {
    /// Create a value id.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Raw id value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// High-level semantic operation kinds used during analysis.
///
/// Physical instruction mapping is target-specific and lives outside this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpKind {
    /// No-op or padding with no semantic effect.
    Nop,
    /// Load from memory.
    Load,
    /// Store to memory.
    Store,
    /// Integer or pointer arithmetic.
    Binary,
    /// Comparison producing a condition.
    Compare,
    /// Conditional or unconditional branch.
    Branch,
    /// Direct or indirect call.
    Call,
    /// Function return.
    Return,
    /// Explicit marker for unmodeled target operations.
    Unknown,
}

/// A single ASIR operation (placeholder payload until full IR lands).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// Semantic kind.
    pub kind: OpKind,
    /// Optional result value.
    pub result: Option<ValueId>,
}

impl Operation {
    /// Create an operation of the given kind with no result.
    #[must_use]
    pub fn new(kind: OpKind) -> Self {
        Self { kind, result: None }
    }
}

/// A basic block containing a linear sequence of operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    /// Block identity.
    pub id: BlockId,
    /// Operations in program order.
    pub ops: Vec<Operation>,
}

impl BasicBlock {
    /// Create an empty basic block.
    #[must_use]
    pub fn new(id: BlockId) -> Self {
        Self {
            id,
            ops: Vec::new(),
        }
    }
}

/// ASIR body for one function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionBody {
    /// Function identity shared with contracts and symbols.
    pub id: FunctionId,
    /// Entry block, if present.
    pub entry: Option<BlockId>,
    /// Blocks in this function.
    pub blocks: Vec<BasicBlock>,
}

impl FunctionBody {
    /// Create an empty function body.
    #[must_use]
    pub fn new(id: FunctionId) -> Self {
        Self {
            id,
            entry: None,
            blocks: Vec::new(),
        }
    }

    /// Number of blocks.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }
}

/// A module-level ASIR unit (one or more functions).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Module {
    /// Functions in this module.
    pub functions: Vec<FunctionBody>,
}

impl Module {
    /// Create an empty module.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of functions.
    #[must_use]
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semasm_core::FunctionId;

    #[test]
    fn empty_module() {
        let m = Module::new();
        assert_eq!(m.function_count(), 0);
    }

    #[test]
    fn function_with_block() {
        let mut body = FunctionBody::new(FunctionId::new(0));
        let block = BasicBlock::new(BlockId::new(0));
        body.entry = Some(block.id);
        body.blocks.push(block);
        assert_eq!(body.block_count(), 1);
        assert_eq!(body.entry.unwrap().get(), 0);
    }

    #[test]
    fn operation_kinds_cover_core_set() {
        assert_eq!(Operation::new(OpKind::Return).kind, OpKind::Return);
        assert_eq!(Operation::new(OpKind::Unknown).result, None);
    }
}
