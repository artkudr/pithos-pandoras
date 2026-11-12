use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::domain::Comment;

use super::error::StoreError;

pub fn insert_comment(
    conn: &Connection,
    task_id: &str,
    author: &str,
    body: &str,
    created_at: &str,
) -> Result<Comment, StoreError> {
    let id = Uuid::now_v7().to_string();
    conn.execute(
        "INSERT INTO comments (id, task_id, author, body, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, task_id, author, body, created_at],
    )?;
    Ok(Comment {
        id,
        task_id: task_id.to_string(),
        author: author.to_string(),
        body: body.to_string(),
        created_at: created_at.to_string(),
    })
}

pub fn list_comments(conn: &Connection, task_id: &str) -> Result<Vec<Comment>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, author, body, created_at
         FROM comments WHERE task_id = ?1 ORDER BY created_at, id",
    )?;
    let rows = stmt.query_map([task_id], |row| {
        Ok(Comment {
            id: row.get(0)?,
            task_id: row.get(1)?,
            author: row.get(2)?,
            body: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
