#[test]
#[ignore = "Requires Windows and an embedded-frontend build: cargo build -p codebeat --features tauri/custom-protocol"]
fn native_notification_process_reports_ready_without_blocking_for_dismissal() {
    use std::{
        io::{BufRead, Write},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("codebeat.exe");
    assert!(
        executable.is_file(),
        "Build the embedded-frontend desktop executable first"
    );
    let isolated_data = tempfile::tempdir().unwrap();
    let directory = isolated_data.path().join("codebeat/notification-acks");
    std::fs::create_dir_all(&directory).unwrap();
    let id = uuid::Uuid::new_v4();
    let ack = directory.join(format!("{id}.read"));
    let dismiss = directory.join(format!("{id}.dismiss"));
    let mut command = Command::new(executable);
    command
        .arg("--notification-helper")
        .env("LOCALAPPDATA", isolated_data.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    struct Cleanup {
        child: std::process::Child,
        ack: std::path::PathBuf,
        dismiss: std::path::PathBuf,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_file(&self.ack);
            let _ = std::fs::remove_file(&self.dismiss);
        }
    }
    let mut helper = Cleanup {
        child: command.spawn().unwrap(),
        ack,
        dismiss,
    };
    let start = std::time::Instant::now();
    helper.child.stdin.take().unwrap().write_all(&serde_json::to_vec(&serde_json::json!({
        "title":"CodeBeat 通知测试","body":"此为本地界面测试，没有发送 AI 请求。","kind":"success",
        "task_id":null,"page":"tasks","sound":false,"ack_path":helper.ack,"dismiss_path":helper.dismiss
    })).unwrap()).unwrap();
    let stdout = helper.child.stdout.take().unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        std::io::BufReader::new(stdout)
            .read_line(&mut line)
            .unwrap();
        let _ = send.send(line);
    });
    assert_eq!(
        receive.recv_timeout(Duration::from_secs(8)).unwrap().trim(),
        "READY"
    );
    assert!(start.elapsed() < Duration::from_secs(8));
    assert!(helper.child.try_wait().unwrap().is_none());
    std::fs::write(&helper.dismiss, b"").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = helper.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Helper failed to respond to external dismissal"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
