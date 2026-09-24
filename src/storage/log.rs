use super::record::{Operation, Record};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};

const MAGIC: [u8; 6] = *b"MEMDB\0";
const VERSION: u8 = 1;
const HEADER_LEN: usize = MAGIC.len() + 1;

pub struct Log {
    file: File,
}

impl Log {
    // opens an existing log
    pub fn open(path: &str) -> io::Result<Self> {
        let mut file = OpenOptions::new().read(true).append(true).open(path)?;

        let mut header = [0u8; HEADER_LEN];
        file.read_exact(&mut header).map_err(|e| match e.kind() {
            io::ErrorKind::UnexpectedEof => invalid_data("not a memdb log"),
            _ => e,
        })?;
        if header[..MAGIC.len()] != MAGIC {
            return Err(invalid_data("not a memdb log"));
        }
        if header[MAGIC.len()] != VERSION {
            return Err(invalid_data("unsupported log version"));
        }
        Ok(Log { file })
    }

    // creates a log if it doesn't exist
    pub fn create(path: &str) -> io::Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create_new(true)
            .open(path)?;

        let mut header = [0u8; HEADER_LEN];
        header[..MAGIC.len()].copy_from_slice(&MAGIC);
        header[MAGIC.len()] = VERSION;
        file.write_all(&header)?;
        file.sync_all()?;
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

fn invalid_data(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, DocumentId};
    use std::io::{ErrorKind, Seek, SeekFrom};

    fn insert(id: u64, content: &str) -> Operation {
        Operation::Insert(Document::new(DocumentId(id), String::from(content)))
    }

    fn temp_path(name: &str) -> String {
        let path = std::env::temp_dir().join(format!("memdb-{}-{}.log", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn first_append_starts_after_the_header() {
        let path = temp_path("first_offset");
        let mut log = Log::create(&path).unwrap();
        assert_eq!(log.append(&insert(1, "hello")).unwrap(), HEADER_LEN as u64);
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

        let start = HEADER_LEN as u64;
        assert_eq!(log.append(&a).unwrap(), start);
        assert_eq!(log.append(&b).unwrap(), start + a_len);
        assert_eq!(log.append(&insert(3, "third")).unwrap(), start + a_len + b_len);
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
        file.seek(SeekFrom::Start(HEADER_LEN as u64)).unwrap();
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
        assert_eq!(
            log.append(&insert(2, "second")).unwrap(),
            HEADER_LEN as u64 + first_len
        );
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

    #[test]
    fn create_writes_the_header() {
        let path = temp_path("header");
        Log::create(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..MAGIC.len()], &MAGIC);
        assert_eq!(bytes[MAGIC.len()], VERSION);
        assert_eq!(bytes.len(), HEADER_LEN);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn open_rejects_a_file_without_the_magic() {
        let path = temp_path("not_a_log");
        std::fs::write(&path, b"definitely not a memdb log file").unwrap();
        let err = Log::open(&path).err().unwrap();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn open_rejects_an_empty_or_short_file() {
        let path = temp_path("short");
        std::fs::write(&path, b"").unwrap();
        assert_eq!(Log::open(&path).err().unwrap().kind(), ErrorKind::InvalidData);
        std::fs::write(&path, &MAGIC[..3]).unwrap();
        assert_eq!(Log::open(&path).err().unwrap().kind(), ErrorKind::InvalidData);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn open_rejects_an_unsupported_version() {
        let path = temp_path("version");
        let mut header = MAGIC.to_vec();
        header.push(VERSION + 1);
        std::fs::write(&path, header).unwrap();
        let err = Log::open(&path).err().unwrap();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        std::fs::remove_file(&path).unwrap();
    }
}
