/// Unique identifier for a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(pub u64);

/// A stored document.
#[derive(Debug, Clone)]
pub struct Document {
    pub id: DocumentId,
    pub content: String,
}
