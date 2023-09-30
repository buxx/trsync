use self::{local::LocalEvent, remote::RemoteEvent};

pub mod local;
pub mod remote;

#[derive(Debug)]
pub enum Event {
    Remote(RemoteEvent),
    Local(LocalEvent),
}
