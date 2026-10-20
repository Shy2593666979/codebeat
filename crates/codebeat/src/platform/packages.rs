//! 查询当前用户注册的 MSIX 应用包，按版本从新到旧返回安装目录。
use anyhow::Result;
use std::path::PathBuf;

/// 精确匹配包名，返回当前用户的安装目录；版本比较使用数值字段。
pub(crate) fn package_roots(names: &[&str]) -> Result<Vec<PathBuf>> {
    use windows::Management::Deployment::PackageManager;
    // windows crate 的激活工厂负责 PackageManager 所需的 WinRT 初始化。
    let packages =
        PackageManager::new()?.FindPackagesByUserSecurityId(&windows::core::HSTRING::new())?;
    let mut roots = Vec::new();
    for package in packages {
        let identity = package.Id()?;
        if names
            .iter()
            .any(|name| identity.Name().is_ok_and(|n| n == *name))
        {
            let version = identity.Version()?;
            roots.push((
                (
                    version.Major,
                    version.Minor,
                    version.Build,
                    version.Revision,
                ),
                PathBuf::from(package.InstalledLocation()?.Path()?.to_os_string()),
            ));
        }
    }
    roots.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    Ok(roots.into_iter().map(|(_, root)| root).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_user_package_inventory_works_without_a_shell() {
        assert!(package_roots(&["CodeBeat.Nonexistent.Package.For.Tests"])
            .unwrap()
            .is_empty());
        for root in package_roots(&["OpenAI.Codex", "OpenAI.ChatGPT"]).unwrap() {
            assert!(root.is_absolute());
            assert!(root.is_dir());
        }
    }
}
