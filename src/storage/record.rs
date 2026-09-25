use crate::document::{Document, DocumentId};
use std::io::{self, Read};

pub enum Operation {
    Insert(Document),
    Delete(DocumentId),
    Update(Document),
}

impl Operation {
    fn tag(&self) -> u8 {
        match self {
            Operation::Insert(_) => 0,
            Operation::Delete(_) => 1,
            Operation::Update(_) => 2,
        }
    }
}

// we save the bytes in order as below
#[derive(Debug, PartialEq)]
pub(crate) struct Record {
    pub(crate) op: u8,
    pub(crate) id: DocumentId,
    pub(crate) content: Vec<u8>,
}

impl Record {
    // creates record from operation
    pub(crate) fn from_operation(op: &Operation) -> Self {
        match op {
            Operation::Delete(id) => Record {
                op: op.tag(),
                id: *id,
                content: Vec::new(),
            },
            Operation::Insert(doc) | Operation::Update(doc) => Record {
                op: op.tag(),
                id: doc.id(),
                content: doc.content.as_bytes().to_vec(),
            },
        }
    }

    // encode in order of fields
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut body: Vec<u8> = Vec::new();
        body.push(self.op);
        body.extend_from_slice(&self.id.value().to_le_bytes());
        body.extend_from_slice(&(self.content.len() as u32).to_le_bytes());
        body.extend_from_slice(&self.content);

