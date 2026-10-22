//! 任务查询、原子抢占、运行状态与心跳。
use super::*;
use entities::{task_runs, tasks};

fn task_from(row: tasks::Model) -> Result<Task> {
    Ok(Task {
        id: row.id,
        request: serde_json::from_str(&row.request_json)?,
        state: row.state,
        message: row.message,
        created_at: row.created_at.parse()?,
        updated_at: row.updated_at.parse()?,
    })
}

fn run_from(row: task_runs::Model) -> Result<RunRecord> {
    Ok(RunRecord {
        id: row.id,
        task_id: row.task_id,
        attempt: row.attempt,
        state: row.state,
        started_at: row.started_at.parse()?,
        finished_at: row.finished_at.map(|v| v.parse()).transpose()?,
        exit_code: row.exit_code,
        error_code: row.error_code,
        error_message: row.error_message,
        log_path: row.log_path,
        pid: row.pid.map(u32::try_from).transpose()?,
    })
}

impl Database {
    pub fn insert_task(&self, request: CreateTaskRequest) -> Result<Task> {
        let now = Utc::now();
        let task = Task {
            id: Uuid::new_v4().to_string(),
            request,
            state: "pending_registration".into(),
            message: None,
            created_at: now,
            updated_at: now,
        };
        tasks::Entity::insert(tasks::ActiveModel {
            id: Set(task.id.clone()),
            request_json: Set(serde_json::to_string(&task.request)?),
            state: Set(task.state.clone()),
            message: Set(None),
            created_at: Set(now.to_rfc3339()),
            updated_at: Set(now.to_rfc3339()),
        })
        .exec_without_returning(&self.conn)?;
        Ok(task)
    }

    pub fn tasks(&self) -> Result<Vec<Task>> {
        let mut tasks = tasks::Entity::find()
            .all(&self.conn)?
            .into_iter()
            .map(task_from)
            .collect::<Result<Vec<_>>>()?;
        tasks.sort_by_key(|t| t.request.scheduled_at_utc);
        Ok(tasks)
    }

    pub fn task(&self, id: &str) -> Result<Task> {
        task_from(
            tasks::Entity::find_by_id(id)
                .one(&self.conn)?
                .context("TASK_NOT_FOUND")?,
        )
    }

    pub fn transition(
        &self,
        id: &str,
        from: &[&str],
        to: &str,
        message: Option<&str>,
    ) -> Result<()> {
        let old = self.task(id)?;
        if !from.contains(&old.state.as_str()) {
            bail!("INVALID_STATE: {}", old.state);
        }
        let result = tasks::Entity::update_many()
            .col_expr(tasks::Column::State, Expr::value(to))
            .col_expr(tasks::Column::Message, Expr::value(message))
            .col_expr(
                tasks::Column::UpdatedAt,
                Expr::value(Utc::now().to_rfc3339()),
            )
            .filter(tasks::Column::Id.eq(id))
            .filter(tasks::Column::State.eq(old.state))
            .exec(&self.conn)?;
        if result.rows_affected != 1 {
            bail!("STATE_CONFLICT");
        }
        Ok(())
    }

    pub fn replace_request(&self, id: &str, request: &CreateTaskRequest) -> Result<()> {
        let result = tasks::Entity::update_many()
            .col_expr(
                tasks::Column::RequestJson,
                Expr::value(serde_json::to_string(request)?),
            )
            .col_expr(
                tasks::Column::UpdatedAt,
                Expr::value(Utc::now().to_rfc3339()),
            )
            .filter(tasks::Column::Id.eq(id))
            .filter(tasks::Column::State.eq("pending_registration"))
            .exec(&self.conn)?;
        if result.rows_affected != 1 {
            bail!("STATE_CONFLICT");
        }
        Ok(())
    }

    pub fn claim(&mut self, id: &str, manual: bool) -> Result<Option<RunRecord>> {
        self.write(|db| {
            let settings = db.settings()?;
            if settings.paused
                || tasks::Entity::find()
                    .filter(tasks::Column::State.is_in(["claimed", "running"]))
                    .count(&db.conn)?
                    > 0
            {
                return Ok(None);
            }
            let task = db.task(id)?;
            if task.state != "scheduled"
                || settings
                    .paused_providers
                    .contains(&task.request.conversation.provider)
                || (!manual && task.request.scheduled_at_utc > Utc::now())
            {
                return Ok(None);
            }
            if !manual {
                let state = missed_state(&task.request, Utc::now());
                if state != "scheduled" {
                    db.transition(id, &["scheduled"], state, None)?;
                    return Ok(None);
                }
            }
            db.transition(id, &["scheduled"], "claimed", None)?;
            let attempt = task_runs::Entity::find()
                .filter(task_runs::Column::TaskId.eq(id))
                .order_by_desc(task_runs::Column::Attempt)
                .one(&db.conn)?
                .map_or(1, |r| r.attempt + 1);
            let now = Utc::now();
            let run = RunRecord {
                id: Uuid::new_v4().to_string(),
                task_id: id.into(),
                attempt,
                state: "claimed".into(),
                started_at: now,
                finished_at: None,
                exit_code: None,
                error_code: None,
                error_message: None,
                log_path: None,
                pid: Some(std::process::id()),
            };
            task_runs::Entity::insert(task_runs::ActiveModel {
                id: Set(run.id.clone()),
                task_id: Set(id.into()),
                attempt: Set(attempt),
                state: Set("claimed".into()),
                started_at: Set(now.to_rfc3339()),
                finished_at: Set(None),
                exit_code: Set(None),
                error_code: Set(None),
                error_message: Set(None),
                log_path: Set(None),
                pid: Set(run.pid.map(i64::from)),
                heartbeat_at: Set(now.to_rfc3339()),
            })
            .exec_without_returning(&db.conn)?;
            Ok(Some(run))
        })
    }

