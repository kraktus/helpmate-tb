use std::{path::Path, process::Command};

fn main() {
    let entries = Path::new("./table")
        .read_dir()
        .expect("read_dir call failed");
    let mut md5_buf: Vec<u8> = Vec::new();
    for entry_res in entries {
        let table_name = entry_res.unwrap().file_name().into_string().unwrap();
        let cmd_output = Command::new("md5")
            .arg(format!("table/{table_name}"))
            .output()
            .expect("failed to execute process");
        md5_buf.extend_from_slice(&cmd_output.stdout);
    }
    std::fs::write("checksum.txt", md5_buf).expect("writing failed");
}