        let mut buf: Vec<u8> = Vec::with_capacity(4 + body.len());
        let crc: u32 = crc32fast::hash(&body);
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(&body);
        buf
    }

    // assume start at crc (beginning of record)
    pub(crate) fn decode(reader: &mut impl Read) -> io::Result<Self> {
        let mut crc_buf = [0u8; 4];
        reader.read_exact(&mut crc_buf)?;
        let stored_crc = u32::from_le_bytes(crc_buf);

        // the crc covers everything after itself, in on-disk order
        let mut hasher = crc32fast::Hasher::new();

        let mut op = [0u8; 1];
        reader.read_exact(&mut op)?;
        hasher.update(&op);

        let mut id_buf = [0u8; 8];
        reader.read_exact(&mut id_buf)?;
        hasher.update(&id_buf);
        let id: DocumentId = DocumentId(u64::from_le_bytes(id_buf));

        let mut content_len_buf = [0u8; 4];
        reader.read_exact(&mut content_len_buf)?;
        hasher.update(&content_len_buf);
        let content_len: u32 = u32::from_le_bytes(content_len_buf);

        let mut content = vec![0u8; content_len as usize];
        reader.read_exact(&mut content)?;
        hasher.update(&content);

        if hasher.finalize() != stored_crc {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "checksum mismatch",
            ));
        }

        Ok(Record {
            op: op[0],
            id,
            content,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    fn insert(id: u64, content: &str) -> Operation {
        Operation::Insert(Document::new(DocumentId(id), String::from(content)))
    }

    fn round_trip(op: &Operation) -> Record {
        let bytes = Record::from_operation(op).encode();
        let mut reader: &[u8] = &bytes;
        Record::decode(&mut reader).unwrap()
    }

    #[test]
    fn tags_are_stable() {
        assert_eq!(insert(1, "a").tag(), 0);
        assert_eq!(Operation::Delete(DocumentId(1)).tag(), 1);
        assert_eq!(
            Operation::Update(Document::new(DocumentId(1), String::from("a"))).tag(),
            2
        );
    }

    #[test]
    fn encode_lays_out_bytes_in_order() {
        let record = Record::from_operation(&insert(7, "hi"));
        let body: Vec<u8> = vec![
            0, // op: insert
            7, 0, 0, 0, 0, 0, 0, 0, // id: u64 little-endian
            2, 0, 0, 0, // content length: u32 little-endian
            b'h', b'i', // content
        ];
        let mut expected = crc32fast::hash(&body).to_le_bytes().to_vec(); // crc first
        expected.extend_from_slice(&body);
        assert_eq!(record.encode(), expected);
    }

    #[test]
    fn insert_round_trips() {
        let op = insert(42, "hello");
        assert_eq!(round_trip(&op), Record::from_operation(&op));
    }

    #[test]
    fn update_round_trips() {
        let op = Operation::Update(Document::new(DocumentId(9), String::from("new text")));
        assert_eq!(round_trip(&op), Record::from_operation(&op));
    }

    #[test]
    fn delete_round_trips_with_empty_content() {
        let op = Operation::Delete(DocumentId(3));
        let record = round_trip(&op);
        assert_eq!(record.op, 1);
        assert_eq!(record.id, DocumentId(3));
        assert!(record.content.is_empty());
    }

    #[test]
    fn multibyte_content_round_trips() {
        // length on disk is in bytes, not chars: "héllo 🦀" is 7 chars but 11 bytes
        let op = insert(1, "héllo 🦀");
        let record = round_trip(&op);
        assert_eq!(record.content, "héllo 🦀".as_bytes());
    }

    #[test]
    fn large_id_round_trips() {
        let op = insert(u64::MAX, "x");
        assert_eq!(round_trip(&op).id, DocumentId(u64::MAX));
    }

    #[test]
    fn decode_reads_consecutive_records_from_one_reader() {
        let mut bytes = Record::from_operation(&insert(1, "first")).encode();
        bytes.extend(Record::from_operation(&Operation::Delete(DocumentId(1))).encode());
        bytes.extend(Record::from_operation(&insert(2, "second")).encode());

        let mut reader: &[u8] = &bytes;
        let a = Record::decode(&mut reader).unwrap();
        let b = Record::decode(&mut reader).unwrap();
        let c = Record::decode(&mut reader).unwrap();

        assert_eq!(a.content, b"first");
        assert_eq!(b.op, 1);
        assert_eq!(c.id, DocumentId(2));
        assert!(reader.is_empty(), "reader should be fully consumed");
    }

    #[test]
    fn decode_on_empty_input_is_unexpected_eof() {
        let mut reader: &[u8] = &[];
        let err = Record::decode(&mut reader).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }

    #[test]
    fn decode_truncated_record_is_unexpected_eof() {
        let bytes = Record::from_operation(&insert(1, "hello")).encode();
        // cut every prefix short of the full record: header and content cases
        for cut in 0..bytes.len() {
            let mut reader: &[u8] = &bytes[..cut];
            let err = Record::decode(&mut reader).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::UnexpectedEof, "cut at {cut}");
        }
    }

    fn decode_bytes(bytes: &[u8]) -> io::Result<Record> {
        let mut reader = bytes;
        Record::decode(&mut reader)
    }

    #[test]
    fn corrupt_crc_is_invalid_data() {
        let mut bytes = Record::from_operation(&insert(1, "hello")).encode();
        bytes[0] ^= 0x01; // flip a bit inside the stored crc
        let err = decode_bytes(&bytes).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn corrupt_op_is_invalid_data() {
        let mut bytes = Record::from_operation(&insert(1, "hello")).encode();
        bytes[4] ^= 0x01; // op byte
        let err = decode_bytes(&bytes).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn corrupt_id_is_invalid_data() {
        let mut bytes = Record::from_operation(&insert(1, "hello")).encode();
        bytes[5] ^= 0x01; // first id byte
        let err = decode_bytes(&bytes).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn corrupt_content_is_invalid_data() {
        let mut bytes = Record::from_operation(&insert(1, "hello")).encode();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01; // last content byte
        let err = decode_bytes(&bytes).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn corrupt_length_is_detected() {
        let mut bytes = Record::from_operation(&insert(1, "hello")).encode();
        bytes[13] ^= 0x01; // length 5 -> 4: still fits in the file, so only the crc catches it
        let err = decode_bytes(&bytes).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }
}
