//! 当前用户的 Windows 任务调度；COM 对象及其初始化环境均在调用线程内管理。
use super::wide;
use anyhow::{Context, Result};
use std::path::Path;
use windows::{
    core::{Interface, BSTR},
    Win32::{
        Foundation::*,
        System::{Com::*, TaskScheduler::*, Variant::VARIANT},
    },
};

// 每次成功初始化（包括 S_FALSE）都必须对应一次 CoUninitialize。
// 已有 STA 也可直接使用，但其初始化由原调用方负责释放。
struct Apartment(bool);
impl Apartment {
    fn enter() -> Result<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            return Ok(Self(false));
        }
        hr.ok()?;
        Ok(Self(true))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

struct Scheduler {
    folder: ITaskFolder,
    service: ITaskService,
    // 字段按声明顺序释放，必须先释放 COM 接口，再结束初始化环境。
    _apartment: Apartment,
}
impl Scheduler {
    fn connect() -> Result<Self> {
        let apartment = Apartment::enter()?;
        unsafe {
            let service: ITaskService =
                CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
            let empty = VARIANT::default();
            service.Connect(&empty, &empty, &empty, &empty)?;
            let folder = service.GetFolder(&BSTR::from("\\"))?;
            Ok(Self {
                folder,
                service,
                _apartment: apartment,
            })
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ScheduleKind {
    /// 用户创建的一次性任务。
    Once,
    /// 每五分钟执行维护，并在用户登录时补充触发。
    Maintenance,
    /// 每分钟检查预触发条件，本身不唤醒机器。
    Pretrigger,
    /// 优化器计算出的下一次单独唤醒。
    OptimizerWake,
}

/// 创建或更新系统任务；命令参数仅保存任务标识，不携带提示词或凭据。
pub(super) fn schedule(
    id: &str,
    runner: &Path,
    kind: ScheduleKind,
    time: Option<&str>,
    wake: bool,
    catchup: bool,
) -> Result<()> {
    let scheduler = Scheduler::connect().context("Connect to Windows Task Scheduler")?;
    unsafe {
        let task = scheduler.service.NewTask(0)?;
        task.RegistrationInfo()?.SetDescription(&BSTR::from(
            "codebeat local task; no prompts or credentials in arguments",
        ))?;
        // 使用调度服务连接的真实用户身份，避免依赖环境变量中的用户名。
        let identity = BSTR::from(format!(
            "{}\\{}",
            scheduler.service.ConnectedDomain()?,
            scheduler.service.ConnectedUser()?
        ));
        let principal = task.Principal()?;
        principal.SetUserId(&identity)?;
        principal.SetLogonType(TASK_LOGON_INTERACTIVE_TOKEN)?;
        principal.SetRunLevel(TASK_RUNLEVEL_LUA)?;
        let settings = task.Settings()?;
        settings.SetEnabled(VARIANT_TRUE)?;
        settings.SetWakeToRun(VARIANT_BOOL::from(wake))?;
        settings.SetStartWhenAvailable(VARIANT_BOOL::from(catchup))?;
        settings.SetMultipleInstances(TASK_INSTANCES_IGNORE_NEW)?;
        settings.SetExecutionTimeLimit(&BSTR::from("PT40M"))?;
        settings.SetDisallowStartIfOnBatteries(VARIANT_FALSE)?;
        settings.SetStopIfGoingOnBatteries(VARIANT_FALSE)?;
        let triggers = task.Triggers()?;
        if matches!(kind, ScheduleKind::Maintenance | ScheduleKind::Pretrigger) {
            let trigger = triggers.Create(TASK_TRIGGER_DAILY)?;
            trigger.SetStartBoundary(&BSTR::from(
                chrono::Local::now().format("%Y-%m-%dT00:00:00").to_string(),
            ))?;
            trigger.cast::<IDailyTrigger>()?.SetDaysInterval(1)?;
            let repeat = trigger.Repetition()?;
            repeat.SetInterval(&BSTR::from(if matches!(kind, ScheduleKind::Pretrigger) {
                "PT1M"
            } else {
                "PT5M"
            }))?;
            repeat.SetDuration(&BSTR::from("P1D"))?;
            triggers
                .Create(TASK_TRIGGER_LOGON)?
                .cast::<ILogonTrigger>()?
                .SetUserId(&identity)?;
        } else {
            let trigger = triggers.Create(TASK_TRIGGER_TIME)?;
            trigger.SetStartBoundary(&BSTR::from(time.context("Missing one-shot task time")?))?;
            trigger.SetEnabled(VARIANT_TRUE)?;
        }
        let action = task
            .Actions()?
            .Create(TASK_ACTION_EXEC)?
            .cast::<IExecAction>()?;
        let runner_path = wide(runner.as_os_str());
        action.SetPath(&BSTR::from_wide(&runner_path[..runner_path.len() - 1]))?;
        let parent = runner.parent().context("Runner has no parent directory")?;
        let parent = wide(parent.as_os_str());
        action.SetWorkingDirectory(&BSTR::from_wide(&parent[..parent.len() - 1]))?;
        let arguments = match kind {
            ScheduleKind::Maintenance => "maintenance".into(),
            ScheduleKind::Pretrigger | ScheduleKind::OptimizerWake => "pretrigger".into(),
            ScheduleKind::Once => format!("run --task-id {id}"),
        };
        action.SetArguments(&BSTR::from(arguments))?;
        scheduler.folder.RegisterTaskDefinition(
            &BSTR::from(format!("codebeat-{id}")),
            &task,
            TASK_CREATE_OR_UPDATE.0,
            &VARIANT::from(identity),
            &VARIANT::default(),
            TASK_LOGON_INTERACTIVE_TOKEN,
            &VARIANT::default(),
        )?;
    }
    Ok(())
}

/// 删除指定任务；不存在视为已完成，其他系统错误向上传递。
pub(super) fn delete_task(id: &str) -> Result<()> {
    let scheduler = Scheduler::connect()?;
    unsafe {
        if let Err(error) = scheduler
            .folder
            .DeleteTask(&BSTR::from(format!("codebeat-{id}")), 0)
        {
            if error.code() != windows::core::HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) {
                return Err(error.into());
            }
        }
    }
    Ok(())
}

pub(super) fn task_diagnostics() -> Result<serde_json::Value> {
    let scheduler = Scheduler::connect()?;
    unsafe {
        let tasks = scheduler.folder.GetTasks(0)?;
        let mut result = Vec::new();
        for index in 1..=tasks.Count()? {
            let task = tasks.get_Item(&VARIANT::from(index))?;
            let name = task.Name()?.to_string();
            if name.starts_with("codebeat-") {
                // Automation DATE 以 1899-12-30 为起点，表示本地日期和时间。
                let date = task.NextRunTime()?;
                let base = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                let next = base
                    .checked_add_signed(chrono::Duration::milliseconds(
                        (date * 86400000.).round() as i64
                    ))
                    .context("Invalid task next-run date")?;
                result.push(serde_json::json!({"name":name,"lastResult":task.LastTaskResult()?,"nextRun":format!("{}0", next.format("%Y-%m-%dT%H:%M:%S%.6f")),"enabled":task.Enabled()?.as_bool()}));
            }
        }
        Ok(serde_json::json!(result))
    }
}

pub(super) fn cleanup_tasks() -> Result<()> {
    // 沿用已有安装的任务名前缀，确保卸载时能清理历史注册。
    let tasks = task_diagnostics()?;
    for task in tasks.as_array().context("Invalid task inventory")? {
        if let Some(id) = task["name"]
            .as_str()
            .and_then(|name| name.strip_prefix("codebeat-"))
        {
            delete_task(id)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Creates isolated current-user scheduler entries; never executes a model"]
    fn scheduler_preserves_all_trigger_and_execution_policies() {
        let directory = tempfile::tempdir().unwrap();
        let runner = directory.path().join("测试 runner.exe");
        // 故意使用不可执行的文件，即使调度发生竞争也不会运行任何代码。
        std::fs::write(&runner, b"fixture").unwrap();
        for kind in [
            ScheduleKind::Once,
            ScheduleKind::Maintenance,
            ScheduleKind::Pretrigger,
            ScheduleKind::OptimizerWake,
        ] {
            let id = uuid::Uuid::new_v4().to_string();
            struct Cleanup(String);
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    let _ = delete_task(&self.0);
                }
            }
            let _cleanup = Cleanup(id.clone());
            let time = (chrono::Utc::now() + chrono::Duration::days(2)).to_rfc3339();
            let wake = matches!(kind, ScheduleKind::OptimizerWake);
            schedule(&id, &runner, kind, Some(&time), wake, true).unwrap();
            let scheduler = Scheduler::connect().unwrap();
            let xml = unsafe {
                let mut xml = BSTR::new();
                scheduler
                    .folder
                    .GetTask(&BSTR::from(format!("codebeat-{id}")))
                    .unwrap()
                    .Definition()
                    .unwrap()
                    .XmlText(&mut xml)
                    .unwrap();
                xml.to_string()
            };
            for expected in [
                "<LogonType>InteractiveToken</LogonType>",
                "<RunLevel>LeastPrivilege</RunLevel>",
                "<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>",
                "<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>",
                "<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>",
                "<StartWhenAvailable>true</StartWhenAvailable>",
                "<ExecutionTimeLimit>PT40M</ExecutionTimeLimit>",
            ] {
                assert!(xml.contains(expected), "Missing {expected}: {xml}");
            }
            assert!(xml.contains(if wake {
                "<WakeToRun>true</WakeToRun>"
            } else {
                "<WakeToRun>false</WakeToRun>"
            }));
            match kind {
                ScheduleKind::Maintenance | ScheduleKind::Pretrigger => {
                    assert!(xml.contains("<LogonTrigger>"));
                    assert!(xml.contains("<DaysInterval>1</DaysInterval>"));
                    assert!(xml.contains("<Duration>P1D</Duration>"));
                    assert!(xml.contains(if matches!(kind, ScheduleKind::Maintenance) {
                        "<Interval>PT5M</Interval>"
                    } else {
                        "<Interval>PT1M</Interval>"
                    }));
                }
                _ => {
                    assert!(xml.contains("<TimeTrigger>"));
                    assert!(!xml.contains("<Repetition>"));
                    let boundary = xml
                        .split("<StartBoundary>")
                        .nth(1)
                        .unwrap()
                        .split("</StartBoundary>")
                        .next()
                        .unwrap();
                    assert_eq!(
                        chrono::DateTime::parse_from_rfc3339(boundary)
                            .unwrap()
                            .timestamp(),
                        chrono::DateTime::parse_from_rfc3339(&time)
                            .unwrap()
                            .timestamp()
                    );
                }
            }
            assert!(xml.contains(&runner.display().to_string()));
            assert!(xml.contains(match kind {
                ScheduleKind::Maintenance => "<Arguments>maintenance</Arguments>",
                ScheduleKind::Once => "run --task-id",
                _ => "<Arguments>pretrigger</Arguments>",
            }));
            delete_task(&id).unwrap();
            delete_task(&id).unwrap();
        }
    }
}
