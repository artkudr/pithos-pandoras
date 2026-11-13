-- Комментарий к файлу: имя файла — это его имя, а не подпись.
-- Только вперёд: старые миграции не правим.

ALTER TABLE files ADD COLUMN comment TEXT NOT NULL DEFAULT '';

-- Раньше в title хранили подпись к файлу; переносим её в комментарий.
UPDATE files SET comment = title WHERE title <> '';
