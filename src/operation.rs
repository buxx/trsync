use std::sync::mpsc::Receiver;

use rusqlite::Connection;

type FilePath = String;
type ContentId = i32;

#[derive(Debug)]
pub enum OperationalMessage {
    // Local files messages
    UnIndexedLocalFileAppear(FilePath),
    IndexedLocalFileModified(FilePath),
    IndexedLocalFileDeleted(FilePath),
    // Remote files messages
    UnIndexedRemoteFileAppear(ContentId),
    IndexedRemoteFileModified(ContentId),
    IndexedRemoteFileDeleted(ContentId),
}

// TODO : Manage a flag set to true when program start to indicate to manage conflicts.
// When resolution done, set flag to false and proceed local and remote messages without
// taking care of conflicts
pub struct OperationalHandler {
    _connection: Connection,
}

impl OperationalHandler {
    pub fn new(connection: Connection) -> Self {
        Self {
            _connection: connection,
        }
    }

    pub fn listen(&mut self, receiver: Receiver<OperationalMessage>) {
        // TODO : Why loop is required ?!
        loop {
            for message in receiver.recv() {
                // TODO : For local files, ignore some patterns : eg. ".*", "*~"
                println!("Message : {:?}", message)
            }
        }
    }
}
