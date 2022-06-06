#[cfg(target_os = "windows")]
use windres::Build;

fn main() {
    #[cfg(target_os = "windows")]
    {
        Build::new().compile("trsync.rc").unwrap();
    }
}
