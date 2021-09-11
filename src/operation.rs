use std::sync::mpsc::Receiver;

use rusqlite::Connection;

#[derive(Debug)]
pub enum OperationalMessage {
    NewRemoteRevision,
    NewLocalRevision,
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
