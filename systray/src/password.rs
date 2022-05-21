use crossbeam_channel::Sender;
use tiny_http::{Response, Server, StatusCode};

pub fn start_password_receiver_server(
    main_channel_sender: Sender<trsync_manager::message::DaemonControlMessage>,
    port: u16,
) {
    let server = Server::http(format!("127.0.0.1:{}", port)).unwrap();
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            log::debug!(
                "received request! method: {:?}, url: {:?}, headers: {:?}",
                request.method(),
                request.url(),
                request.headers()
            );

            if let Some(instance_name) = request.url().split("/").last() {
                let instance_name = instance_name.to_string();
                log::info!("Save password for instance name '{}'", instance_name);
                let mut password = String::new();
                if let Err(error) = request.as_reader().read_to_string(&mut password) {
                    log::error!("Unable to read sent password: '{}', abort", error);
                    if let Err(error) = request.respond(Response::empty(StatusCode::from(500))) {
                        log::error!("Unable to respond to client : '{}'", error);
                    }
                    break;
                };

                if let Err(error) = main_channel_sender.send(
                    trsync_manager::message::DaemonControlMessage::StorePassword(
                        instance_name.to_string(),
                        password.to_string(),
                    ),
                ) {
                    log::error!("Unable to send message to main thread: '{}'", error);
                };
            }

            let response = Response::from_string("hello world");
            // FIXME : manage (error must not stop thread)
            request.respond(response).unwrap();
            // FIXME : if ok, send message to main thread to save password
        }
    });
}
