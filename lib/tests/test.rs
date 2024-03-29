use std::process::Command;

#[test]
fn check_checksum() {
    let entries = std::path::Path::new("../table")
        .read_dir()
        .expect("read_dir call failed");

    let checksum_bytes = std::fs::read("../checksum.txt").expect("no checksum file found");
    let checksum = String::from_utf8_lossy(&checksum_bytes);

    for entry_res in entries {
        let table_name = entry_res
            .expect("dir not readable")
            .file_name()
            .into_string()
            .expect("filename conversion failed");

        let cmd_output = Command::new("md5sum")
            .arg(format!("../table/{table_name}"))
            .output()
            .expect("failed to execute md5");
        let checksum_line = String::from_utf8_lossy(&cmd_output.stdout).to_string();
        let one_checksum = checksum_line.split_once(' ').unwrap().0;
        assert!(
            checksum.contains(one_checksum),
            "{table_name} checksum changed"
        )
    }
}
