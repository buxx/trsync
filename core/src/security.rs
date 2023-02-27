extern crate keyring;

use std::error::Error;

pub fn get_password(instance_address: &str, username: &str) -> Result<String, Box<dyn Error>> {
    let service = format!("trsync::{}", instance_address);
    let entry = keyring::Entry::new(&service, username);
    log::info!(
        "Get password for service '{}' and user '{}'",
        &service,
        &username
    );
    Ok(entry.get_password()?)
}

pub fn set_password(
    instance_address: &str,
    username: &str,
    password: &str,
) -> Result<(), Box<dyn Error>> {
    let service = format!("trsync::{}", instance_address);
    let entry = keyring::Entry::new(&service, username);
    log::info!(
        "Store password for service '{}' and user '{}'",
        &service,
        &username
    );
    entry.set_password(password)?;
    Ok(())
}
