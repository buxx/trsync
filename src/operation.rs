use std::sync::mpsc::Receiver;

use rusqlite::Connection;

type FilePath = String;

#[derive(Debug)]
pub enum OperationalMessage {
    // Local files messages
    UnIndexedLocalFileAppear(FilePath),
    IndexedLocalFileModified(FilePath),
    IndexedLocalFileDeleted(FilePath),
    // Remote files messages
    FakeMessage,
}

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
                println!("Message : {:?}", message)
            }
        }
    }
}
