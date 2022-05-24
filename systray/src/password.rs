use std::str::FromStr;

use tiny_http::{HeaderField, Method, Response, Server, StatusCode};

pub fn start_password_receiver_server(port: u16, token: &str) {
    let server = Server::http(format!("127.0.0.1:{}", port)).unwrap();
    let token_ = token.to_string();
    let username = whoami::username();
    // Note : This is an extremely simple http server.
    // Solve keyring python/rust problem or replace this by a more modern http server
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            if request
                .headers()
                .iter()
                .find(|h| {
                    h.field
                        == HeaderField::from_str("X-Auth-Token")
                            .expect("This header field should be valid")
                        && h.value == token_
                })
                .is_none()
            {
                log::error!("Password receiver: invalid token");
                // TODO : manage error
                request
                    .respond(Response::empty(StatusCode::from(403)))
                    .unwrap();
                continue;
            }

            if let Some(instance_name) = request.url().split("/").last() {
                let instance_name = instance_name.to_string();

                // Update a password
                if request.method() == &Method::Post {
                    log::info!("Save password for instance name '{}'", instance_name);
                    let mut password = String::new();
                    if let Err(error) = request.as_reader().read_to_string(&mut password) {
                        log::error!("Unable to read sent password: '{}', abort", error);
                        if let Err(error) = request.respond(Response::empty(StatusCode::from(500)))
                        {
                            log::error!("Unable to respond to client : '{}'", error);
                        }
                        continue;
                    };

                    if let Err(error) =
                        trsync_manager::security::set_password(&instance_name, &username, &password)
                    {
                        log::error!("Unable to save password: '{}', abort", error);
                        if let Err(error) = request.respond(Response::empty(StatusCode::from(500)))
                        {
                            log::error!("Unable to respond to client : '{}'", error);
                        }
                        continue;
                    }

                    if let Err(error) = request.respond(Response::empty(StatusCode::from(201))) {
                        log::error!("Unable to respond to client : '{}'", error);
                    }
                    continue;
                }
                // Request a password
                else if request.method() == &Method::Get {
                    log::info!("Get password for instance name '{}'", instance_name);
                    match trsync_manager::security::get_password(&instance_name, &username) {
                        Ok(password) => {
                            if let Err(error) = request.respond(Response::from_string(password)) {
                                log::error!("Unable to respond to client : '{}'", error);
                            }
                        }
                        Err(error) => {
                            log::error!("Unable to get password: '{}', abort", error);
                            if let Err(error) =
                                request.respond(Response::empty(StatusCode::from(500)))
                            {
                                log::error!("Unable to respond to client : '{}'", error);
                            }
                            continue;
                        }
                    }
                }
            }
        }
    });
}
