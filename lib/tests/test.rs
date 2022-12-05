#[test]
fn check_checksum() {
    let entries = std::path::Path::new("../table")
        .read_dir()
        .expect("read_dir call failed");

    let checksum_bytes = std::fs::read("../checksum.txt")
        .or_else(|_| std::fs::read("./checksum.txt"))
        .or_else(|_| std::fs::read("../../checksum.txt"))
        .expect("no checksum file found");
    let checksum = String::from_utf8_lossy(&checksum_bytes);

    for entry_res in entries {
        let table_name = entry_res
            .expect("dir not readable")
            .file_name()
            .into_string()
            .expect("filename conversion failed");
        let cmd_output = std::process::Command::new("md5")
            .arg(format!("table/{table_name}"))
            .output()
            .expect("failed to execute process");
        let one_checksum = String::from_utf8_lossy(&cmd_output.stdout).to_string();
        assert!(checksum.contains(&one_checksum))
    }
}
