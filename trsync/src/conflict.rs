use trsync_core::types::RelativeFilePath;

use crate::{
    client::Client,
    context::Context,
    error::ClientError,
    knowledge::{ExternalKnowledge, Knowledge},
    operation::OperationalMessage,
};

pub struct ConflictResolver {
    knowledge: Box<dyn Knowledge>,
    strategy: Box<dyn ResolveStrategy>,
    messages: Vec<OperationalMessage>,
}

impl ConflictResolver {
    pub fn new(
        context: Context,
        client: Client,
        strategy: Box<dyn ResolveStrategy>,
        messages: Vec<OperationalMessage>,
    ) -> Self {
        let knowledge = Box::new(ExternalKnowledge::new(context, client));
        Self {
            knowledge,
            strategy,
            messages,
        }
    }

    /// Resolve conflicts between local and remote files by consuming operational message
    /// and look at conflicts. Then, merge conflicted messages by found solutions
    pub fn resolve(&self) -> Vec<OperationalMessage> {
        log::info!(
            "[{}::{}] Start conflict resolution",
            self.knowledge.instance_name(),
            self.knowledge.workspace_id(),
        );

        let mut outgoing = vec![];
        let (operations, original_left) = self.prepare();
        let (mut local_operations, mut remote_operations) = self.split_operations(operations);

        while let Some((relative_path, message)) = &local_operations.pop() {
            if let Some(conflicting_message) =
                self.get_conflicting_message(relative_path, &remote_operations)
            {
                log::warn!(
                    "[{}::{}] Conflict found for {}",
                    self.knowledge.instance_name(),
                    self.knowledge.workspace_id(),
                    relative_path
                );

                // Conflicting messages will be managed by the strategy, so we can remove them from remote_operations
                remote_operations.retain(|o| &conflicting_message != &o.1);

                let solutions = self
                    .strategy
                    .resolve_conflict(message, &conflicting_message);
                outgoing.extend(solutions);
            } else {
                // No conflicts means deal with original message
                outgoing.push(message.clone());
            }
        }

        // Left remote messages can be added to outgoing
        outgoing.extend(remote_operations.into_iter().map(|(_, m)| m));
        outgoing.extend(original_left);

        outgoing
    }

    fn prepare(
        &self,
    ) -> (
        Vec<(RelativeFilePath, OperationalMessage)>,
        Vec<OperationalMessage>,
    ) {
        let mut operations: Vec<(RelativeFilePath, OperationalMessage)> = vec![];
        let mut outgoing: Vec<OperationalMessage> = vec![];

        for message in &self.messages {
            match message {
                OperationalMessage::NewLocalFile(relative_path)
                | OperationalMessage::ModifiedLocalFile(relative_path)
                | OperationalMessage::DeletedLocalFile(relative_path) => {
                    // For a local change, use local relative path change
                    operations.push((relative_path.clone(), message.clone()));
                }
                OperationalMessage::RenamedLocalFile(relative_path, _) => {
                    operations.push((relative_path.clone(), message.clone()));
                }
                OperationalMessage::NewRemoteFile(content_id) => {
                    // For a new remote file, use remote relative path for reference
                    let relative_path = match self.knowledge.get_remote_relative_path(*content_id) {
                        Ok(relative_path_) => relative_path_,
                        Err(ClientError::NotRelevant(message)) => {
                            log::info!(
                                "[{}::{}] {}",
                                self.knowledge.instance_name(),
                                self.knowledge.workspace_id(),
                                message,
                            );
                            continue;
                        }
                        Err(error) => {
                            log::error!(
                                "[{}::{}] Error while building relative path: {}",
                                self.knowledge.instance_name(),
                                self.knowledge.workspace_id(),
                                error,
                            );
                            continue;
                        }
                    };

                    operations.push((relative_path, message.clone()));
                }
                OperationalMessage::ModifiedRemoteFile(content_id)
                | OperationalMessage::DeletedRemoteFile(content_id) => {
                    // For a remote change, use local relative path
                    let relative_path = match self.knowledge.get_local_relative_path(*content_id) {
                        Ok(relative_path_) => relative_path_,
                        Err(error) => {
                            log::error!(
                                "[{}::{}] Error while matching local relative path: {}",
                                self.knowledge.instance_name(),
                                self.knowledge.workspace_id(),
                                error,
                            );
                            continue;
                        }
                    };

                    operations.push((relative_path, message.clone()));
                }
                OperationalMessage::Exit => {
                    // If there is an Exit message, just follow it
                    outgoing.push(message.clone());
                }
            }
        }

        (operations, outgoing)
    }

