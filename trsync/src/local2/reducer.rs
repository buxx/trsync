use std::path::PathBuf;

use crossbeam_channel::Receiver;

use crate::{local::DiskEvent, util::TryRemove};

#[derive(Clone)]
pub struct LocalReceiverReducer {
    local_receiver: Receiver<DiskEvent>,
    events: Vec<DiskEvent>,
}

impl LocalReceiverReducer {
    pub fn new(local_receiver: Receiver<DiskEvent>) -> Self {
        Self {
            local_receiver,
            events: vec![],
        }
    }

    pub fn recv(&mut self) -> Result<DiskEventWrap, Error> {
        loop {
            self.events
                .extend(self.local_receiver.try_iter().collect::<Vec<DiskEvent>>());

            if let Some(event) = self.next_event()? {
                return Ok(event);
            } else {
                match self.local_receiver.recv() {
                    Ok(event) => self.events.push(event),
                    Err(_) => return Err(Error::ChannelClosed),
                };
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn next_event(&mut self) -> Result<Option<DiskEventWrap>, Error> {
        let mut disk_event = self
            .events
            .try_remove(0)
            .map(|event| DiskEventWrap::from(&event));
        let mut new_disk_events = vec![];

        while disk_event.is_some() && !self.events.is_empty() {
            'events_test: while let Some(test_disk_event) = self.events.try_remove(0) {
                // FIXME update current path when evolve
                match &DiskEvent::from(disk_event.as_ref().expect("Tested just before")) {
                    DiskEvent::Created(path_a) => match &test_disk_event {
                        DiskEvent::Deleted(path_b) => {
                            if path_a == path_b {
                                // Do not keep a created then deleted file
                                disk_event = self
                                    .events
                                    .try_remove(0)
                                    .map(|event| DiskEventWrap::from(&event));
                                break 'events_test;
                            } else {
                                new_disk_events.push(test_disk_event.clone())
                            }
                        }
                        DiskEvent::Created(_) => new_disk_events.push(test_disk_event.clone()),
                        DiskEvent::Modified(path_b) => {
                            if path_a == path_b {
                                // Create will take the last bytes
                            } else {
                                new_disk_events.push(test_disk_event.clone())
                            }
                        }
                        DiskEvent::Renamed(before_path_b, after_path_b) => {
                            if before_path_b == path_a {
                                // Must track the path change
                                disk_event = Some(DiskEventWrap::new(
                                    disk_event
                                        .as_ref()
                                        .expect("Tested just before")
                                        .stored_path(),
                                    DiskEvent::Created(after_path_b.clone()),
                                ))
                            } else {
                                new_disk_events.push(test_disk_event.clone())
                            }
                        }
                    },
                    DiskEvent::Deleted(_) => {
                        // keep all after a deletion
                        new_disk_events.push(test_disk_event.clone())
                    }
                    DiskEvent::Modified(path_a) => match &test_disk_event {
                        DiskEvent::Deleted(path_b) => {
                            if path_a == path_b {
                                // Do not keep a modified then deleted file
                                disk_event = Some(DiskEventWrap::new(
                                    disk_event
                                        .as_ref()
                                        .expect("Tested just before")
                                        .stored_path(),
                                    DiskEvent::Deleted(path_b.clone()),
                                ));
                                break 'events_test;
                            } else {
                                new_disk_events.push(test_disk_event.clone())
                            }
                        }
                        DiskEvent::Created(_) => new_disk_events.push(test_disk_event.clone()),
                        DiskEvent::Modified(path_b) => {
                            if path_a == path_b {
                                // Modified will take the last bytes
                            } else {
                                new_disk_events.push(test_disk_event.clone())
                            }
                        }
                        DiskEvent::Renamed(before_path_b, after_path_b) => {
                            if before_path_b == path_a {
                                // Must track the path change
                                disk_event = Some(DiskEventWrap::new(
                                    disk_event
                                        .as_ref()
                                        .expect("Tested just before")
                                        .stored_path(),
                                    DiskEvent::Modified(after_path_b.clone()),
                                ));
                            } else {
                                new_disk_events.push(test_disk_event.clone())
                            }
                        }
                    },
                    DiskEvent::Renamed(_, after_path_a) => match &test_disk_event {
                        DiskEvent::Deleted(path_b) => {
                            if after_path_a == path_b {
                                // Do not keep a rename then deleted file
                                disk_event = Some(DiskEventWrap::new(
                                    disk_event
                                        .as_ref()
                                        .expect("Tested just before")
                                        .stored_path(),
                                    DiskEvent::Deleted(after_path_a.clone()),
                                ));
                                break 'events_test;
                            } else {
                                new_disk_events.push(test_disk_event.clone())
                            }
                        }
                        DiskEvent::Created(_) => new_disk_events.push(test_disk_event.clone()),
                        DiskEvent::Modified(path_b) => {
                            if after_path_a == path_b {
                                // Modified will take the last bytes
                            } else {
                                new_disk_events.push(test_disk_event.clone())
                            }
                        }
                        DiskEvent::Renamed(before_path_b, after_path_b) => {
                            if after_path_a == before_path_b {
                                disk_event = Some(DiskEventWrap::new(
                                    disk_event
                                        .as_ref()
                                        .expect("Tested just before")
                                        .stored_path(),
                                    DiskEvent::Renamed(before_path_b.clone(), after_path_b.clone()),
                                ));
                            } else {
                                new_disk_events.push(test_disk_event.clone())
                            }
                        }
                    },
                }
            }
        }

        self.events = new_disk_events;
        Ok(disk_event.clone())
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct DiskEventWrap(pub PathBuf, pub DiskEvent);

impl From<&DiskEventWrap> for DiskEvent {
    fn from(value: &DiskEventWrap) -> Self {
        value.1.clone()
    }
}

impl From<&DiskEvent> for DiskEventWrap {
    fn from(value: &DiskEvent) -> Self {
        match value {
            DiskEvent::Deleted(path)
            | DiskEvent::Created(path)
            | DiskEvent::Modified(path)
            | DiskEvent::Renamed(path, _) => Self::new(path.clone(), value.clone()),
        }
    }
}

impl DiskEventWrap {
    pub fn stored_path(&self) -> PathBuf {
        self.0.clone()
    }

    pub fn new(path: PathBuf, event: DiskEvent) -> DiskEventWrap {
        Self(path, event)
    }
}

#[derive(Debug)]
pub enum Error {
    ChannelClosed,
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::event::Event;
    use crate::local::DiskEvent;
    use crossbeam_channel::{unbounded, Receiver, Sender};
    use rstest::*;

    use super::*;

    #[rstest]
    #[case(vec![], vec![])]
    #[case(
        vec![
            DiskEvent::Created(PathBuf::from("a.txt")),
            DiskEvent::Deleted(PathBuf::from("a.txt")),
        ],
        vec![],
    )]
    #[case(
        vec![
            DiskEvent::Created(PathBuf::from("a.txt")),
            DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Created(PathBuf::from("b.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Created(PathBuf::from("a.txt")),
            DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")),
            DiskEvent::Deleted(PathBuf::from("a.txt")),
        ],
        vec![],
    )]
    #[case(
        vec![
            DiskEvent::Created(PathBuf::from("a.txt")),
            DiskEvent::Modified(PathBuf::from("a.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Created(PathBuf::from("a.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Created(PathBuf::from("a.txt")),
            DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")),
            DiskEvent::Renamed(PathBuf::from("b.txt"), PathBuf::from("c.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Created(PathBuf::from("c.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Modified(PathBuf::from("a.txt")),
            DiskEvent::Deleted(PathBuf::from("a.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Deleted(PathBuf::from("a.txt")))
        ],
    )]
    #[case(
        vec![
            DiskEvent::Modified(PathBuf::from("a.txt")),
            DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Modified(PathBuf::from("b.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Modified(PathBuf::from("a.txt")),
            DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")),
            DiskEvent::Renamed(PathBuf::from("b.txt"), PathBuf::from("c.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Modified(PathBuf::from("c.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Modified(PathBuf::from("a.txt")),
            DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")),
            DiskEvent::Deleted(PathBuf::from("b.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Deleted(PathBuf::from("b.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")),
            DiskEvent::Renamed(PathBuf::from("b.txt"), PathBuf::from("c.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Renamed(PathBuf::from("b.txt"), PathBuf::from("c.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")),
            DiskEvent::Renamed(PathBuf::from("b.txt"), PathBuf::from("c.txt")),
            DiskEvent::Renamed(PathBuf::from("c.txt"), PathBuf::from("d.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Renamed(PathBuf::from("c.txt"), PathBuf::from("d.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")),
            DiskEvent::Modified(PathBuf::from("c.txt")),
            DiskEvent::Renamed(PathBuf::from("c.txt"), PathBuf::from("d.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt"))),
            DiskEventWrap::new(PathBuf::from("c.txt"), DiskEvent::Modified(PathBuf::from("d.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")),
            DiskEvent::Deleted(PathBuf::from("b.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Deleted(PathBuf::from("b.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Created(PathBuf::from("a.txt")),
            DiskEvent::Created(PathBuf::from("b.txt")),
            DiskEvent::Created(PathBuf::from("c.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Created(PathBuf::from("a.txt"))),
            DiskEventWrap::new(PathBuf::from("b.txt"), DiskEvent::Created(PathBuf::from("b.txt"))),
            DiskEventWrap::new(PathBuf::from("c.txt"), DiskEvent::Created(PathBuf::from("c.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Modified(PathBuf::from("a.txt")),
            DiskEvent::Modified(PathBuf::from("b.txt")),
            DiskEvent::Modified(PathBuf::from("c.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Modified(PathBuf::from("a.txt"))),
            DiskEventWrap::new(PathBuf::from("b.txt"), DiskEvent::Modified(PathBuf::from("b.txt"))),
            DiskEventWrap::new(PathBuf::from("c.txt"), DiskEvent::Modified(PathBuf::from("c.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Deleted(PathBuf::from("a.txt")),
            DiskEvent::Deleted(PathBuf::from("b.txt")),
            DiskEvent::Deleted(PathBuf::from("c.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Deleted(PathBuf::from("a.txt"))),
            DiskEventWrap::new(PathBuf::from("b.txt"), DiskEvent::Deleted(PathBuf::from("b.txt"))),
            DiskEventWrap::new(PathBuf::from("c.txt"), DiskEvent::Deleted(PathBuf::from("c.txt"))),
        ],
    )]
    #[case(
        vec![
            DiskEvent::Deleted(PathBuf::from("a.txt")),
            DiskEvent::Created(PathBuf::from("a.txt")),
            DiskEvent::Deleted(PathBuf::from("a.txt")),
            DiskEvent::Created(PathBuf::from("a.txt")),
        ],
        vec![
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Deleted(PathBuf::from("a.txt"))),
            DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Created(PathBuf::from("a.txt"))),
        ],
    )]
    fn test_local_receiver_reducer(
        #[case] given: Vec<DiskEvent>,
        #[case] expected: Vec<DiskEventWrap>,
    ) {
        // Given
        let (_op_sender, _): (Sender<Event>, Receiver<Event>) = unbounded();
        let (local_sender, local_receiver): (Sender<DiskEvent>, Receiver<DiskEvent>) = unbounded();
        let mut reducer = LocalReceiverReducer::new(local_receiver);
        given
            .into_iter()
            .for_each(|event| local_sender.send(event).unwrap());

        // When Then
        let result = expected
            .iter()
            .map(|_| reducer.recv().unwrap())
            .collect::<Vec<DiskEventWrap>>();
        assert_eq!(result, expected);
        assert!(reducer.is_empty())
    }
}
