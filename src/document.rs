#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(pub u64);

impl DocumentId {
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// A stored document.
/// consider using Arc instead of clone-on-read for shared pointer
/// though arc, we would need to encorporate mutex if we allow parallel writes
#[derive(Debug, Clone)]
pub struct Document {
    id: DocumentId,
    pub content: String,
}

impl Document {
    pub fn new(id: DocumentId, content: String) -> Self {
        Document { id, content }
    }

    pub fn id(&self) -> DocumentId {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use std::{assert_eq, assert_ne};

    use super::*;

    fn create_document(id: u64, content: &str) -> Document {
        Document::new(DocumentId(id), String::from(content))
    }

    #[test]
    fn creates_a_document_with_given_id_and_content() {
        let document = create_document(1, "hello");
        assert_eq!(document.id().value(), 1);
        assert_eq!(document.content, "hello");
    }

    #[test]
    fn document_ids_with_same_value_are_equal() {
        let document1 = create_document(1, "hello");
        let document2 = create_document(2, "world");
        assert_eq!(document1.id().value(), 1);
        assert_eq!(document1.content, "hello");
        assert_eq!(document2.id().value(), 2);
        assert_eq!(document2.content, "world");
    }

    #[test]
    fn cloning_a_document_produces_an_independent_copy() {
        let d1 = create_document(1, "hello");
        let mut d1_clone = d1.clone();
        assert_eq!(d1.id().value(), d1_clone.id().value());
        assert_eq!(d1.content, d1_clone.content);
        d1_clone.content = String::from("world");
        assert_ne!(d1.content, d1_clone.content);
    }
}
