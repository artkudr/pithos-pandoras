-- Проекты: переименование таблиц и колонок из workspaces.
-- Только вперёд: старые миграции не правим.

ALTER TABLE workspaces RENAME TO projects;
ALTER TABLE workspace_counters RENAME TO project_counters;
ALTER TABLE workspace_statuses RENAME TO project_statuses;
ALTER TABLE workspace_roles RENAME TO project_roles;

ALTER TABLE project_counters RENAME COLUMN workspace_id TO project_id;
ALTER TABLE project_statuses RENAME COLUMN workspace_id TO project_id;
ALTER TABLE project_roles RENAME COLUMN workspace_id TO project_id;
ALTER TABLE tasks RENAME COLUMN workspace_id TO project_id;

DROP INDEX IF EXISTS tasks_workspace_status;
CREATE INDEX IF NOT EXISTS tasks_project_status ON tasks(project_id, status_id);