    fn split_operations(
        &self,
        operations: Vec<(RelativeFilePath, OperationalMessage)>,
    ) -> (
        Vec<(RelativeFilePath, OperationalMessage)>,
        Vec<(RelativeFilePath, OperationalMessage)>,
    ) {
        let mut local_operations = vec![];
        let mut remote_operations = vec![];

        for operation in operations {
            match operation.1 {
                OperationalMessage::NewLocalFile(_)
                | OperationalMessage::ModifiedLocalFile(_)
                | OperationalMessage::DeletedLocalFile(_)
                | OperationalMessage::RenamedLocalFile(_, _) => {
                    local_operations.push(operation);
                }
                OperationalMessage::NewRemoteFile(_)
                | OperationalMessage::ModifiedRemoteFile(_)
                | OperationalMessage::DeletedRemoteFile(_) => {
                    remote_operations.push(operation);
                }
                OperationalMessage::Exit => unreachable!(),
            }
        }

        (local_operations, remote_operations)
    }

    fn get_conflicting_message(
        &self,
        relative_path: &RelativeFilePath,
        operations: &Vec<(RelativeFilePath, OperationalMessage)>,
    ) -> Option<OperationalMessage> {
        let mut matching_messages: Vec<OperationalMessage> = vec![];

        for (relative_path_, message) in operations {
            if relative_path_ == relative_path {
                matching_messages.push(message.clone());
            }
        }

        if matching_messages.len() == 0 {
            return None;
        }

        if matching_messages.len() == 1 {
            return Some(matching_messages[0].clone());
        }

        log::error!(
            "Conflicting message should always be alone ! But found : {:?}",
            matching_messages
        );
        Some(matching_messages[0].clone())
    }
}

pub trait ResolveStrategy {
    fn resolve_conflict(
        &self,
        local_message: &OperationalMessage,
        remote_message: &OperationalMessage,
    ) -> Vec<OperationalMessage>;
}

pub struct LocalIsTruth;

