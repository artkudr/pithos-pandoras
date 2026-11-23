use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::domain::{AppError, DraftRevision, ReportDraft, ReportDraftDocument, ReportPublished};
use crate::store::{self, StoreError};
use store::{db, drafts, events, reports, stages};

use super::{
    ensure_role, require_field, require_project, require_task, validate_kind, ReportAppendRequest,
    ReportDiscardRequest, ReportOpenRequest, ReportPublishRequest, ReportSectionRequest, Service,
    STAGE_KINDS,
};

/// Сколько живёт неопубликованный черновик.
const DRAFT_TTL_SECONDS: i64 = 24 * 60 * 60;
const MIN_PUBLISHED_CHARS: usize = 20;

impl Service {
    pub async fn report_open(&self, req: ReportOpenRequest) -> Result<ReportDraft, AppError> {
        let project_name = require_project(req.project.as_deref())?.to_string();
        let task_ref = require_task(req.task.as_deref())?.to_string();
        let kind = req.kind.unwrap_or_else(|| String::from("report"));
        validate_kind(&kind, STAGE_KINDS)?;
        let title = req
            .title
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let view = self.project_view(&project_name).await?;
        let project = view.project.clone();
        let task = self.task(&project, &task_ref).await?;
        let task_id = task.id.clone();

        let open = self
            .store
            .read({
                let task_id = task_id.clone();
                move |conn| stages::open_stage(conn, &task_id)
            })
            .await?;
        let Some(stage) = open else {
            return Err(AppError::conflict("нет открытого этапа"));
        };

        let role = match req.role.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            Some(role) => role.to_string(),
            None => stage.role.clone(),
        };
        ensure_role(&view.roles, &role)?;
        if role != stage.role {
            return Err(AppError::conflict(format!(
                "этап открыт ролью `{}`",
                stage.role
            )));
        }

        let title = title.unwrap_or_else(|| format!("Отчёт: {role}"));
        let now = db::now_rfc3339();
        let stage_id = stage.id.clone();
        let task_id_for_write = task_id.clone();
        let role_for_write = role.clone();
        let kind_for_write = kind.clone();
        let now_for_write = now.clone();

