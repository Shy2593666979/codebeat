//! 查找桌面应用附带的原生引擎，整个发现过程不启动应用界面。
use std::path::{Path, PathBuf};

pub fn native_codex_path(path: PathBuf) -> Option<PathBuf> {
    let parent = path.parent()?;
    if parent
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("WindowsApps"))
    {
        // 商店执行别名可能打开界面，改为查询应用包中的实际引擎路径。
        return None;
    }
    let embedded = parent.join("resources/codex.exe");
    Some(if embedded.is_file() { embedded } else { path })
}

pub fn desktop_candidates(runtime_root: &Path, package_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut runtimes: Vec<_> = std::fs::read_dir(runtime_root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path().join("codex.exe");
            path.is_file()
                .then(|| (entry.metadata().and_then(|m| m.modified()).ok(), path))
        })
        .collect();
    // 运行时目录的哈希不代表版本，优先选择最近安装的可用运行时。
    runtimes.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut paths: Vec<_> = runtimes.into_iter().map(|(_, p)| p).collect();
    for root in package_roots {
        // app/Codex.exe 是界面启动器，真正的 app-server 引擎位于 resources 下。
        let engine = root.join("app/resources/codex.exe");
        if engine.is_file() {
            paths.push(engine);
        }
    }
    paths
}

pub fn desktop_executable() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    let runtime = PathBuf::from(local).join("OpenAI/Codex/bin");
    if let Some(path) = desktop_candidates(&runtime, &[]).into_iter().next() {
        return Some(path);
    }
    #[cfg(windows)]
    {
        let roots =
            crate::platform::packages::package_roots(&["OpenAI.Codex", "OpenAI.ChatGPT"]).ok()?;
        desktop_candidates(&runtime, &roots).into_iter().next()
    }
    #[cfg(not(windows))]
    None
}

/// 仅检查安装状态，不把 ChatGPT 应用包当作可执行引擎。
pub async fn chatgpt_installed() -> Option<bool> {
    #[cfg(windows)]
    {
        tokio::time::timeout(
            std::time::Duration::from_secs(15),
            tokio::task::spawn_blocking(|| {
                crate::platform::packages::package_roots(&["OpenAI.ChatGPT"])
                    .map(|roots| !roots.is_empty())
                    .ok()
            }),
        )
        .await
        .ok()
        .and_then(Result::ok)
        .flatten()
    }
    #[cfg(not(windows))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_discovery_does_not_launch_the_desktop_ui_or_store_alias() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        std::fs::create_dir_all(app.join("resources")).unwrap();
        let native = app.join("resources/codex.exe");
        std::fs::write(&native, b"fixture").unwrap();
        assert_eq!(
            native_codex_path(app.join("Codex.exe")),
            Some(native.clone())
        );
        assert_eq!(native_codex_path(native.clone()), Some(native));
        assert!(native_codex_path(dir.path().join("Microsoft/WindowsApps/codex.exe")).is_none());
    }
    #[test]
    fn finds_desktop_only_runtime_and_ignores_ui_and_incomplete_updates() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("bin");
        std::fs::create_dir_all(runtime.join("ready")).unwrap();
        std::fs::create_dir_all(runtime.join("incomplete")).unwrap();
        let engine = runtime.join("ready/codex.exe");
        std::fs::write(&engine, b"fixture").unwrap();
        assert_eq!(desktop_candidates(&runtime, &[]), vec![engine]);
        let package = dir.path().join("package");
        std::fs::create_dir_all(package.join("app/resources")).unwrap();
        std::fs::write(package.join("app/Codex.exe"), b"UI").unwrap();
        assert!(
            desktop_candidates(&dir.path().join("missing"), std::slice::from_ref(&package))
                .is_empty()
        );
        let bundled = package.join("app/resources/codex.exe");
        std::fs::write(&bundled, b"engine").unwrap();
        assert_eq!(
            desktop_candidates(&dir.path().join("missing"), &[package]),
            vec![bundled]
        );
    }
    #[test]
    fn rescans_after_runtime_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        for hash in ["old", "new"] {
            std::fs::create_dir(dir.path().join(hash)).unwrap();
        }
        let old = dir.path().join("old/codex.exe");
        let new = dir.path().join("new/codex.exe");
        std::fs::write(&old, b"fixture").unwrap();
        assert_eq!(desktop_candidates(dir.path(), &[]), vec![old.clone()]);
        std::fs::remove_file(old).unwrap();
        std::fs::write(&new, b"fixture").unwrap();
        assert_eq!(desktop_candidates(dir.path(), &[]), vec![new]);
    }
}