impl ResolveStrategy for LocalIsTruth {
    fn resolve_conflict(
        &self,
        local_message: &OperationalMessage,
        remote_message: &OperationalMessage,
    ) -> Vec<OperationalMessage> {
        match local_message {
            OperationalMessage::NewLocalFile(relative_path) => match remote_message {
                OperationalMessage::NewRemoteFile(_) => {
                    vec![OperationalMessage::ModifiedLocalFile(relative_path.clone())]
                }
                OperationalMessage::ModifiedRemoteFile(_) => {
                    vec![OperationalMessage::ModifiedLocalFile(relative_path.clone())]
                }
                OperationalMessage::DeletedRemoteFile(_) => {
                    vec![local_message.clone()]
                }
                _ => unreachable!(),
            },
            OperationalMessage::ModifiedLocalFile(relative_path) => match remote_message {
                OperationalMessage::NewRemoteFile(_) => {
                    vec![local_message.clone()]
                }
                OperationalMessage::ModifiedRemoteFile(_) => {
                    vec![local_message.clone()]
                }
                OperationalMessage::DeletedRemoteFile(_) => {
                    vec![OperationalMessage::NewLocalFile(relative_path.clone())]
                }
                _ => unreachable!(),
            },
            OperationalMessage::DeletedLocalFile(_) => match remote_message {
                OperationalMessage::NewRemoteFile(_) => {
                    vec![local_message.clone()]
                }
                OperationalMessage::ModifiedRemoteFile(_) => {
                    vec![local_message.clone()]
                }
                OperationalMessage::DeletedRemoteFile(_) => {
                    vec![]
                }
                _ => unreachable!(),
            },
            OperationalMessage::RenamedLocalFile(_, relative_path) => match remote_message {
                OperationalMessage::NewRemoteFile(_) => {
                    vec![local_message.clone()]
                }
                OperationalMessage::ModifiedRemoteFile(_) => {
                    vec![local_message.clone()]
                }
                OperationalMessage::DeletedRemoteFile(_) => {
                    vec![OperationalMessage::NewLocalFile(relative_path.clone())]
                }
                // ResolveStrategy only deals with remote message as comparative
                _ => unreachable!(),
            },

            // ResolveStrategy only deals with local message as reference
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::MockKnowledge;
    use rstest::*;

    #[rstest]
    // Empty
    #[case(vec![], vec![])]
    // Modified both
    #[case(
        vec![
            OperationalMessage::ModifiedLocalFile("a.txt".to_string()),
            OperationalMessage::ModifiedRemoteFile(1),
        ],
        vec![
            OperationalMessage::ModifiedLocalFile("a.txt".to_string())
        ]
    )]
    // Modified locally, deleted remotely
    #[case(
        vec![
            OperationalMessage::ModifiedLocalFile("a.txt".to_string()),
            OperationalMessage::DeletedRemoteFile(1),
        ],
        vec![
            OperationalMessage::NewLocalFile("a.txt".to_string())
        ]
    )]
    // New locally, New remotely
    #[case(
        vec![
            OperationalMessage::NewLocalFile("a.txt".to_string()),
            OperationalMessage::NewRemoteFile(1),
        ],
        vec![
            OperationalMessage::ModifiedLocalFile("a.txt".to_string())
        ]
    )]
    // New locally, deleted remotely
    #[case(
        vec![
            OperationalMessage::NewLocalFile("a.txt".to_string()),
            OperationalMessage::DeletedRemoteFile(1),
        ],
        vec![
            OperationalMessage::NewLocalFile("a.txt".to_string())
        ]
    )]
    // Renamed locally, modified remotely
    #[case(
        vec![
            OperationalMessage::RenamedLocalFile("a.txt".to_string(), "b.txt".to_string()),
            OperationalMessage::ModifiedRemoteFile(1),
        ],
        vec![
            OperationalMessage::RenamedLocalFile("a.txt".to_string(), "b.txt".to_string())
        ]
    )]
    // Renamed locally, new remotely
    #[case(
        vec![
            OperationalMessage::RenamedLocalFile("a.txt".to_string(), "b.txt".to_string()),
            OperationalMessage::NewRemoteFile(1),
        ],
        vec![
            OperationalMessage::RenamedLocalFile("a.txt".to_string(), "b.txt".to_string())
        ]
    )]
    // Renamed locally, deleted remotely
    #[case(
        vec![
            OperationalMessage::RenamedLocalFile("a.txt".to_string(), "b.txt".to_string()),
            OperationalMessage::DeletedRemoteFile(1),
        ],
        vec![
            OperationalMessage::NewLocalFile("b.txt".to_string())
        ]
    )]
    // Exit message musty be at the end
    #[case(
        vec![
            OperationalMessage::NewRemoteFile(1),
            OperationalMessage::NewRemoteFile(2),
            OperationalMessage::Exit,
        ],
        vec![
            OperationalMessage::NewRemoteFile(1),
            OperationalMessage::NewRemoteFile(2),
            OperationalMessage::Exit,
        ]
    )]
    fn multiple_cases(
        #[case] input: Vec<OperationalMessage>,
        #[case] expected: Vec<OperationalMessage>,
    ) {
        // Given
        let mut knowledge = Box::new(MockKnowledge::new());
        knowledge
            .expect_get_local_relative_path()
            .returning(|i| match i {
                1 => Ok("a.txt".to_string()),
                2 => Ok("b.txt".to_string()),
                _ => unreachable!(),
            });
        knowledge
            .expect_get_remote_relative_path()
            .returning(|i| match i {
                1 => Ok("a.txt".to_string()),
                2 => Ok("b.txt".to_string()),
                _ => unreachable!(),
            });
        let resolver = ConflictResolver {
            knowledge,
            strategy: Box::new(LocalIsTruth {}),
            messages: input,
        };

        // When
        let messages = resolver.resolve();

        // Then
        assert_eq!(expected, messages)
    }
}
