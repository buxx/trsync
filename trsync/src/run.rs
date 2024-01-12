// extern crate notify;
// use crate::client::{ensure_availability, Client};
// use crate::conflict::{ConflictResolver, LocalIsTruth};
// use crate::context::Context;
// use crate::database::{Database, DatabaseOperation};
// use crate::error::Error;
// use crate::local::{start_local_sync, start_local_watch};
// use crate::operation::OperationalMessage;
// use crate::operation::{start_operation, Job};
// use crate::remote::start_remote_sync;
// use crossbeam_channel::Sender as CrossbeamSender;
// use log;
// use std::fs;
// use std::sync::atomic::{AtomicBool, Ordering};
// use std::sync::mpsc::{channel, Receiver, Sender};
// use std::sync::Arc;

// pub fn run(
//     context: Context,
//     stop_signal: Arc<AtomicBool>,
//     activity_sender: Option<CrossbeamSender<WrappedActivity>>,
// ) -> Result<(), Error> {
//     // Digest input folder to watch
//     log::info!(
//         "[{}::{}] Prepare to sync {:?}",
//         context.instance_name,
//         context.workspace_id,
//         &context.folder_path
//     );
//     fs::create_dir_all(&context.folder_path)?;
//     let exit_after_sync = context.exit_after_sync;

//     // Initialize database if needed
//     log::info!(
//         "[{}::{}] Initialize index",
//         context.instance_name,
//         context.workspace_id,
//     );
//     Database::new(context.database_path.clone()).with_new_connection(|connection| {
//         DatabaseOperation::new(&connection).create_tables()?;
//         Ok(())
//     })?;

//     loop {
//         let (operational_sender, operational_receiver): (
//             Sender<OperationalMessage>,
//             Receiver<OperationalMessage>,
//         ) = channel();

//         let restart_signal = Arc::new(AtomicBool::new(false));

//         // Blocks until remote api successfully responded
//         ensure_availability(&context)?;

//         log::info!(
//             "[{}::{}] Start synchronization",
//             context.instance_name,
//             context.workspace_id,
//         );
//         // First, start local sync to know local changes since last start
//         let local_sync_handle = start_local_sync(&context, &operational_sender);
//         // Second, start remote sync to know remote changes since last run
//         let remote_sync_handle = start_remote_sync(&context, &operational_sender);

//         log::info!(
//             "[{}::{}] Start watchers",
//             context.instance_name,
//             context.workspace_id,
//         );
//         // Start local watcher
//         let local_watch_handle =
//             start_local_watch(&context, &operational_sender, &stop_signal, &restart_signal)?;
//         // Start remote watcher
//         let remote_watch_handle =
//             start_remote_watch(&context, &operational_sender, &stop_signal, &restart_signal)?;

//         // Wait end of local and remote sync
//         log::info!(
//             "[{}::{}] Wait synchronizations to finish their jobs",
//             context.instance_name,
//             context.workspace_id,
//         );
//         let local_sync_result = local_sync_handle
//             .join()
//             .expect("Fail to join local sync handler");
//         let remote_sync_result = remote_sync_handle
//             .join()
//             .expect("Fail to join remote sync handler");

//         if let Err(error) = &local_sync_result {
//             log::error!(
//                 "[{}::{}] Local sync failed: {:?}",
//                 context.instance_name,
//                 context.workspace_id,
//                 error,
//             );
//         }
//         if let Err(error) = &remote_sync_result {
//             log::error!(
//                 "[{}::{}] Remote sync failed: {:?}",
//                 context.instance_name,
//                 context.workspace_id,
//                 error,
//             );
//         }
//         if local_sync_result.is_err() || remote_sync_result.is_err() {
//             return Err(Error::StartupError(format!(
//                 "Synchronization fail, interrupt now"
//             )));
//         }

//         if exit_after_sync {
//             stop_signal.swap(true, Ordering::Relaxed);
//             log::info!(
//                 "[{}::{}] Synchronization finished",
//                 context.instance_name,
//                 context.workspace_id,
//             );
//             operational_sender
//                 .send(OperationalMessage::Exit)
//                 .expect("Fail to send exit message");
//         }

//         log::info!(
//             "[{}::{}] Prepare conflicts resolution",
//             context.instance_name,
//             context.workspace_id,
//         );
//         // Handle possible conflict by analyzing operational messages
//         let client = Client::new(context.clone())?;
//         let context_ = context.clone();
//         let strategy = Box::new(LocalIsTruth {});
//         let operational_messages: Vec<OperationalMessage> =
//             operational_receiver.try_iter().collect();
//         log::debug!(
//             "[{}::{}] messages before conflict resolution : {:?}",
//             context.instance_name,
//             context.workspace_id,
//             &operational_messages,
//         );
//         let operational_messages =
//             ConflictResolver::new(context_, client, strategy, operational_messages).resolve();
//         log::debug!(
//             "[{}::{}] messages after conflict resolution : {:?}",
//             context.instance_name,
//             context.workspace_id,
//             &operational_messages,
//         );
//         for message in operational_messages {
//             match operational_sender.send(message) {
//                 Err(error) => {
//                     return Err(Error::UnexpectedError(format!(
//                         "Fail to send message after conflict resolution : {}",
//                         error
//                     )))
//                 }
//                 _ => {}
//             };
//         }

//         log::info!(
//             "[{}::{}] Start operations",
//             context.instance_name,
//             context.workspace_id,
//         );
//         // Operational
//         let operational_handle = start_operation(
//             &context,
//             operational_receiver,
//             &stop_signal,
//             &restart_signal,
//             &activity_sender,
//         );

//         local_watch_handle
//             .join()
//             .expect("Fail to join local listener handler")?;
//         remote_watch_handle
//             .join()
//             .expect("Fail to join remote listener handler")?;
//         operational_handle
//             .join()
//             .expect("Fail to join operational handler")?;

//         if stop_signal.load(Ordering::Relaxed) {
//             log::info!(
//                 "[{}::{}] Stop signal received, interrupt now",
//                 context.instance_name,
//                 context.workspace_id,
//             );
//             break;
//         }
//     }

//     Ok(())
// }
