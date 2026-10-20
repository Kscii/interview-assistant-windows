use crate::{model::*, normalize::question_key};
use anyhow::{Context, Result};
use rusqlite::{Connection as Db, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

pub struct Store {
    db: Db,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::from_db(Db::open(path)?)
    }
    pub fn memory() -> Result<Self> {
        Self::from_db(Db::open_in_memory()?)
    }
    fn from_db(db: Db) -> Result<Self> {
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS objects(kind TEXT NOT NULL,id TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(kind,id));
            CREATE TABLE IF NOT EXISTS answers(id TEXT PRIMARY KEY,profile TEXT NOT NULL,revision TEXT NOT NULL,qkey TEXT NOT NULL,prepared INTEGER NOT NULL,enabled INTEGER NOT NULL,data TEXT NOT NULL,created INTEGER NOT NULL DEFAULT(unixepoch()));
            CREATE INDEX IF NOT EXISTS answer_lookup ON answers(profile,revision,qkey,enabled,prepared);
            PRAGMA user_version=1;")?;
        Ok(Self { db })
    }
    pub fn put<T: Serialize>(&self, kind: &str, id: &str, data: &T) -> Result<()> {
        self.db.execute("INSERT INTO objects VALUES(?1,?2,?3) ON CONFLICT(kind,id) DO UPDATE SET data=excluded.data", params![kind,id,serde_json::to_string(data)?])?;
        Ok(())
    }
    pub fn get<T: DeserializeOwned>(&self, kind: &str, id: &str) -> Result<Option<T>> {
        let data: Option<String> = self
            .db
            .query_row(
                "SELECT data FROM objects WHERE kind=?1 AND id=?2",
                params![kind, id],
                |r| r.get(0),
            )
            .optional()?;
        data.map(|s| serde_json::from_str(&s).context("读取数据失败"))
            .transpose()
    }
    pub fn list<T: DeserializeOwned>(&self, kind: &str) -> Result<Vec<T>> {
        let mut stmt = self
            .db
            .prepare("SELECT data FROM objects WHERE kind=?1 ORDER BY rowid")?;
        let rows = stmt.query_map([kind], |r| r.get::<_, String>(0))?;
        rows.map(|s| Ok(serde_json::from_str(&s?)?)).collect()
    }
    pub fn delete(&self, kind: &str, id: &str) -> Result<()> {
        self.db.execute(
            "DELETE FROM objects WHERE kind=?1 AND id=?2",
            params![kind, id],
        )?;
        Ok(())
    }
    pub fn save_answer(&self, answer: &Answer) -> Result<()> {
        self.db.execute("INSERT INTO answers(id,profile,revision,qkey,prepared,enabled,data) VALUES(?1,?2,?3,?4,?5,?6,?7)
            ON CONFLICT(id) DO UPDATE SET qkey=excluded.qkey,enabled=excluded.enabled,data=excluded.data",
            params![answer.id,answer.profile_id,answer.revision,question_key(&answer.question),answer.prepared,answer.enabled,serde_json::to_string(answer)?])?;
        Ok(())
    }
    pub fn lookup(&self, profile: &str, revision: &str, question: &str) -> Result<Option<Answer>> {
        let json: Option<String> = self.db.query_row("SELECT data FROM answers WHERE profile=?1 AND revision=?2 AND qkey=?3 AND enabled=1 ORDER BY prepared DESC, rowid DESC LIMIT 1",
            params![profile,revision,question_key(question)], |r| r.get(0)).optional()?;
        json.map(|s| Ok(serde_json::from_str(&s)?)).transpose()
    }
    pub fn answers(&self, profile: &str) -> Result<Vec<Answer>> {
        let mut stmt = self
            .db
            .prepare("SELECT data FROM answers WHERE profile=?1 ORDER BY rowid DESC")?;
        let rows = stmt.query_map([profile], |r| r.get::<_, String>(0))?;
        rows.map(|s| Ok(serde_json::from_str(&s?)?)).collect()
    }
    pub fn delete_answer(&self, id: &str) -> Result<()> {
        self.db.execute("DELETE FROM answers WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn clear_cache(&self, profile: &str) -> Result<()> {
        self.db.execute(
            "DELETE FROM answers WHERE profile=?1 AND prepared=0",
            [profile],
        )?;
        Ok(())
    }
    pub fn delete_profile(&self, id: &str) -> Result<()> {
        self.db
            .execute("DELETE FROM answers WHERE profile=?1", [id])?;
        self.delete("profile", id)
    }
}
