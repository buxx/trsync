extern crate keyring;

use std::error::Error;

pub fn get_password(address: &str, username: &str) -> Result<String, Box<dyn Error>> {
    let service = format!("trsync::{}", address);
    let entry = keyring::Entry::new(&service, username);
    Ok(entry.get_password()?)
}
