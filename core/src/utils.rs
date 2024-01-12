use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

pub fn strbool(value: &str) -> bool {
    ["true", "True", "t", "T", "1"].contains(&value)
}

// TODO : Result instead unwrap
// From https://stackoverflow.com/a/75443325/801924
pub fn md5_file(file_path: &PathBuf) -> String {
    let f = File::open(file_path).unwrap();
    // Find the length of the file
    let len = f.metadata().unwrap().len();
    // Decide on a reasonable buffer size (1MB in this case, fastest will depend on hardware)
    let buf_len = len.min(1_000_000) as usize;
    let mut buf = BufReader::with_capacity(buf_len, f);
    let mut context = md5::Context::new();
    loop {
        // Get a chunk of the file
        let part = buf.fill_buf().unwrap();
        // If that chunk was empty, the reader has reached EOF
        if part.is_empty() {
            break;
        }
        // Add chunk to the md5
        context.consume(part);
        // Tell the buffer that the chunk is consumed
        let part_len = part.len();
        buf.consume(part_len);
    }
    format!("{:x}", context.compute())
}
