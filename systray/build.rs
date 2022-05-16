use windres::Build;

fn main() {
    Build::new().compile("tray.rc").unwrap();
}
