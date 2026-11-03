//! Explicit, durable and no-clobber organization of complete local file sets.
//! Originals and removed output bytes remain in operation-owned quarantines;
//! startup records interruption and never replays filesystem mutations.
mod fs;
use self::fs::{RootCap, Stamp};
use crate::{
    library::{LibraryRepository, RepositoryError},
    media_output::AdultOutputAccess,
};
use cap_std::fs::{File, OpenOptions};
use chrono::Utc;
use mediaatlas_domain::{normalize_relative_path, LibraryRoot, MediaFile, MediaItem, MediaKind};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::{Seek, SeekFrom},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use uuid::Uuid;
type Discovery = (BTreeMap<String, Vec<String>>, BTreeSet<(Uuid, String)>);
type Result<T> = std::result::Result<T, RepositoryError>;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationStrategy {
    #[default]
    Preserve,
    Move,
    Copy,
    Hardlink,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationRequest {
    pub media_item_id: Uuid,
    pub target_root_id: Uuid,
    pub template: String,
    #[serde(default)]
    pub strategy: OrganizationStrategy,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationFile {
    pub source: String,
    pub target: String,
    pub role: String,
    pub bytes: u64,
    pub conflict: Option<String>,
    pub same_path: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationPlan {
    pub id: Uuid,
    pub media_item_id: Uuid,
    pub target_root_id: Uuid,
    pub strategy: OrganizationStrategy,
    pub state: String,
    pub files: Vec<OrganizationFile>,
    pub estimated_bytes: u64,
    pub file_count: usize,
    pub warnings: Vec<String>,
    pub kind: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationResult {
    pub id: Uuid,
    pub state: String,
    pub completed: usize,
    pub already_applied: bool,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationLog {
    pub file_index: i64,
    pub stage: String,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    file: MediaFile,
    target: String,
    target_id: Uuid,
    stamp: Stamp,
    role: String,
    indexed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DirectoryStamp {
    root_id: Uuid,
    relative: String,
    identity: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Plan {
    rollback_files: Option<Vec<MediaFile>>,
    directories: Vec<DirectoryStamp>,
    view: OrganizationPlan,
    item: MediaItem,
    roots: Vec<LibraryRoot>,
    target_root: LibraryRoot,
    entries: Vec<Entry>,
    inventory: BTreeMap<String, Vec<String>>,
    lane: String,
    original: Option<Uuid>,
}
struct Session {
    _directories: Vec<cap_std::fs::Dir>,
    roots: HashMap<Uuid, RootCap>,
    sources: std::sync::Mutex<HashMap<usize, File>>,
    created: Instant,
    cancel: Arc<AtomicBool>,
    access: Option<AdultOutputAccess>,
    finished: AtomicBool,
    completion: tokio::sync::Notify,
}
struct Completion(Arc<Session>);
impl Drop for Completion {
    fn drop(&mut self) {
        self.0.finished.store(true, Ordering::SeqCst);
        self.0.completion.notify_waiters();
    }
}
fn sessions() -> &'static Mutex<HashMap<Uuid, Arc<Session>>> {
    static S: OnceLock<Mutex<HashMap<Uuid, Arc<Session>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}
fn preview_budget() -> &'static tokio::sync::Semaphore {
    static B: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    &B
}
fn mutation() -> &'static Mutex<()> {
    static M: OnceLock<Mutex<()>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(()))
}
#[derive(Clone)]
pub struct OrganizationService {
    repo: LibraryRepository,
    access: Option<AdultOutputAccess>,
}
impl OrganizationService {
    pub fn new(repo: LibraryRepository) -> Self {
        Self { repo, access: None }
    }
    pub fn new_adult(repo: LibraryRepository, access: AdultOutputAccess) -> Self {
        Self {
            repo,
            access: Some(access),
        }
    }
    fn lane(&self) -> &'static str {
        if self.access.is_some() {
            "adult"
        } else {
            "ordinary"
        }
    }
    fn access(&self) -> Result<()> {
        if let Some(a) = &self.access {
            a.ensure()?;
        }
        Ok(())
    }
    pub async fn recover_interrupted(&self) -> Result<u64> {
        Ok(sqlx::query("UPDATE organization_operations SET state='interrupted',error_code='interrupted',updated_at=? WHERE state='running'").bind(Utc::now().to_rfc3339()).execute(self.repo.pool()).await?.rows_affected())
    }
    async fn authority(
        &self,
        media_id: Uuid,
        target_id: Uuid,
    ) -> Result<(MediaItem, Vec<MediaFile>, Vec<LibraryRoot>, LibraryRoot)> {
        self.access()?;
        let item = self
            .repo
            .get_media(media_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let isolated: bool =
            sqlx::query_scalar("SELECT adult_isolated FROM media_items WHERE id=?")
                .bind(media_id.to_string())
                .fetch_one(self.repo.pool())
                .await?;
        let adult = self.access.is_some();
        if (adult && (!isolated || item.kind != MediaKind::AdultVideo))
            || (!adult && (isolated || item.kind == MediaKind::AdultVideo))
        {
            return Err(RepositoryError::InvalidInput);
        }
        // Organization is a typed video workflow; Music retains its own domain tools.
        if !matches!(
            item.kind,
            MediaKind::Movie
                | MediaKind::TvSeries
                | MediaKind::TvSeason
                | MediaKind::TvEpisode
                | MediaKind::Anime
                | MediaKind::AdultVideo
        ) {
            return Err(RepositoryError::InvalidInput);
        }
        let files = self.repo.list_files(media_id).await?;
        if files.len() > 4096 {
            return Err(RepositoryError::InvalidInput);
        }
        if files.is_empty() {
            return Err(RepositoryError::InvalidInput);
        }
        let mut roots = vec![];
        for f in &files {
            if roots.iter().any(|r: &LibraryRoot| r.id == f.root_id) {
                continue;
            }
            let r = self
                .repo
                .get_root(f.root_id)
                .await?
                .ok_or(RepositoryError::NotFound)?;
            if !r.enabled || !compatible_root(item.kind, r.kind, adult) {
                return Err(RepositoryError::InvalidInput);
            }
            roots.push(r);
        }
        let target = self
            .repo
            .get_root(target_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        if !target.enabled || !compatible_root(item.kind, target.kind, adult) {
            return Err(RepositoryError::InvalidInput);
        }
        Ok((item, files, roots, target))
    }
    pub async fn preview(&self, input: OrganizationRequest) -> Result<OrganizationPlan> {
        let _budget = preview_budget()
            .acquire()
            .await
            .map_err(|_| RepositoryError::Conflict)?;
        if input.template.is_empty() || input.template.len() > 1024 {
            return Err(RepositoryError::InvalidInput);
        }
        let (item, files, roots, target) = self
            .authority(input.media_item_id, input.target_root_id)
            .await?;
        let mut foreign = BTreeSet::new();
        for root in &roots {
            let rows = sqlx::query(
                "SELECT relative_path FROM media_files WHERE root_id=? AND media_item_id<>?",
            )
            .bind(root.id.to_string())
            .bind(item.id.to_string())
            .fetch_all(self.repo.pool())
            .await?;
            for row in rows {
                foreign.insert((root.id.to_string(), row.get::<String, _>("relative_path")));
            }
        }
        let access = self.access.clone();
        let lane = self.lane().to_owned();
        let context = PreviewContext {
            input,
            item,
            files,
            roots,
            target,
            foreign,
            access,
            lane,
        };
        let (plan, session) = tokio::task::spawn_blocking(move || {
            fs::with_budget(context.access.clone(), || build_preview(context))
        })
        .await
        .map_err(|_| RepositoryError::Conflict)??;
        let view = plan.view.clone();
        self.persist(&plan, session).await?;
        Ok(view)
    }
    async fn persist(&self, plan: &Plan, session: Session) -> Result<()> {
        self.access()?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO organization_operations(id,lane,kind,original_id,media_item_id,plan_json,state,created_at,updated_at) VALUES(?,?,?,?,?,?,'preview',?,?)").bind(plan.view.id.to_string()).bind(&plan.lane).bind(&plan.view.kind).bind(plan.original.map(|id|id.to_string())).bind(plan.item.id.to_string()).bind(serde_json::to_string(plan).map_err(|_|RepositoryError::CorruptData)?).bind(&now).bind(&now).execute(self.repo.pool()).await?;
        let mut active = sessions().lock().await;
        active.retain(|_, s| {
            s.created.elapsed() < Duration::from_secs(600)
                && s.access.as_ref().is_none_or(|a| a.ensure().is_ok())
        });
        if active.len() >= 32 {
            if let Some(id) = active
                .iter()
                .min_by_key(|(_, s)| s.created)
                .map(|(id, _)| *id)
            {
                active.remove(&id);
            }
        }
        active.insert(plan.view.id, Arc::new(session));
        drop(active);
        let id = plan.view.id;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(600)).await;
            sessions().lock().await.remove(&id);
        });
        Ok(())
    }
    async fn load(&self, id: Uuid) -> Result<(Plan, String)> {
        self.access()?;
        let row = sqlx::query(
            "SELECT plan_json,state FROM organization_operations WHERE id=? AND lane=?",
        )
        .bind(id.to_string())
        .bind(self.lane())
        .fetch_optional(self.repo.pool())
        .await?
        .ok_or(RepositoryError::NotFound)?;
        let p: Plan = serde_json::from_str(&row.get::<String, _>("plan_json"))
            .map_err(|_| RepositoryError::CorruptData)?;
        self.authority(p.item.id, p.target_root.id)
            .await
            .map_err(|e| {
                if matches!(e, RepositoryError::Database | RepositoryError::CorruptData) {
                    e
                } else {
                    RepositoryError::NotFound
                }
            })?;
        for historical in p.roots.iter().chain(std::iter::once(&p.target_root)) {
            let current = self
                .repo
                .get_root(historical.id)
                .await?
                .ok_or(RepositoryError::NotFound)?;
            if !current.enabled
                || ((current.kind == Some(MediaKind::AdultVideo)) != self.access.is_some())
            {
                return Err(RepositoryError::NotFound);
            }
        }
        Ok((p, row.get("state")))
    }
    pub async fn plan(&self, id: Uuid) -> Result<OrganizationPlan> {
        let (p, state) = self.load(id).await?;
        let mut view = p.view;
        view.state = state;
        Ok(view)
    }
    pub async fn history(&self) -> Result<Vec<OrganizationResult>> {
        self.access()?;
        let rows=sqlx::query("SELECT id FROM organization_operations WHERE lane=? ORDER BY created_at DESC LIMIT 100").bind(self.lane()).fetch_all(self.repo.pool()).await?;
        let mut result = vec![];
        for row in rows {
            let id = Uuid::parse_str(&row.get::<String, _>("id"))
                .map_err(|_| RepositoryError::CorruptData)?;
            match self.load(id).await {
                Ok(_) => result.push(self.result(id, false).await?),
                Err(
                    RepositoryError::NotFound
                    | RepositoryError::InvalidInput
                    | RepositoryError::Conflict
                    | RepositoryError::ProtectedMetadata,
                ) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(result)
    }
    pub async fn logs(&self, id: Uuid) -> Result<Vec<OrganizationLog>> {
        self.load(id).await?;
        Ok(sqlx::query("SELECT file_index,stage,created_at FROM organization_journal WHERE operation_id=? ORDER BY sequence").bind(id.to_string()).fetch_all(self.repo.pool()).await?.iter().map(|r|OrganizationLog{file_index:r.get("file_index"),stage:r.get("stage"),created_at:r.get("created_at")}).collect())
    }
    async fn result(&self, id: Uuid, already_applied: bool) -> Result<OrganizationResult> {
        let row = sqlx::query(
            "SELECT state,error_code FROM organization_operations WHERE id=? AND lane=?",
        )
        .bind(id.to_string())
        .bind(self.lane())
        .fetch_one(self.repo.pool())
        .await?;
        let n:i64=sqlx::query_scalar("SELECT count(DISTINCT file_index) FROM organization_journal WHERE operation_id=? AND stage='file_complete'").bind(id.to_string()).fetch_one(self.repo.pool()).await?;
        Ok(OrganizationResult {
            id,
            state: row.get("state"),
            error_code: row.get("error_code"),
            completed: n as usize,
            already_applied,
        })
    }
    async fn journal(
        &self,
        id: Uuid,
        index: usize,
        stage: &str,
        stamp: Option<&Stamp>,
    ) -> Result<()> {
        sqlx::query("INSERT INTO organization_journal(operation_id,file_index,stage,stamp_json,created_at) VALUES(?,?,?,?,?)").bind(id.to_string()).bind(index as i64).bind(stage).bind(stamp.map(serde_json::to_string).transpose().map_err(|_|RepositoryError::CorruptData)?).bind(Utc::now().to_rfc3339()).execute(self.repo.pool()).await?;
        Ok(())
    }
    async fn published(&self, id: Uuid, index: usize) -> Result<Option<Stamp>> {
        let value:Option<String>=sqlx::query_scalar("SELECT stamp_json FROM organization_journal WHERE operation_id=? AND file_index=? AND stage='publish_prepared' ORDER BY sequence DESC LIMIT 1").bind(id.to_string()).bind(index as i64).fetch_optional(self.repo.pool()).await?.flatten();
        value
            .map(|s| serde_json::from_str(&s).map_err(|_| RepositoryError::CorruptData))
            .transpose()
    }
    pub async fn cancel(&self, id: Uuid) -> Result<OrganizationResult> {
        self.load(id).await?;
        let active = sessions().lock().await.get(&id).cloned();
        if let Some(s) = &active {
            s.cancel.store(true, Ordering::SeqCst);
        }
        let changed=sqlx::query("UPDATE organization_operations SET state='cancelled',error_code='cancelled',updated_at=? WHERE id=? AND lane=? AND state='preview'").bind(Utc::now().to_rfc3339()).bind(id.to_string()).bind(self.lane()).execute(self.repo.pool()).await?.rows_affected();
        if changed == 1 {
            if let Some(s) = active {
                s.finished.store(true, Ordering::SeqCst);
                s.completion.notify_waiters();
            }
            sessions().lock().await.remove(&id);
        } else if let Some(s) = active {
            loop {
                let notified = s.completion.notified();
                if s.finished.load(Ordering::SeqCst) {
                    break;
                }
                notified.await;
            }
        }
        let result = self.result(id, false).await?;
        if result.state == "running" {
            return Err(RepositoryError::Conflict);
        }
        Ok(result)
    }
    pub async fn discard(&self, id: Uuid) -> Result<()> {
        self.cancel(id).await?;
        sessions().lock().await.remove(&id);
        Ok(())
    }
    pub async fn execute(&self, id: Uuid) -> Result<OrganizationResult> {
        self.apply(id, false).await
    }
    pub async fn rollback_execute(&self, id: Uuid) -> Result<OrganizationResult> {
        self.apply(id, true).await
    }
    async fn apply(&self, id: Uuid, rollback: bool) -> Result<OrganizationResult> {
        let (plan, state) = self.load(id).await?;
        if (plan.view.kind == "rollback") != rollback {
            return Err(RepositoryError::InvalidInput);
        }
        if state == "succeeded" {
            let service = self.clone();
            let checked = plan.clone();
            let handle = tokio::runtime::Handle::current();
            let access = self.access.clone();
            tokio::task::spawn_blocking(move || {
                fs::with_budget(access, || {
                    handle.block_on(service.verify_completed(&checked))
                })
            })
            .await
            .map_err(|_| RepositoryError::Conflict)??;
            return self.result(id, true).await;
        }
        if state != "preview" {
            return Err(RepositoryError::Conflict);
        }
        let session = sessions()
            .lock()
            .await
            .get(&id)
            .cloned()
            .ok_or(RepositoryError::Conflict)?;
        check_session(&session)?;
        let _lock = mutation().lock().await;
        let claimed=sqlx::query("UPDATE organization_operations SET state='running',updated_at=? WHERE id=? AND lane=? AND state='preview'").bind(Utc::now().to_rfc3339()).bind(id.to_string()).bind(self.lane()).execute(self.repo.pool()).await?.rows_affected();
        if claimed != 1 {
            return Err(RepositoryError::Conflict);
        }
        let _completion = Completion(session.clone());
        let service = self.clone();
        let executing = plan.clone();
        let active = session.clone();
        let handle = tokio::runtime::Handle::current();
        let applied = tokio::task::spawn_blocking(move || {
            #[cfg(test)]
            tests::execution_start(executing.view.id);
            fs::with_cancel(active.cancel.clone(), active.access.clone(), || {
                handle.block_on(async move {
                    if rollback {
                        service.run_rollback(&executing, &active).await
                    } else {
                        service.run_organization(&executing, &active).await
                    }
                })
            })
        })
        .await
        .map_err(|_| RepositoryError::Conflict)
        .and_then(|r| r);
        let (state, error) = match applied {
            Ok(()) => ("succeeded", None),
            Err(_e) if session.cancel.load(Ordering::SeqCst) => ("cancelled", Some("cancelled")),
            Err(_) => ("conflict", Some("organization_conflict")),
        };
        sqlx::query("UPDATE organization_operations SET state=?,error_code=?,updated_at=? WHERE id=? AND lane=? AND state='running'").bind(state).bind(error).bind(Utc::now().to_rfc3339()).bind(id.to_string()).bind(self.lane()).execute(self.repo.pool()).await?;
        sessions().lock().await.remove(&id);
        self.result(id, false).await
    }
    async fn validate_plan(&self, plan: &Plan) -> Result<()> {
        let (item, files, roots, target) =
            self.authority(plan.item.id, plan.target_root.id).await?;
        if item != plan.item
            || roots != plan.roots
            || target != plan.target_root
            || files
                != plan
                    .entries
                    .iter()
                    .filter(|e| e.indexed)
                    .map(|e| e.file.clone())
                    .collect::<Vec<_>>()
        {
            return Err(RepositoryError::Conflict);
        }
        Ok(())
    }
    async fn run_organization(&self, plan: &Plan, session: &Session) -> Result<()> {
        self.validate_plan(plan).await?;
        check_session(session)?;
        check_directories(plan, &session.roots, true)?;
        if plan.view.files.iter().any(|f| f.conflict.is_some()) {
            return Err(RepositoryError::Conflict);
        }
        let mut all = plan
            .entries
            .iter()
            .filter(|e| e.indexed)
            .map(|e| {
                (
                    e.file.root_id,
                    e.file.relative_path.clone(),
                    e.file.clone(),
                    true,
                )
            })
            .collect::<Vec<_>>();
        let files = plan
            .entries
            .iter()
            .filter(|e| e.indexed)
            .map(|e| e.file.clone())
            .collect::<Vec<_>>();
        if discover(&files, &session.roots, &mut all)?.0 != plan.inventory {
            return Err(RepositoryError::Conflict);
        }
        for (index, e) in plan.entries.iter().enumerate() {
            check_session(session)?;
            let cap = &session.roots[&e.file.root_id];
            cap.recheck()?;
            let p = cap.parent(&e.file.relative_path, false)?;
            if fs::optional_stamp(&p.dir, &p.name)?.as_ref() != Some(&e.stamp) {
                return Err(RepositoryError::Conflict);
            }
            let _ = index;
        }
        let target = &session.roots[&plan.target_root.id];
        target.recheck()?;
        // Prepare durable ownership records before taking the SQLite writer lease.
        // Copy stages never change a source; failed/crashed stages are retained.
        let stage_name = format!(".mediaatlas-organization-{}", plan.view.id);
        let stage = if plan.view.strategy != OrganizationStrategy::Preserve {
            Some(fs::create_owned(&target.root, &stage_name)?)
        } else {
            None
        };
        let mut prepared_parents = HashMap::new();
        for (index, e) in plan.entries.iter().enumerate() {
            if plan.view.strategy == OrganizationStrategy::Preserve
                || plan.view.files[index].same_path
            {
                continue;
            }
            check_session(session)?;
            let src = session.roots[&e.file.root_id].parent(&e.file.relative_path, false)?;
            let dst = target.parent(&e.target, true)?;
            if target_exists(target, &e.target)? {
                return Err(RepositoryError::Conflict);
            }
            prepared_parents.insert(index, dst);
            let stage = stage.as_ref().ok_or(RepositoryError::CorruptData)?;
            let name = index.to_string();
            self.journal(plan.view.id, index, "stage_intent", None)
                .await?;
            if plan.view.strategy == OrganizationStrategy::Hardlink {
                #[cfg(test)]
                tests::checkpoint(plan.view.id, "hardlink_unsupported")?;
                src.dir
                    .hard_link(&src.name, stage, &name)
                    .map_err(fs::err)?;
            } else {
                let mut source = fs::file(&src.dir, &src.name)?;
                if fs::stamp(&mut source)? != e.stamp {
                    return Err(RepositoryError::Conflict);
                }
                source.seek(SeekFrom::Start(0)).map_err(fs::err)?;
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                let mut output = stage.open_with(&name, &options).map_err(fs::err)?;
                copy_stream(&mut source, &mut output)?;
                output.sync_all().map_err(fs::err)?;
                if fs::stamp(&mut source)? != e.stamp {
                    return Err(RepositoryError::Conflict);
                }
            }
            let output = fs::optional_stamp(stage, &name)?.ok_or(RepositoryError::Conflict)?;
            if output.len != e.stamp.len || output.hash != e.stamp.hash {
                return Err(RepositoryError::Conflict);
            }
            self.journal(plan.view.id, index, "publish_prepared", Some(&output))
                .await?;
            #[cfg(test)]
            tests::checkpoint(plan.view.id, "stage_prepared")?;
            self.journal(plan.view.id, index, "publish_intent", None)
                .await?;
            if plan.view.strategy == OrganizationStrategy::Move {
                self.journal(
                    plan.view.id,
                    index,
                    "source_quarantine_intent",
                    Some(&e.stamp),
                )
                .await?;
            }
        }
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("UPDATE organization_operations SET updated_at=updated_at WHERE id=?")
            .bind(plan.view.id.to_string())
            .execute(&mut *tx)
            .await?;
        validate_tx(&mut tx, plan, self.access.is_some()).await?;
        check_session(session)?;
        for (index, e) in plan.entries.iter().enumerate() {
            check_session(session)?;
            if plan.view.strategy == OrganizationStrategy::Preserve
                || plan.view.files[index].same_path
            {
                journal_tx(&mut tx, plan.view.id, index, "file_complete", None).await?;
                continue;
            }
            let src_cap = &session.roots[&e.file.root_id];
            src_cap.recheck()?;
            target.recheck()?;
            check_directories(plan, &session.roots, false)?;
            let src = src_cap.parent(&e.file.relative_path, false)?;
            check_source_parent(plan, e.file.root_id, &e.file.relative_path, &src.dir)?;
            let dst = prepared_parents
                .get(&index)
                .ok_or(RepositoryError::CorruptData)?;
            let current = target.parent(&e.target, false)?;
            if fs::identity(&current.dir.dir_metadata().map_err(fs::err)?)?
                != fs::identity(&dst.dir.dir_metadata().map_err(fs::err)?)?
                || target_exists(target, &e.target)?
            {
                return Err(RepositoryError::Conflict);
            }
            if fs::optional_stamp(&src.dir, &src.name)?.as_ref() != Some(&e.stamp)
                || fs::optional_stamp(&dst.dir, &dst.name)?.is_some()
            {
                return Err(RepositoryError::Conflict);
            }
            let stage = stage.as_ref().ok_or(RepositoryError::CorruptData)?;
            let name = index.to_string();
            let owned = self
                .published(plan.view.id, index)
                .await?
                .ok_or(RepositoryError::CorruptData)?;
            if fs::optional_stamp(stage, &name)?.as_ref() != Some(&owned) {
                return Err(RepositoryError::Conflict);
            }
            self.access()?;
            check_session(session)?;
            fs::no_replace(
                stage,
                &name,
                &target.path.join(&stage_name).join(&name),
                &dst.dir,
                &dst.name,
                &dst.absolute.join(&dst.name),
            )?;
            if !current_parent(target, &e.target, &dst.dir)? {
                return Err(RepositoryError::Conflict);
            }
            journal_tx(&mut tx, plan.view.id, index, "published", Some(&owned)).await?;
            #[cfg(test)]
            tests::checkpoint(plan.view.id, "published")?;
            if plan.view.strategy == OrganizationStrategy::Move {
                let backup = format!(".mediaatlas-organization-{}-{index}", plan.view.id);
                let q = fs::create_owned(&src.dir, &backup)?;
                // Release only our original handle; all ancestor/root leases remain.
                session
                    .sources
                    .lock()
                    .map_err(|_| RepositoryError::Conflict)?
                    .remove(&index);
                self.access()?;
                check_session(session)?;
                fs::no_replace(
                    &src.dir,
                    &src.name,
                    &src.absolute.join(&src.name),
                    &q,
                    "source",
                    &src.absolute.join(&backup).join("source"),
                )?;
                // Validate the actual moved object. Never remove a replacement source.
                if fs::optional_stamp(&q, "source").ok().flatten().as_ref() != Some(&e.stamp)
                    || !current_parent(src_cap, &e.file.relative_path, &src.dir).unwrap_or(false)
                {
                    let _ = fs::no_replace(
                        &q,
                        "source",
                        &src.absolute.join(&backup).join("source"),
                        &src.dir,
                        &src.name,
                        &src.absolute.join(&src.name),
                    );
                    return Err(RepositoryError::Conflict);
                }
                journal_tx(
                    &mut tx,
                    plan.view.id,
                    index,
                    "source_quarantined",
                    Some(&e.stamp),
                )
                .await?;
                #[cfg(test)]
                tests::checkpoint(plan.view.id, "source_quarantined")?;
            }
            self.access()?;
            check_session(session)?;
            let actual =
                fs::optional_stamp(&dst.dir, &dst.name)?.ok_or(RepositoryError::Conflict)?;
            if actual != owned || !current_parent(target, &e.target, &dst.dir)? {
                return Err(RepositoryError::Conflict);
            }
            let relative = normalize_relative_path(&e.target, plan.target_root.path_style)
                .map_err(|_| RepositoryError::InvalidInput)?;
            let modified = i64::try_from(actual.modified / 1_000_000)
                .map_err(|_| RepositoryError::InvalidInput)?;
            if plan.view.strategy == OrganizationStrategy::Move && e.indexed {
                sqlx::query("UPDATE media_files SET root_id=?,relative_path=?,normalized_relative_path=?,size_bytes=?,modified_at=? WHERE id=? AND media_item_id=?").bind(plan.target_root.id.to_string()).bind(&e.target).bind(relative).bind(actual.len as i64).bind(modified).bind(e.file.id.to_string()).bind(plan.item.id.to_string()).execute(&mut *tx).await?;
            } else {
                sqlx::query("INSERT INTO media_files(id,media_item_id,root_id,relative_path,normalized_relative_path,size_bytes,modified_at,part) VALUES(?,?,?,?,?,?,?,?)").bind(e.target_id.to_string()).bind(plan.item.id.to_string()).bind(plan.target_root.id.to_string()).bind(&e.target).bind(relative).bind(actual.len as i64).bind(modified).bind(e.file.part.map(i64::from)).execute(&mut *tx).await?;
            }
            if plan.view.strategy == OrganizationStrategy::Move {
                relocate_references(
                    &mut tx,
                    plan.item.id,
                    &format!("library://{}/{}", e.file.root_id, e.file.relative_path),
                    &format!("library://{}/{}", plan.target_root.id, e.target),
                )
                .await?;
            }
            journal_tx(&mut tx, plan.view.id, index, "file_complete", None).await?;
        }
        self.access()?;
        check_session(session)?;
        tx.commit().await?;
        #[cfg(test)]
        tests::checkpoint(plan.view.id, "index_committed")?;
        Ok(())
    }
    pub async fn rollback_preview(&self, operation_id: Uuid) -> Result<OrganizationPlan> {
        let _budget = preview_budget()
            .acquire()
            .await
            .map_err(|_| RepositoryError::Conflict)?;
        let (original, state) = self.load(operation_id).await?;
        if original.view.kind != "organize"
            || !matches!(
                state.as_str(),
                "succeeded" | "interrupted" | "conflict" | "cancelled"
            )
        {
            return Err(RepositoryError::Conflict);
        }
        self.access()?;
        let mut caps = HashMap::new();
        for r in original
            .roots
            .iter()
            .chain(std::iter::once(&original.target_root))
        {
            let current = self
                .repo
                .get_root(r.id)
                .await?
                .ok_or(RepositoryError::NotFound)?;
            if &current != r {
                return Err(RepositoryError::Conflict);
            }
            if let std::collections::hash_map::Entry::Vacant(e) = caps.entry(r.id) {
                e.insert(RootCap::open(Path::new(&r.path))?);
            }
        }
        let mut view = original.view.clone();
        view.id = Uuid::new_v4();
        view.kind = "rollback".into();
        view.state = "preview".into();
        view.warnings.push("Recovery quarantines owned destinations and restores moved originals without replacing external files.".into());
        let current_item = self
            .repo
            .get_media(original.item.id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let current_files = self.repo.list_files(original.item.id).await?;
        let mut directory_map = BTreeMap::new();
        let mut directory_leases = vec![];
        for e in &original.entries {
            for (root_id, relative) in [
                (e.file.root_id, &e.file.relative_path),
                (original.target_root.id, &e.target),
            ] {
                let (states, held) = caps[&root_id].directory_states(relative)?;
                directory_leases.extend(held);
                for (path, identity) in states {
                    directory_map.insert(
                        format!("{root_id}:{path}"),
                        DirectoryStamp {
                            root_id,
                            relative: path,
                            identity,
                        },
                    );
                }
            }
        }
        let plan = Plan {
            rollback_files: Some(current_files),
            item: current_item,
            directories: directory_map.into_values().collect(),
            view: view.clone(),
            original: Some(operation_id),
            ..original
        };
        let service = self.clone();
        let handle = tokio::runtime::Handle::current();
        let access = self.access.clone();
        let (plan, caps) = tokio::task::spawn_blocking(move || {
            fs::with_budget(access, || {
                handle.block_on(async move {
                    service.check_rollback(&plan, &caps).await?;
                    Ok::<_, RepositoryError>((plan, caps))
                })
            })
        })
        .await
        .map_err(|_| RepositoryError::Conflict)??;
        self.persist(
            &plan,
            Session {
                _directories: directory_leases,
                roots: caps,
                sources: std::sync::Mutex::new(HashMap::new()),
                created: Instant::now(),
                cancel: Arc::new(AtomicBool::new(false)),
                access: self.access.clone(),
                finished: AtomicBool::new(false),
                completion: tokio::sync::Notify::new(),
            },
        )
        .await?;
        Ok(view)
    }
    async fn check_rollback(&self, plan: &Plan, caps: &HashMap<Uuid, RootCap>) -> Result<()> {
        let original = plan.original.ok_or(RepositoryError::CorruptData)?;
        for (index, e) in plan.entries.iter().enumerate() {
            if plan.view.strategy == OrganizationStrategy::Preserve
                || plan.view.files[index].same_path
            {
                continue;
            }
            let target = caps[&plan.target_root.id].parent(&e.target, false)?;
            if let Some(actual) = fs::optional_stamp(&target.dir, &target.name)? {
                if self.published(original, index).await?.as_ref() != Some(&actual) {
                    return Err(RepositoryError::Conflict);
                }
            }
            if plan.view.strategy == OrganizationStrategy::Move {
                let src = caps[&e.file.root_id].parent(&e.file.relative_path, false)?;
                match fs::optional_stamp(&src.dir, &src.name)? {
                    Some(current) if current != e.stamp => return Err(RepositoryError::Conflict),
                    Some(_) => {}
                    None => {
                        let q = fs::directory(
                            &src.dir,
                            format!(".mediaatlas-organization-{original}-{index}"),
                        )?;
                        if fs::optional_stamp(&q, "source")?.as_ref() != Some(&e.stamp) {
                            return Err(RepositoryError::Conflict);
                        }
                    }
                }
            }
        }
        Ok(())
    }
    async fn run_rollback(&self, plan: &Plan, session: &Session) -> Result<()> {
        let original = plan.original.ok_or(RepositoryError::CorruptData)?;
        check_directories(plan, &session.roots, true)?;
        self.check_rollback(plan, &session.roots).await?;
        for (index, _) in plan.entries.iter().enumerate() {
            self.journal(plan.view.id, index, "rollback_quarantine_intent", None)
                .await?;
            self.journal(plan.view.id, index, "rollback_restore_intent", None)
                .await?;
        }
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("UPDATE organization_operations SET updated_at=updated_at WHERE id=?")
            .bind(original.to_string())
            .execute(&mut *tx)
            .await?;
        validate_rollback_tx(&mut tx, plan, self.access.is_some()).await?;
        check_session(session)?;
        for (index, e) in plan.entries.iter().enumerate() {
            check_session(session)?;
            if plan.view.strategy == OrganizationStrategy::Preserve
                || plan.view.files[index].same_path
            {
                journal_tx(&mut tx, plan.view.id, index, "file_complete", None).await?;
                continue;
            }
            for cap in session.roots.values() {
                cap.recheck()?;
            }
            check_directories(plan, &session.roots, true)?;
            let dst = session.roots[&plan.target_root.id].parent(&e.target, false)?;
            if let Some(actual) = fs::optional_stamp(&dst.dir, &dst.name)? {
                let owned = self
                    .published(original, index)
                    .await?
                    .ok_or(RepositoryError::Conflict)?;
                if actual != owned {
                    return Err(RepositoryError::Conflict);
                }
                let qname = format!(".mediaatlas-rollback-{}-{index}", plan.view.id);
                let q = fs::create_owned(&dst.dir, &qname)?;
                self.access()?;
                check_session(session)?;
                fs::no_replace(
                    &dst.dir,
                    &dst.name,
                    &dst.absolute.join(&dst.name),
                    &q,
                    "output",
                    &dst.absolute.join(&qname).join("output"),
                )?;
                if fs::optional_stamp(&q, "output").ok().flatten().as_ref() != Some(&owned)
                    || !current_parent(&session.roots[&plan.target_root.id], &e.target, &dst.dir)
                        .unwrap_or(false)
                {
                    let _ = fs::no_replace(
                        &q,
                        "output",
                        &dst.absolute.join(&qname).join("output"),
                        &dst.dir,
                        &dst.name,
                        &dst.absolute.join(&dst.name),
                    );
                    return Err(RepositoryError::Conflict);
                }
                journal_tx(
                    &mut tx,
                    plan.view.id,
                    index,
                    "rollback_target_quarantined",
                    Some(&owned),
                )
                .await?;
            }
            if plan.view.strategy == OrganizationStrategy::Move {
                let src = session.roots[&e.file.root_id].parent(&e.file.relative_path, false)?;
                match fs::optional_stamp(&src.dir, &src.name)? {
                    Some(current) if current != e.stamp => return Err(RepositoryError::Conflict),
                    Some(_) => {}
                    None => {
                        let qname = format!(".mediaatlas-organization-{original}-{index}");
                        let q = fs::directory(&src.dir, &qname)?;
                        if fs::optional_stamp(&q, "source")?.as_ref() != Some(&e.stamp) {
                            return Err(RepositoryError::Conflict);
                        }
                        self.access()?;
                        check_session(session)?;
                        fs::no_replace(
                            &q,
                            "source",
                            &src.absolute.join(&qname).join("source"),
                            &src.dir,
                            &src.name,
                            &src.absolute.join(&src.name),
                        )?;
                        if fs::optional_stamp(&src.dir, &src.name)?.as_ref() != Some(&e.stamp) {
                            return Err(RepositoryError::Conflict);
                        }
                    }
                }
                if e.indexed {
                    let root = plan
                        .roots
                        .iter()
                        .find(|r| r.id == e.file.root_id)
                        .ok_or(RepositoryError::CorruptData)?;
                    let normalized =
                        normalize_relative_path(&e.file.relative_path, root.path_style)
                            .map_err(|_| RepositoryError::InvalidInput)?;
                    sqlx::query("UPDATE media_files SET root_id=?,relative_path=?,normalized_relative_path=?,size_bytes=?,modified_at=?,part=? WHERE id=? AND media_item_id=?").bind(e.file.root_id.to_string()).bind(&e.file.relative_path).bind(normalized).bind(e.file.size_bytes as i64).bind(e.file.modified_at.map(|v|v.timestamp_millis())).bind(e.file.part.map(i64::from)).bind(e.file.id.to_string()).bind(plan.item.id.to_string()).execute(&mut *tx).await?;
                }
            }
            if plan.view.strategy == OrganizationStrategy::Move {
                relocate_references(
                    &mut tx,
                    plan.item.id,
                    &format!("library://{}/{}", plan.target_root.id, e.target),
                    &format!("library://{}/{}", e.file.root_id, e.file.relative_path),
                )
                .await?;
            }
            sqlx::query("DELETE FROM media_files WHERE id=? AND media_item_id=? AND root_id=? AND relative_path=?").bind(e.target_id.to_string()).bind(plan.item.id.to_string()).bind(plan.target_root.id.to_string()).bind(&e.target).execute(&mut *tx).await?;
            journal_tx(&mut tx, plan.view.id, index, "file_complete", None).await?;
        }
        self.access()?;
        check_session(session)?;
        tx.commit().await?;
        Ok(())
    }
    async fn verify_completed(&self, plan: &Plan) -> Result<()> {
        for root in plan.roots.iter().chain(std::iter::once(&plan.target_root)) {
            if self.repo.get_root(root.id).await?.as_ref() != Some(root) {
                return Err(RepositoryError::Conflict);
            }
        }
        for (index, e) in plan.entries.iter().enumerate() {
            let root = plan
                .roots
                .iter()
                .find(|r| r.id == e.file.root_id)
                .ok_or(RepositoryError::CorruptData)?;
            let cap = RootCap::open(Path::new(&root.path))?;
            let source = cap.parent(&e.file.relative_path, false)?;
            if plan.view.kind == "organize"
                && plan.view.strategy == OrganizationStrategy::Move
                && !plan.view.files[index].same_path
            {
                if fs::optional_stamp(&source.dir, &source.name)?.is_some() {
                    return Err(RepositoryError::Conflict);
                }
                let quarantine = fs::directory(
                    &source.dir,
                    format!(".mediaatlas-organization-{}-{index}", plan.view.id),
                )?;
                if fs::optional_stamp(&quarantine, "source")?.as_ref() != Some(&e.stamp) {
                    return Err(RepositoryError::Conflict);
                }
            } else if fs::optional_stamp(&source.dir, &source.name)?.as_ref() != Some(&e.stamp) {
                return Err(RepositoryError::Conflict);
            }
            if plan.view.kind == "rollback"
                && plan.view.strategy != OrganizationStrategy::Preserve
                && !plan.view.files[index].same_path
            {
                let target = RootCap::open(Path::new(&plan.target_root.path))?;
                let destination = target.parent(&e.target, false)?;
                if fs::optional_stamp(&destination.dir, &destination.name)?.is_some() {
                    return Err(RepositoryError::Conflict);
                }
            }
        }
        if plan.view.kind == "rollback" {
            self.authority(plan.item.id, plan.target_root.id).await?;
            for e in &plan.entries {
                if plan.view.strategy == OrganizationStrategy::Move {
                    let root = plan
                        .roots
                        .iter()
                        .find(|r| r.id == e.file.root_id)
                        .ok_or(RepositoryError::CorruptData)?;
                    let cap = RootCap::open(Path::new(&root.path))?;
                    let p = cap.parent(&e.file.relative_path, false)?;
                    if fs::optional_stamp(&p.dir, &p.name)?.as_ref() != Some(&e.stamp) {
                        return Err(RepositoryError::Conflict);
                    }
                }
            }
            return Ok(());
        }
        let (_, _, _, target) = self.authority(plan.item.id, plan.target_root.id).await?;
        if target != plan.target_root {
            return Err(RepositoryError::Conflict);
        }
        for (index, e) in plan.entries.iter().enumerate() {
            let (root, relative) = if plan.view.strategy == OrganizationStrategy::Preserve
                || plan.view.files[index].same_path
            {
                (
                    plan.roots
                        .iter()
                        .find(|r| r.id == e.file.root_id)
                        .ok_or(RepositoryError::CorruptData)?,
                    &e.file.relative_path,
                )
            } else {
                (&plan.target_root, &e.target)
            };
            let cap = RootCap::open(Path::new(&root.path))?;
            let p = cap.parent(relative, false)?;
            let expected = if plan.view.strategy == OrganizationStrategy::Preserve
                || plan.view.files[index].same_path
            {
                Some(e.stamp.clone())
            } else {
                self.published(plan.view.id, index).await?
            };
            if fs::optional_stamp(&p.dir, &p.name)? != expected {
                return Err(RepositoryError::Conflict);
            }
        }
        Ok(())
    }
}
fn compatible_root(item: MediaKind, root: Option<MediaKind>, adult: bool) -> bool {
    if adult {
        return item == MediaKind::AdultVideo && root == Some(MediaKind::AdultVideo);
    }
    match root {
        None => true,
        Some(kind) if kind == item => true,
        Some(MediaKind::TvSeries) => matches!(
            item,
            MediaKind::TvSeason | MediaKind::TvEpisode | MediaKind::Anime
        ),
        Some(MediaKind::TvSeason) => matches!(item, MediaKind::TvEpisode | MediaKind::Anime),
        _ => false,
    }
}
struct PreviewContext {
    input: OrganizationRequest,
    item: MediaItem,
    files: Vec<MediaFile>,
    roots: Vec<LibraryRoot>,
    target: LibraryRoot,
    foreign: BTreeSet<(String, String)>,
    access: Option<AdultOutputAccess>,
    lane: String,
}
fn build_preview(context: PreviewContext) -> Result<(Plan, Session)> {
    let PreviewContext {
        input,
        item,
        files,
        roots,
        target,
        foreign,
        access,
        lane,
    } = context;
    let mut caps = HashMap::new();
    for r in roots.iter().chain(std::iter::once(&target)) {
        if let std::collections::hash_map::Entry::Vacant(e) = caps.entry(r.id) {
            e.insert(RootCap::open(Path::new(&r.path))?);
        }
    }
    let mut all = files
        .iter()
        .cloned()
        .map(|f| (f.root_id, f.relative_path.clone(), f, true))
        .collect::<Vec<_>>();
    let (inventory, ambiguous) = discover(&files, &caps, &mut all)?;
    if all.len() > 4096 {
        return Err(RepositoryError::InvalidInput);
    }
    let mut entries = vec![];
    let mut views = vec![];
    let mut held = HashMap::new();
    let mut targets = BTreeSet::new();
    let mut warnings = vec![];
    let playable = files
        .iter()
        .filter(|f| is_media(&f.relative_path))
        .collect::<Vec<_>>();
    if playable.is_empty() {
        return Err(RepositoryError::InvalidInput);
    }
    for (index, (root_id, relative, f, indexed)) in all.into_iter().enumerate() {
        let cap = caps.get(&root_id).ok_or(RepositoryError::CorruptData)?;
        let parent = cap.parent(&relative, false)?;
        let mut opened = fs::file(&parent.dir, &parent.name)?;
        let stamp = fs::stamp(&mut opened)?;
        if indexed
            && (stamp.len != f.size_bytes
                || f.modified_at.is_some_and(|t| {
                    t.timestamp_millis() as i128 != (stamp.modified / 1_000_000) as i128
                }))
        {
            return Err(RepositoryError::Conflict);
        }
        let owner = if is_media(&relative) {
            &f
        } else {
            attachment_owner(&relative, root_id, &playable).unwrap_or(&f)
        };
        let output = render(&input.template, &item, owner)?;
        let target_relative = if is_media(&relative) {
            output
        } else {
            attachment_target(&output, &relative, &owner.relative_path)?
        };
        validate_name(&target_relative)?;
        let same = target.id == root_id && target_relative == relative;
        let mut conflict = if ambiguous.contains(&(root_id, relative.clone())) {
            Some("ambiguous_shared_attachment".into())
        } else {
            None
        };
        if foreign.contains(&(root_id.to_string(), relative.clone())) {
            conflict = Some("attachment_owned_by_other_media".into());
        }
        if !targets.insert(target_relative.to_lowercase()) {
            conflict = Some("portable_name_collision".into());
        }
        if !same
            && target_exists(
                caps.get(&target.id).ok_or(RepositoryError::CorruptData)?,
                &target_relative,
            )?
        {
            conflict = Some("target_exists".into());
        }
        let source_root = roots
            .iter()
            .find(|r| r.id == root_id)
            .ok_or(RepositoryError::CorruptData)?;
        if input.strategy == OrganizationStrategy::Hardlink
            && volume(cap)? != volume(caps.get(&target.id).ok_or(RepositoryError::CorruptData)?)?
        {
            conflict = Some("hardlink_cross_volume".into());
        }
        let role = role(&relative).to_owned();
        views.push(OrganizationFile {
            source: Path::new(&source_root.path)
                .join(&relative)
                .to_string_lossy()
                .into_owned(),
            target: Path::new(&target.path)
                .join(&target_relative)
                .to_string_lossy()
                .into_owned(),
            role: role.clone(),
            bytes: stamp.len,
            conflict,
            same_path: same,
        });
        held.insert(index, opened);
        entries.push(Entry {
            file: f,
            target: target_relative,
            target_id: Uuid::new_v4(),
            stamp,
            role,
            indexed,
        });
    }
    if input.strategy == OrganizationStrategy::Hardlink {
        warnings.push("Hardlinks share bytes with the source; edits affect both names. Unsupported filesystems fail without fallback.".into());
    }
    warnings.push("Quarantines are retained for explicit recovery; organization never overwrites existing files.".into());
    let mut directory_map = BTreeMap::new();
    let mut directory_leases = vec![];
    for e in &entries {
        for (root_id, relative) in [
            (e.file.root_id, &e.file.relative_path),
            (target.id, &e.target),
        ] {
            let (states, held) = caps[&root_id].directory_states(relative)?;
            directory_leases.extend(held);
            for (path, identity) in states {
                directory_map.insert(
                    format!("{root_id}:{path}"),
                    DirectoryStamp {
                        root_id,
                        relative: path,
                        identity,
                    },
                );
            }
        }
    }
    let view = OrganizationPlan {
        id: Uuid::new_v4(),
        media_item_id: item.id,
        target_root_id: target.id,
        strategy: input.strategy,
        state: "preview".into(),
        estimated_bytes: if matches!(
            input.strategy,
            OrganizationStrategy::Copy | OrganizationStrategy::Move
        ) {
            views.iter().filter(|v| !v.same_path).map(|v| v.bytes).sum()
        } else {
            0
        },
        file_count: views.len(),
        files: views,
        warnings,
        kind: "organize".into(),
    };
    let plan = Plan {
        rollback_files: None,
        directories: directory_map.into_values().collect(),
        view: view.clone(),
        item,
        roots,
        target_root: target,
        entries,
        inventory,
        lane,
        original: None,
    };
    Ok((
        plan,
        Session {
            _directories: directory_leases,
            roots: caps,
            sources: std::sync::Mutex::new(held),
            created: Instant::now(),
            cancel: Arc::new(AtomicBool::new(false)),
            access,
            finished: AtomicBool::new(false),
            completion: tokio::sync::Notify::new(),
        },
    ))
}
pub async fn release_revoked_adult_sessions() {
    sessions()
        .lock()
        .await
        .retain(|_, s| s.access.as_ref().is_none_or(|a| a.ensure().is_ok()));
}
fn current_parent(cap: &RootCap, relative: &str, parent: &cap_std::fs::Dir) -> Result<bool> {
    cap.recheck()?;
    let current = cap.parent(relative, false)?;
    Ok(fs::identity(&current.dir.dir_metadata().map_err(fs::err)?)?
        == fs::identity(&parent.dir_metadata().map_err(fs::err)?)?)
}
fn check_source_parent(
    plan: &Plan,
    root_id: Uuid,
    relative: &str,
    parent: &cap_std::fs::Dir,
) -> Result<()> {
    let path = Path::new(relative)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .replace('\\', "/");
    if path.is_empty() {
        return Ok(());
    }
    let expected = plan
        .directories
        .iter()
        .find(|d| d.root_id == root_id && d.relative == path)
        .and_then(|d| d.identity.as_ref())
        .ok_or(RepositoryError::Conflict)?;
    if &fs::identity(&parent.dir_metadata().map_err(fs::err)?)? != expected {
        return Err(RepositoryError::Conflict);
    }
    Ok(())
}
fn check_directories(
    plan: &Plan,
    caps: &HashMap<Uuid, RootCap>,
    include_missing: bool,
) -> Result<()> {
    for d in &plan.directories {
        if d.identity.is_none() && !include_missing {
            continue;
        }
        let probe = format!("{}/__mediaatlas_probe__", d.relative);
        let (states, _held) = caps[&d.root_id].directory_states(&probe)?;
        let current = states
            .into_iter()
            .find(|(p, _)| p == &d.relative)
            .and_then(|(_, id)| id);
        if current != d.identity {
            return Err(RepositoryError::Conflict);
        }
    }
    Ok(())
}
fn copy_stream(source: &mut File, output: &mut File) -> Result<()> {
    use std::io::{Read, Write};
    let mut buffer = [0u8; 65536];
    loop {
        fs::cancellation()?;
        let count = source.read(&mut buffer).map_err(fs::err)?;
        if count == 0 {
            break;
        }
        #[cfg(test)]
        tests::copy_chunk();
        output.write_all(&buffer[..count]).map_err(fs::err)?;
    }
    Ok(())
}
fn check_session(s: &Session) -> Result<()> {
    if s.created.elapsed() >= Duration::from_secs(600) || s.cancel.load(Ordering::SeqCst) {
        return Err(RepositoryError::Conflict);
    }
    if let Some(a) = &s.access {
        a.ensure()?;
    }
    Ok(())
}
fn is_media(relative: &str) -> bool {
    matches!(
        Path::new(relative)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "mkv"
            | "mp4"
            | "avi"
            | "mov"
            | "wmv"
            | "m4v"
            | "webm"
            | "ts"
            | "m2ts"
            | "mpg"
            | "mpeg"
            | "iso"
    )
}
fn role(relative: &str) -> &'static str {
    if is_media(relative) {
        "media"
    } else {
        match Path::new(relative)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "nfo" => "nfo",
            "srt" | "ass" | "ssa" | "sub" | "idx" | "vtt" => "subtitle",
            "png" | "jpg" | "jpeg" | "webp" => "artwork",
            _ => "attachment",
        }
    }
}
fn same_parent(a: &str, b: &str) -> bool {
    Path::new(a).parent() == Path::new(b).parent()
}
fn attachment_owner<'a>(
    relative: &str,
    root: Uuid,
    media: &[&'a MediaFile],
) -> Option<&'a MediaFile> {
    let name = Path::new(relative).file_name()?.to_str()?;
    media
        .iter()
        .copied()
        .filter(|f| f.root_id == root && same_parent(relative, &f.relative_path))
        .find(|f| {
            let stem = Path::new(&f.relative_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            name.strip_prefix(stem).is_some_and(|suffix| {
                suffix.starts_with('.')
                    || matches!(
                        suffix.to_ascii_lowercase().as_str(),
                        "-poster.jpg"
                            | "-poster.png"
                            | "-fanart.jpg"
                            | "-fanart.png"
                            | "-thumb.jpg"
                            | "-thumb.png"
                    )
            })
        })
        .or_else(|| {
            media
                .iter()
                .copied()
                .find(|f| f.root_id == root && same_parent(relative, &f.relative_path))
        })
}
fn associated(name: &str, stems: &[String]) -> bool {
    let lower = name.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "movie.nfo"
            | "tvshow.nfo"
            | "season.nfo"
            | "poster.jpg"
            | "poster.png"
            | "fanart.jpg"
            | "fanart.png"
            | "folder.jpg"
            | "folder.png"
            | "banner.jpg"
            | "banner.png"
            | "clearlogo.png"
            | "landscape.jpg"
    ) {
        return true;
    }
    stems.iter().any(|stem| {
        name.strip_prefix(stem).is_some_and(|suffix| {
            suffix.starts_with('.')
                || matches!(
                    suffix.to_ascii_lowercase().as_str(),
                    "-poster.jpg"
                        | "-poster.png"
                        | "-fanart.jpg"
                        | "-fanart.png"
                        | "-thumb.jpg"
                        | "-thumb.png"
                )
        })
    })
}
fn discover(
    files: &[MediaFile],
    caps: &HashMap<Uuid, RootCap>,
    all: &mut Vec<(Uuid, String, MediaFile, bool)>,
) -> Result<Discovery> {
    let mut inventory = BTreeMap::new();
    let mut ambiguous = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut paths = files
        .iter()
        .map(|f| (f.root_id, f.relative_path.clone()))
        .collect::<BTreeSet<_>>();
    for f in files.iter().filter(|f| is_media(&f.relative_path)) {
        let parent = caps[&f.root_id].parent(&f.relative_path, false)?;
        let key = format!(
            "{}:{}",
            f.root_id,
            Path::new(&f.relative_path)
                .parent()
                .unwrap_or(Path::new(""))
                .display()
        );
        if !seen.insert(key.clone()) {
            continue;
        }
        let stems = files
            .iter()
            .filter(|other| {
                other.root_id == f.root_id
                    && is_media(&other.relative_path)
                    && same_parent(&f.relative_path, &other.relative_path)
            })
            .filter_map(|other| {
                Path::new(&other.relative_path)
                    .file_stem()?
                    .to_str()
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();
        let mut unindexed_generic = vec![];
        let mut foreign_media = false;
        let mut names = vec![];
        for entry in parent.dir.entries().map_err(fs::err)? {
            fs::cancellation()?;
            let entry = entry.map_err(fs::err)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| RepositoryError::InvalidInput)?;
            if name.starts_with(".mediaatlas-") || name.starts_with(".mediaatlas_") {
                continue;
            }
            names.push(name.clone());
            if names.len() > 16384 {
                return Err(RepositoryError::InvalidInput);
            }
            let candidate_relative = Path::new(&f.relative_path)
                .parent()
                .unwrap_or(Path::new(""))
                .join(&name)
                .to_string_lossy()
                .replace('\\', "/");
            if is_media(&name) && !paths.contains(&(f.root_id, candidate_relative)) {
                foreign_media = true;
            }
            if !entry.file_type().map_err(fs::err)?.is_file()
                || is_media(&name)
                || !associated(&name, &stems)
            {
                continue;
            }
            let relative = Path::new(&f.relative_path)
                .parent()
                .unwrap_or(Path::new(""))
                .join(&name)
                .to_str()
                .ok_or(RepositoryError::InvalidInput)?
                .replace('\\', "/");
            if paths.insert((f.root_id, relative.clone())) {
                if !stems.iter().any(|stem| {
                    name.strip_prefix(stem)
                        .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('-'))
                }) {
                    unindexed_generic.push(relative.clone());
                }
                let mut added = f.clone();
                added.id = Uuid::new_v4();
                added.relative_path = relative.clone();
                added.part = None;
                let p = caps[&f.root_id].parent(&relative, false)?;
                let stamp =
                    fs::optional_stamp(&p.dir, &p.name)?.ok_or(RepositoryError::Conflict)?;
                added.size_bytes = stamp.len;
                added.modified_at =
                    chrono::DateTime::from_timestamp_millis((stamp.modified / 1_000_000) as i64);
                all.push((f.root_id, relative, added, false));
            }
        }
        if foreign_media {
            for relative in unindexed_generic {
                ambiguous.insert((f.root_id, relative));
            }
        }
        names.sort();
        inventory.insert(key, names);
    }
    Ok((inventory, ambiguous))
}
fn render(template: &str, item: &MediaItem, file: &MediaFile) -> Result<String> {
    let path = Path::new(&file.relative_path);
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or(RepositoryError::InvalidInput)?;
    let result = template
        .replace("{title}", &item.title)
        .replace(
            "{year}",
            &item.year.map(|v| v.to_string()).unwrap_or_default(),
        )
        .replace(
            "{part}",
            &file.part.map(|p| format!(".part{p}")).unwrap_or_default(),
        )
        .replace("{stem}", stem)
        .replace("{ext}", ext);
    if result.contains(['{', '}']) {
        return Err(RepositoryError::InvalidInput);
    }
    validate_name(&result)?;
    Ok(result)
}
fn attachment_target(output: &str, relative: &str, owner: &str) -> Result<String> {
    let output_path = Path::new(output);
    let name = Path::new(relative)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(RepositoryError::InvalidInput)?;
    let owner_stem = Path::new(owner)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or(RepositoryError::InvalidInput)?;
    let output_stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or(RepositoryError::InvalidInput)?;
    let new_name = if let Some(suffix) = name.strip_prefix(owner_stem) {
        format!("{output_stem}{suffix}")
    } else {
        name.to_owned()
    };
    Ok(output_path
        .parent()
        .unwrap_or(Path::new(""))
        .join(new_name)
        .to_str()
        .ok_or(RepositoryError::InvalidInput)?
        .replace('\\', "/"))
}
fn validate_name(relative: &str) -> Result<()> {
    fs::checked(relative)?;
    if relative.len() > 2048 {
        return Err(RepositoryError::InvalidInput);
    }
    for part in relative.split('/') {
        if part.len() > 240
            || part.ends_with([' ', '.'])
            || part
                .chars()
                .any(|c| c.is_control() || "<>:\"|?*".contains(c))
        {
            return Err(RepositoryError::InvalidInput);
        }
        let base = part
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end_matches(' ')
            .to_ascii_uppercase();
        if matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
            || (base.len() == 4
                && (base.starts_with("COM") || base.starts_with("LPT"))
                && matches!(base.as_bytes()[3], b'1'..=b'9'))
            || matches!(
                base.as_str(),
                "COM¹" | "COM²" | "COM³" | "LPT¹" | "LPT²" | "LPT³"
            )
        {
            return Err(RepositoryError::InvalidInput);
        }
    }
    Ok(())
}
fn target_exists(cap: &RootCap, relative: &str) -> Result<bool> {
    let mut current = cap.root.try_clone().map_err(fs::err)?;
    let parts = relative.split('/').collect::<Vec<_>>();
    for (index, part) in parts.iter().enumerate() {
        let mut actual = None;
        for entry in current.entries().map_err(fs::err)? {
            let entry = entry.map_err(fs::err)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| RepositoryError::Conflict)?;
            if name.trim_end_matches([' ', '.']).to_lowercase() == part.to_lowercase() {
                actual = Some(name);
                break;
            }
        }
        let Some(actual) = actual else {
            return Ok(false);
        };
        if index == parts.len() - 1 {
            return Ok(true);
        }
        // A differently cased ancestor would be distinct on Linux and identical on
        // Windows. Reject it as a portable collision instead of choosing silently.
        if actual != *part {
            return Ok(true);
        }
        current = fs::directory(&current, &actual)?;
    }
    Ok(false)
}
fn volume(cap: &RootCap) -> Result<String> {
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        Ok(cap.root.dir_metadata().map_err(fs::err)?.dev().to_string())
    }
    #[cfg(windows)]
    {
        use cap_fs_ext::MetadataExt;
        Ok(format!(
            "{:?}",
            cap.root.dir_metadata().map_err(fs::err)?.dev()
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = cap;
        Err(RepositoryError::InvalidInput)
    }
}
async fn relocate_references(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    media_id: Uuid,
    before: &str,
    after: &str,
) -> Result<()> {
    for table in ["artworks", "metadata_values"] {
        sqlx::query(&format!(
            "UPDATE {table} SET source_url=? WHERE media_item_id=? AND source_url=?"
        ))
        .bind(after)
        .bind(media_id.to_string())
        .bind(before)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query("UPDATE local_metadata_provenance SET source_reference=? WHERE media_item_id=? AND source_reference=?").bind(after).bind(media_id.to_string()).bind(before).execute(&mut **tx).await?;
    Ok(())
}
async fn journal_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: Uuid,
    index: usize,
    stage: &str,
    stamp: Option<&Stamp>,
) -> Result<()> {
    sqlx::query("INSERT INTO organization_journal(operation_id,file_index,stage,stamp_json,created_at) VALUES(?,?,?,?,?)").bind(id.to_string()).bind(index as i64).bind(stage).bind(stamp.map(serde_json::to_string).transpose().map_err(|_|RepositoryError::CorruptData)?).bind(Utc::now().to_rfc3339()).execute(&mut **tx).await?;
    Ok(())
}
async fn validate_roots_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    plan: &Plan,
    adult: bool,
) -> Result<()> {
    for root in plan.roots.iter().chain(std::iter::once(&plan.target_root)) {
        let n:i64=sqlx::query_scalar("SELECT count(*) FROM library_roots WHERE id=? AND name=? AND path=? AND path_style=? AND kind IS ? AND enabled=1 AND created_at=? AND updated_at=?").bind(root.id.to_string()).bind(&root.name).bind(&root.path).bind(if root.path_style==mediaatlas_domain::PathStyle::Windows{"windows"}else{"posix"}).bind(root.kind.map(|k|k.as_str())).bind(root.created_at.timestamp_millis()).bind(root.updated_at.timestamp_millis()).fetch_one(&mut **tx).await?;
        if n != 1 || ((root.kind == Some(MediaKind::AdultVideo)) != adult) {
            return Err(RepositoryError::Conflict);
        }
    }
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM media_items WHERE id=? AND kind=? AND adult_isolated=?",
    )
    .bind(plan.item.id.to_string())
    .bind(plan.item.kind.as_str())
    .bind(adult)
    .fetch_one(&mut **tx)
    .await?;
    if n != 1 {
        return Err(RepositoryError::Conflict);
    }
    Ok(())
}
async fn validate_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    plan: &Plan,
    adult: bool,
) -> Result<()> {
    validate_roots_tx(tx, plan, adult).await?;
    let n:i64=sqlx::query_scalar("SELECT count(*) FROM media_items WHERE id=? AND title=? AND original_title IS ? AND year IS ? AND updated_at=?").bind(plan.item.id.to_string()).bind(&plan.item.title).bind(&plan.item.original_title).bind(plan.item.year).bind(plan.item.updated_at.timestamp_millis()).fetch_one(&mut **tx).await?;
    if n != 1 {
        return Err(RepositoryError::Conflict);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM media_files WHERE media_item_id=?")
        .bind(plan.item.id.to_string())
        .fetch_one(&mut **tx)
        .await?;
    if count != plan.entries.iter().filter(|e| e.indexed).count() as i64 {
        return Err(RepositoryError::Conflict);
    }
    for e in plan.entries.iter().filter(|e| !e.indexed) {
        let owner: Option<String> = sqlx::query_scalar(
            "SELECT media_item_id FROM media_files WHERE root_id=? AND relative_path=?",
        )
        .bind(e.file.root_id.to_string())
        .bind(&e.file.relative_path)
        .fetch_optional(&mut **tx)
        .await?;
        if owner.is_some() {
            return Err(RepositoryError::Conflict);
        }
    }
    for e in plan.entries.iter().filter(|e| e.indexed) {
        let f = &e.file;
        let n:i64=sqlx::query_scalar("SELECT count(*) FROM media_files WHERE id=? AND media_item_id=? AND root_id=? AND relative_path=? AND size_bytes=? AND modified_at IS ? AND part IS ?").bind(f.id.to_string()).bind(f.media_item_id.to_string()).bind(f.root_id.to_string()).bind(&f.relative_path).bind(f.size_bytes as i64).bind(f.modified_at.map(|v|v.timestamp_millis())).bind(f.part.map(i64::from)).fetch_one(&mut **tx).await?;
        if n != 1 {
            return Err(RepositoryError::Conflict);
        }
    }
    Ok(())
}
async fn validate_rollback_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    plan: &Plan,
    adult: bool,
) -> Result<()> {
    validate_roots_tx(tx, plan, adult).await?;
    let n:i64=sqlx::query_scalar("SELECT count(*) FROM media_items WHERE id=? AND title=? AND original_title IS ? AND year IS ? AND updated_at=?").bind(plan.item.id.to_string()).bind(&plan.item.title).bind(&plan.item.original_title).bind(plan.item.year).bind(plan.item.updated_at.timestamp_millis()).fetch_one(&mut **tx).await?;
    if n != 1 {
        return Err(RepositoryError::Conflict);
    }
    let files = plan
        .rollback_files
        .as_ref()
        .ok_or(RepositoryError::CorruptData)?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM media_files WHERE media_item_id=?")
        .bind(plan.item.id.to_string())
        .fetch_one(&mut **tx)
        .await?;
    if count != files.len() as i64 {
        return Err(RepositoryError::Conflict);
    }
    for f in files {
        let n:i64=sqlx::query_scalar("SELECT count(*) FROM media_files WHERE id=? AND media_item_id=? AND root_id=? AND relative_path=? AND size_bytes=? AND modified_at IS ? AND part IS ?").bind(f.id.to_string()).bind(f.media_item_id.to_string()).bind(f.root_id.to_string()).bind(&f.relative_path).bind(f.size_bytes as i64).bind(f.modified_at.map(|v|v.timestamp_millis())).bind(f.part.map(i64::from)).fetch_one(&mut **tx).await?;
        if n != 1 {
            return Err(RepositoryError::Conflict);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
