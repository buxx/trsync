use std::sync::mpsc::Receiver;

use rusqlite::Connection;

type FilePath = String;
type ContentId = i32;

#[derive(Debug)]
pub enum OperationalMessage {
    // Local files messages
    NewLocalFile(FilePath),
    ModifiedLocalFile(FilePath),
    DeletedLocalFile(FilePath),
    // Remote files messages
    NewRemoteFile(ContentId),
    ModifiedRemoteFile(ContentId),
    DeletedRemoteFile(ContentId),
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
                println!("MESSAGE : {:?}", &message);
                // TODO : For local files, ignore some patterns : eg. ".*", "*~"
                match message {
                    // Local changes
                    OperationalMessage::NewLocalFile(path) => self.new_local_file(path),
                    OperationalMessage::ModifiedLocalFile(path) => self.modified_local_file(path),
                    OperationalMessage::DeletedLocalFile(path) => self.deleted_local_file(path),
                    // Remote changes
                    OperationalMessage::NewRemoteFile(content_id) => {
                        self.new_remote_file(content_id)
                    }
                    OperationalMessage::ModifiedRemoteFile(content_id) => {
                        self.modified_remote_file(content_id)
                    }
                    OperationalMessage::DeletedRemoteFile(content_id) => {
                        self.deleted_remote_file(content_id)
                    }
                }
            }
        }
    }

    fn new_local_file(&self, path: String) {
        // TODO : POST new file on api (take car on fallback event !)
        // TODO : Add it in database (move code here)
    }

    fn modified_local_file(&self, path: String) {
        // TODO : POST new content on api (take car on fallback event !)
        // TODO : Update it in database (move code here)
    }

    fn deleted_local_file(&self, path: String) {
        // TODO : DELETE content on api (take car on fallback event !)
        // TODO : remove it from database (move code here)
    }

    fn new_remote_file(&self, content_id: i32) {
        // TODO : Get content path, filename, then content and create local file on disk
        // TODO : Insert it in database (move code here?)
    }

    fn modified_remote_file(&self, content_id: i32) {
        // TODO : Get new file content, and update local disk file (path is in db)
        // TODO : Update it in database (move code here ?)
    }

    fn deleted_remote_file(&self, content_id: i32) {
        // TODO : Delete local disk file (path is in db)
        // TODO : Delete it from database (move code here ?)
    }
}
