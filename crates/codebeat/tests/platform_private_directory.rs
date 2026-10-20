#[test]
fn restricts_owned_directory_without_security_privilege() {
    let dir = tempfile::tempdir().unwrap();
    codebeat_lib::platform::restrict_data_directory(dir.path()).unwrap();
    std::fs::write(dir.path().join("write-test"), b"private").unwrap();
    codebeat_lib::platform::restrict_data_directory(dir.path()).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("write-test")).unwrap(),
        b"private"
    );
}