        let draft = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                if let Some(existing) = drafts::find_open_draft(
                    &tx,
                    &task_id_for_write,
                    &stage_id,
                    &role_for_write,
                    &kind_for_write,
                )? {
                    if !is_expired(&existing.created_at, &now_for_write) {
                        return Ok(existing);
                    }
                    drafts::delete_draft(&tx, &existing.draft_id)?;
                }
                if reports::report_for_stage_kind(&tx, &stage_id, &kind_for_write)?.is_some() {
                    return Err(StoreError::Conflict(
                        "в этом этапе уже есть отчёт такого вида".to_string(),
                    ));
                }
                let draft = drafts::insert_draft(
                    &tx,
                    &task_id_for_write,
                    &stage_id,
                    &role_for_write,
                    &kind_for_write,
                    &title,
                    &now_for_write,
                )?;
                let payload = serde_json::json!({
                    "draft_id": &draft.draft_id,
                    "kind": &kind_for_write,
                    "title": &title,
                })
                .to_string();
                events::insert_event(
                    &tx,
                    &task_id_for_write,
                    "report_draft_opened",
                    Some(&stage_id),
                    Some(&role_for_write),
                    &payload,
                    "agent",
                )?;
                tx.commit()?;
                Ok(draft)
            })
            .await?;

        Ok(draft)
    }

    pub async fn report_section(
        &self,
        req: ReportSectionRequest,
    ) -> Result<DraftRevision, AppError> {
        let draft_id = require_field(req.draft_id.as_deref(), "draft_id")?.to_string();
        let key = require_field(req.key.as_deref(), "key")?.to_string();
        if !is_valid_key(&key) {
            return Err(AppError::field_invalid(
                "key",
                "Ключ раздела: буквы, цифры, `_` и `-`, без пробелов.",
            ));
        }
        let body = require_field(req.body.as_deref(), "body")?.to_string();
        let title = normalize_section_title(&key, req.title.as_deref());
        let expected = req.expected_revision;

        let draft = self.find_draft(&draft_id).await?;
        self.check_draft_writable(&draft, req.role.as_deref())
            .await?;

        let now = db::now_rfc3339();
        let order = req.order;
        let draft_id_for_write = draft_id.clone();
        let revision = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let current = drafts::get_draft(&tx, &draft_id_for_write)?;
                if current.published_report_id.is_some() {
                    return Err(StoreError::Conflict("черновик уже опубликован".to_string()));
                }
                check_revision(&current, expected)?;
                ensure_stage_open(&tx, &current)?;
                let sort = match order {
                    Some(value) => value,
                    None => match drafts::block_sort(&tx, &draft_id_for_write, &key)? {
                        Some(value) => value,
                        None => drafts::next_sort(&tx, &draft_id_for_write)?,
                    },
                };
                drafts::upsert_block(&tx, &draft_id_for_write, &key, &title, &body, sort)?;
                let revision = drafts::bump_revision(&tx, &draft_id_for_write, &now)?;
                stages::touch_stage(&tx, &current.stage_id, &now)?;
                tx.commit()?;
                Ok(revision)
            })
            .await?;

        Ok(DraftRevision { draft_id, revision })
    }

    pub async fn report_append(&self, req: ReportAppendRequest) -> Result<DraftRevision, AppError> {
        let draft_id = require_field(req.draft_id.as_deref(), "draft_id")?.to_string();
        let text = require_field(req.text.as_deref(), "text")?.to_string();
        let expected = req.expected_revision;

        let draft = self.find_draft(&draft_id).await?;
        self.check_draft_writable(&draft, req.role.as_deref())
            .await?;

        let now = db::now_rfc3339();
        let draft_id_for_write = draft_id.clone();
        let revision = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let current = drafts::get_draft(&tx, &draft_id_for_write)?;
                if current.published_report_id.is_some() {
                    return Err(StoreError::Conflict("черновик уже опубликован".to_string()));
                }
                check_revision(&current, expected)?;
                ensure_stage_open(&tx, &current)?;
                let sort = drafts::next_sort(&tx, &draft_id_for_write)?;
                let key = format!("append-{sort}");
                drafts::upsert_block(&tx, &draft_id_for_write, &key, "", &text, sort)?;
                let revision = drafts::bump_revision(&tx, &draft_id_for_write, &now)?;
                stages::touch_stage(&tx, &current.stage_id, &now)?;
                tx.commit()?;
                Ok(revision)
            })
            .await?;

        Ok(DraftRevision { draft_id, revision })
    }

    pub async fn report_draft(
        &self,
        draft_id: Option<String>,
    ) -> Result<ReportDraftDocument, AppError> {
        let draft_id = require_field(draft_id.as_deref(), "draft_id")?.to_string();
        let draft = self.find_draft(&draft_id).await?;
        if is_expired(&draft.created_at, &db::now_rfc3339()) {
            return Err(AppError::conflict("черновик устарел, откройте новый"));
        }
        let blocks = self
            .store
            .read({
                let draft_id = draft_id.clone();
                move |conn| drafts::list_blocks(conn, &draft_id)
            })
            .await?;
        let rendered = render_draft(&blocks);
        Ok(ReportDraftDocument {
            draft,
            blocks,
            rendered,
        })
    }

    pub async fn report_publish(
        &self,
        req: ReportPublishRequest,
    ) -> Result<ReportPublished, AppError> {
        let draft_id = require_field(req.draft_id.as_deref(), "draft_id")?.to_string();
        let expected = req.expected_revision;

        let draft = self.find_draft(&draft_id).await?;
        if let Some(report_id) = &draft.published_report_id {
            let report_id = report_id.clone();
            let report = self
                .store
                .read(move |conn| reports::get_report(conn, &report_id))
                .await?;
            return Ok(ReportPublished {
                draft_id,
                report_id: report.report_id.clone(),
                report,
            });
        }
        self.check_draft_writable(&draft, req.role.as_deref())
            .await?;

        let now = db::now_rfc3339();
        let draft_id_for_write = draft_id.clone();
        let result = self
            .store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let current = drafts::get_draft(&tx, &draft_id_for_write)?;
                if let Some(report_id) = &current.published_report_id {
                    let report = reports::get_report(&tx, report_id)?;
                    tx.commit()?;
                    return Ok(ReportPublished {
                        draft_id: current.draft_id,
                        report_id: report.report_id.clone(),
                        report,
                    });
                }
                check_revision(&current, expected)?;
                ensure_stage_open(&tx, &current)?;
                let blocks = drafts::list_blocks(&tx, &draft_id_for_write)?;
                let rendered = render_draft(&blocks);
                validate_published(&current.kind, &blocks, &rendered)?;
                if reports::report_for_stage_kind(&tx, &current.stage_id, &current.kind)?.is_some()
                {
                    return Err(StoreError::Conflict(
                        "в этом этапе уже есть отчёт такого вида".to_string(),
                    ));
                }
                let report = reports::insert_report(
                    &tx,
                    &current.task_id,
                    Some(&current.stage_id),
                    None,
                    &current.kind,
                    &current.title,
                    &rendered,
                    &now,
                )?;
                drafts::mark_published(&tx, &draft_id_for_write, &report.report_id, &now)?;
                stages::touch_stage(&tx, &current.stage_id, &now)?;
                let payload = serde_json::json!({
                    "draft_id": &draft_id_for_write,
                    "report_id": &report.report_id,
                    "kind": &current.kind,
                })
                .to_string();
                events::insert_event(
                    &tx,
                    &current.task_id,
                    "report_published",
                    Some(&current.stage_id),
                    Some(&current.actor),
                    &payload,
                    "agent",
                )?;
                tx.commit()?;
                Ok(ReportPublished {
                    draft_id: draft_id_for_write,
                    report_id: report.report_id.clone(),
                    report,
                })
            })
            .await;

        match result {
            Ok(value) => Ok(value),
            Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(ffi_err, _)))
                if ffi_err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(AppError::conflict(
                    "в этом этапе уже есть отчёт такого вида",
                ))
            }
            Err(err) => Err(err.into()),
        }
    }

    pub async fn report_discard(&self, req: ReportDiscardRequest) -> Result<(), AppError> {
        let draft_id = require_field(req.draft_id.as_deref(), "draft_id")?.to_string();
        let draft = self.find_draft(&draft_id).await?;
        self.check_draft_writable(&draft, req.role.as_deref())
            .await?;

        self.store
            .write(move |conn| {
                let tx = conn.transaction()?;
                let current = drafts::get_draft(&tx, &draft_id)?;
                if current.published_report_id.is_some() {
                    return Err(StoreError::Conflict("черновик уже опубликован".to_string()));
                }
                drafts::delete_draft(&tx, &draft_id)?;
                let payload = serde_json::json!({"draft_id": &draft_id}).to_string();
                events::insert_event(
                    &tx,
                    &current.task_id,
                    "report_draft_discarded",
                    Some(&current.stage_id),
                    Some(&current.actor),
                    &payload,
                    "agent",
                )?;
                tx.commit()?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    async fn find_draft(&self, draft_id: &str) -> Result<ReportDraft, AppError> {
        let draft_id = draft_id.to_string();
        let draft = self
            .store
            .read(move |conn| drafts::get_draft(conn, &draft_id))
            .await;
        match draft {
            Ok(draft) => Ok(draft),
            Err(StoreError::NotFound) => Err(AppError::not_found("Черновик не найден")),
            Err(err) => Err(err.into()),
        }
    }

    async fn check_draft_writable(
        &self,
        draft: &ReportDraft,
        role: Option<&str>,
    ) -> Result<(), AppError> {
        if is_expired(&draft.created_at, &db::now_rfc3339()) {
            return Err(AppError::conflict("черновик устарел, откройте новый"));
        }
        if draft.published_report_id.is_some() {
            return Err(AppError::conflict("черновик уже опубликован"));
        }
        let role = match role.map(str::trim).filter(|value| !value.is_empty()) {
            Some(role) => role.to_string(),
            None => {
                let task_id = draft.task_id.clone();
                let open = self
                    .store
                    .read(move |conn| stages::open_stage(conn, &task_id))
                    .await?;
                match open {
                    Some(stage) => stage.role,
                    None => return Err(AppError::role_missing()),
                }
            }
        };
        if role != draft.actor {
            return Err(AppError::conflict(format!(
                "черновик принадлежит роли `{}`",
                draft.actor
            )));
        }
        Ok(())
    }
}

fn is_valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .chars()
            .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-')
}

fn normalize_section_title(key: &str, title: Option<&str>) -> String {
    let title = title.map(str::trim).unwrap_or_default();
    if !title.is_empty() {
        return title.to_string();
    }
    match key {
        "summary" => String::from("Итог"),
        "checks" => String::from("Что проверено"),
        "changes" => String::from("Изменения"),
        _ => String::new(),
    }
}

fn check_revision(draft: &ReportDraft, expected: Option<i64>) -> Result<(), StoreError> {
    if let Some(expected) = expected {
        if expected != draft.revision {
            return Err(StoreError::from(AppError::conflict(format!(
                "черновик изменился: ожидалась ревизия {expected}, сейчас {}",
                draft.revision
            ))));
        }
    }
    Ok(())
}

fn ensure_stage_open(conn: &rusqlite::Connection, draft: &ReportDraft) -> Result<(), StoreError> {
    let open = stages::open_stage(conn, &draft.task_id)?;
    match open {
        Some(stage) if stage.id == draft.stage_id => Ok(()),
        Some(_) => Err(StoreError::Conflict(
            "этап черновика уже закрыт, работа идёт в другом этапе".to_string(),
        )),
        None => Err(StoreError::Conflict(
            "этап черновика уже закрыт".to_string(),
        )),
    }
}

fn validate_published(
    kind: &str,
    blocks: &[crate::domain::DraftBlock],
    rendered: &str,
) -> Result<(), StoreError> {
    let text = rendered.trim();
    if text.is_empty() {
        return Err(StoreError::from(AppError::field_invalid(
            "draft",
            "Черновик пуст: добавьте хотя бы один раздел.",
        )));
    }
    if text.chars().count() < MIN_PUBLISHED_CHARS {
        return Err(StoreError::from(AppError::field_invalid(
            "draft",
            format!("Отчёт короче {MIN_PUBLISHED_CHARS} знаков."),
        )));
    }
    if kind == "acceptance" {
        if !has_section(blocks, "summary", "Итог", rendered) {
            return Err(StoreError::from(AppError::field_invalid(
                "summary",
                "Для приёмки нужен раздел «Итог» с ключом `summary`.",
            )));
        }
        if !has_section(blocks, "checks", "Что проверено", rendered) {
            return Err(StoreError::from(AppError::field_invalid(
                "checks",
                "Для приёмки нужен раздел «Что проверено» с ключом `checks`.",
            )));
        }
    }
    Ok(())
}

fn has_section(
    blocks: &[crate::domain::DraftBlock],
    key: &str,
    title: &str,
    rendered: &str,
) -> bool {
    blocks
        .iter()
        .any(|block| block.key == key && !block.body.trim().is_empty())
        || rendered.contains(&format!("## {title}"))
}

pub(crate) fn render_draft(blocks: &[crate::domain::DraftBlock]) -> String {
    let mut out = String::new();
    for block in blocks {
        let title = block.title.trim();
        if !title.is_empty() {
            out.push_str("## ");
            out.push_str(title);
            out.push_str("\n\n");
        }
        out.push_str(block.body.trim());
        out.push_str("\n\n");
    }
    out.trim_end().to_string()
}

pub(crate) fn is_expired(created_at: &str, now: &str) -> bool {
    let Ok(created) = OffsetDateTime::parse(created_at, &Rfc3339) else {
        return false;
    };
    let Ok(now) = OffsetDateTime::parse(now, &Rfc3339) else {
        return false;
    };
    (now - created).whole_seconds() > DRAFT_TTL_SECONDS
}
