use rusqlite::Connection;

use crate::domain::report::preview;
use crate::domain::{Project, ProjectConfig, RoleInput};
use crate::store::{reports, StoreError};

pub fn resolve(
    conn: &Connection,
    project: &Project,
    task_id: &str,
    role: &str,
    current_round: i64,
) -> Result<Vec<RoleInput>, StoreError> {
    let config: ProjectConfig = serde_json::from_str(&project.config_json)?;
    let rules = config.inputs.get(role).cloned().unwrap_or_default();
    let mut out = Vec::new();

    for rule in rules {
        let found = reports::find_report_for_rule(
            conn,
            task_id,
            &rule.from,
            &rule.kind,
            &rule.round,
            current_round,
            role,
        )?;
        match found {
            Some((round, report)) => out.push(RoleInput {
                role: rule.from,
                round,
                kind: report.kind,
                report_id: report.report_id,
                title: report.title,
                preview: preview(&report.content),
            }),
            None if rule.required => {
                return Err(StoreError::InputMissing {
                    role: rule.from,
                    kind: rule.kind,
                    round: rule.round,
                })
            }
            None => {}
        }
    }

    Ok(out)
}