    pub fn start_run(&self, run: &str, task: &str, log: &str, pid: u32) -> Result<()> {
        self.write(|db| {
            task_runs::Entity::update_many()
                .col_expr(task_runs::Column::State, Expr::value("running"))
                .col_expr(task_runs::Column::LogPath, Expr::value(log))
                .col_expr(task_runs::Column::Pid, Expr::value(i64::from(pid)))
                .filter(task_runs::Column::Id.eq(run))
                .exec(&db.conn)?;
            db.transition(task, &["claimed"], "running", None)
        })
    }

    pub fn heartbeat(&self, run: &str) -> Result<bool> {
        task_runs::Entity::update_many()
            .col_expr(
                task_runs::Column::HeartbeatAt,
                Expr::value(Utc::now().to_rfc3339()),
            )
            .filter(task_runs::Column::Id.eq(run))
            .exec(&self.conn)?;
        let run = task_runs::Entity::find_by_id(run)
            .one(&self.conn)?
            .context("RUN_NOT_FOUND")?;
        Ok(self.task(&run.task_id)?.state == "cancelling")
    }

    pub fn finish(
        &mut self,
        run: &str,
        state: &str,
        exit: Option<i32>,
        code: Option<&str>,
        message: Option<&str>,
    ) -> Result<()> {
        self.write(|db| {
            let current = task_runs::Entity::find_by_id(run)
                .one(&db.conn)?
                .context("RUN_NOT_FOUND")?;
            if !["claimed", "running"].contains(&current.state.as_str()) {
                return Ok(());
            }
            let now = Utc::now().to_rfc3339();
            task_runs::Entity::update_many()
                .col_expr(task_runs::Column::State, Expr::value(state))
                .col_expr(task_runs::Column::FinishedAt, Expr::value(now.clone()))
                .col_expr(task_runs::Column::ExitCode, Expr::value(exit))
                .col_expr(task_runs::Column::ErrorCode, Expr::value(code))
                .col_expr(task_runs::Column::ErrorMessage, Expr::value(message))
                .filter(task_runs::Column::Id.eq(run))
                .exec(&db.conn)?;
            tasks::Entity::update_many()
                .col_expr(tasks::Column::State, Expr::value(state))
                .col_expr(tasks::Column::Message, Expr::value(message))
                .col_expr(tasks::Column::UpdatedAt, Expr::value(now))
                .filter(tasks::Column::Id.eq(current.task_id))
                .exec(&db.conn)?;
            Ok(())
        })
    }

    pub fn recover_stale(&self) -> Result<usize> {
        for mut run in self.pretrigger_runs()? {
            if run.state == "running" && run.attempted_at < Utc::now() - Duration::minutes(5) {
                run.state = "uncertain".into();
                run.message = "上次执行中断，结果未知；本轮不重复发送".into();
                run.finished_at = Some(Utc::now());
                self.finish_pretrigger(&run)?;
            }
        }
        self.write(|db| {
            let cutoff = (Utc::now() - Duration::minutes(3)).to_rfc3339();
            let ids = task_runs::Entity::find()
                .select_only()
                .column(task_runs::Column::TaskId)
                .filter(task_runs::Column::FinishedAt.is_null())
                .filter(task_runs::Column::HeartbeatAt.lt(cutoff.clone()))
                .into_tuple::<String>()
                .all(&db.conn)?;
            if ids.is_empty() {
                return Ok(0);
            }
            let changed = tasks::Entity::update_many()
                .col_expr(tasks::Column::State, Expr::value("needs_review"))
                .col_expr(
                    tasks::Column::Message,
                    Expr::value("执行器心跳失联，请审查后手动重试"),
                )
                .filter(tasks::Column::State.is_in(["claimed", "running", "cancelling"]))
                .filter(tasks::Column::Id.is_in(ids))
                .exec(&db.conn)?;
            task_runs::Entity::update_many()
                .col_expr(task_runs::Column::State, Expr::value("needs_review"))
                .col_expr(
                    task_runs::Column::FinishedAt,
                    Expr::value(Utc::now().to_rfc3339()),
                )
                .col_expr(task_runs::Column::ErrorCode, Expr::value("RUNNER_LOST"))
                .filter(task_runs::Column::FinishedAt.is_null())
                .filter(task_runs::Column::HeartbeatAt.lt(cutoff))
                .exec(&db.conn)?;
            Ok(changed.rows_affected as usize)
        })
    }

    pub fn runs(&self) -> Result<Vec<RunRecord>> {
        task_runs::Entity::find()
            .order_by_desc(task_runs::Column::StartedAt)
            .limit(500)
            .all(&self.conn)?
            .into_iter()
            .map(run_from)
            .collect()
    }

    pub fn prepare_next_occurrence(&self, id: &str) -> Result<Option<Task>> {
        let task = self.task(id)?;
        if !matches!(task.state.as_str(), "succeeded" | "expired") {
            return Ok(None);
        }
        let Some(next) = next_occurrence(&task.request, Utc::now())? else {
            return Ok(None);
        };
        let result = tasks::Entity::update_many()
            .col_expr(
                tasks::Column::RequestJson,
                Expr::value(serde_json::to_string(&next)?),
            )
            .col_expr(tasks::Column::State, Expr::value("pending_registration"))
            .col_expr(
                tasks::Column::UpdatedAt,
                Expr::value(Utc::now().to_rfc3339()),
            )
            .filter(tasks::Column::Id.eq(id))
            .filter(tasks::Column::State.eq(task.state))
            .exec(&self.conn)?;
        if result.rows_affected != 1 {
            return Ok(None);
        }
        Ok(Some(self.task(id)?))
    }
}
