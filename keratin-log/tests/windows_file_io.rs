#![cfg(windows)]
#[path = "../src/windows_file.rs"]
mod windows_file;

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use windows_file::FileExt;

#[test]
fn positional_io_preserves_writer_cursor_and_link_identity() {
    let dir = std::env::temp_dir().join(format!("keratin-win-io-{}", fastrand::u64(..)));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("segment");
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    file.write_all(b"abcdef").unwrap();
    let background = file.try_clone().unwrap();
    background.write_all_at(b"zeros", 20).unwrap();
    assert_eq!(file.stream_position().unwrap(), 6);
    file.write_all(b"GHI").unwrap();
    let mut bytes = [0; 9];
    background.read_exact_at(&mut bytes, 0).unwrap();
    assert_eq!(&bytes, b"abcdefGHI");
    assert_eq!(file.stream_position().unwrap(), 9);
    assert_eq!(background.read_at(&mut bytes, 100).unwrap(), 0);
    assert!(background.read_exact_at(&mut bytes, 100).is_err());
    assert_eq!(windows_file::number_of_links(&file).unwrap(), 1);
    let link = dir.join("fork");
    fs::hard_link(&path, &link).unwrap();
    assert_eq!(windows_file::number_of_links(&file).unwrap(), 2);
    fs::rename(&path, dir.join("renamed")).unwrap();
    background.write_all_at(b"J", 9).unwrap();
    file.seek(SeekFrom::Start(9)).unwrap();
    let mut last = [0];
    background.read_exact_at(&mut last, 9).unwrap();
    assert_eq!(&last, b"J");
    file.sync_all().unwrap();
    drop(background);
    drop(file);
    fs::remove_file(dir.join("renamed")).unwrap();
    fs::remove_file(link).unwrap();
    fs::remove_dir(dir).unwrap();
}
