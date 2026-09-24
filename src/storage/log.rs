use super::record::{Operation, Record};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};

pub struct Log {
    file: File,
}

impl Log {
    // opens an existing log
    pub fn open(path: &str) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).append(true).open(path)?;
        Ok(Log { file })
    }

    // creates a log if it doesn't exist
    pub fn create(path: &str) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .create_new(true)
            .open(path)?;
        Ok(Log { file })
    }

    /// Appends a record, syncs it to disk, and returns its starting byte offset.
    pub fn append(&mut self, op: &Operation) -> io::Result<u64> {
        let offset = self.file.metadata()?.len();
        let bytes = Record::from_operation(op).encode();

        let result = self.file.write_all(&bytes).and_then(|()| self.file.sync_all());
        if let Err(e) = result {
            // drop any partial record so the next append doesn't land after it
            let _ = self.file.set_len(offset);
            return Err(e);
        }
        Ok(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, DocumentId};
    use std::io::ErrorKind;

    fn insert(id: u64, content: &str) -> Operation {
        Operation::Insert(Document::new(DocumentId(id), String::from(content)))
    }

    fn temp_path(name: &str) -> String {
        let path = std::env::temp_dir().join(format!("memdb-{}-{}.log", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn first_append_returns_offset_zero() {
        let path = temp_path("first_offset");
        let mut log = Log::create(&path).unwrap();
        assert_eq!(log.append(&insert(1, "hello")).unwrap(), 0);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn each_append_returns_the_end_of_the_previous_record() {
        let path = temp_path("offsets");
        let mut log = Log::create(&path).unwrap();

        let a = insert(1, "first");
        let b = insert(2, "second record");
        let a_len = Record::from_operation(&a).encode().len() as u64;
        let b_len = Record::from_operation(&b).encode().len() as u64;

        assert_eq!(log.append(&a).unwrap(), 0);
        assert_eq!(log.append(&b).unwrap(), a_len);
        assert_eq!(log.append(&insert(3, "third")).unwrap(), a_len + b_len);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn appended_records_can_be_read_back_in_order() {
        let path = temp_path("read_back");
        let mut log = Log::create(&path).unwrap();
        log.append(&insert(1, "first")).unwrap();
        log.append(&Operation::Delete(DocumentId(1))).unwrap();
        log.append(&insert(2, "second")).unwrap();

        let mut file = File::open(&path).unwrap();
        let a = Record::decode(&mut file).unwrap();
        let b = Record::decode(&mut file).unwrap();
        let c = Record::decode(&mut file).unwrap();
        assert_eq!(a.content, b"first");
        assert_eq!(b.op, 1);
        assert_eq!(c.id, DocumentId(2));

        let err = Record::decode(&mut file).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn reopening_a_log_appends_after_existing_records() {
        let path = temp_path("reopen");
        let first = insert(1, "first");
        let first_len = Record::from_operation(&first).encode().len() as u64;

        let mut log = Log::create(&path).unwrap();
        log.append(&first).unwrap();
        drop(log);

        let mut log = Log::open(&path).unwrap();
        assert_eq!(log.append(&insert(2, "second")).unwrap(), first_len);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn create_fails_if_the_file_already_exists() {
        let path = temp_path("create_twice");
        Log::create(&path).unwrap();
        let err = Log::create(&path).err().unwrap();
        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        std::fs::remove_file(&path).unwrap();
    }
}
