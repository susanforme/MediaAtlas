//! Native notifications refresh ordinary video roots by default. Explicit opt-out
//! is preserved; isolated scan services still create read-only previews.
use crate::{
    library::{LibraryRepository, RepositoryError},
    scan_service::ScanService,
};
use chrono::Utc;
use mediaatlas_domain::{LibraryRoot, MediaKind};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use uuid::Uuid;

const MAX_WATCHERS: usize = 32;
const QUEUE_CAPACITY: usize = 256;
const DEBOUNCE: Duration = Duration::from_millis(350);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatcherStatus {
    pub root_id: Uuid,
    pub opted_in: bool,
    pub active: bool,
    pub pending_rescan: bool,
    pub last_task_id: Option<Uuid>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Signal {
    root_id: Uuid,
    generation: Uuid,
}
struct LiveWatcher {
    _watcher: RecommendedWatcher,
    root: LibraryRoot,
    signal: Signal,
    root_stamp: RootStamp,
    active: Arc<AtomicBool>,
    overflow: Arc<Mutex<HashSet<Signal>>>,
}
impl Drop for LiveWatcher {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        if let Ok(mut overflow) = self.overflow.lock() {
            overflow.remove(&self.signal);
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct RootStamp {
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}
fn root_stamp(path: &Path) -> Option<RootStamp> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return None;
    }
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Some(RootStamp {
        created: metadata.created().ok(),
        #[cfg(unix)]
        dev: metadata.dev(),
        #[cfg(unix)]
        ino: metadata.ino(),
    })
}
#[derive(Default)]
struct LiveState {
    handles: HashMap<Uuid, LiveWatcher>,
    pending: HashSet<Uuid>,
    errors: HashMap<Uuid, String>,
}
struct Runtime {
    live: Mutex<LiveState>,
    sender: mpsc::Sender<Signal>,
    overflow: Arc<Mutex<HashSet<Signal>>>,
    dispatch: tokio::sync::Mutex<()>,
    stopped: AtomicBool,
    #[cfg(test)]
    native_callbacks_enabled: Arc<AtomicBool>,
}
#[derive(Clone)]
pub struct WatcherService {
    repo: LibraryRepository,
    scans: ScanService,
    runtime: Arc<Runtime>,
}
impl WatcherService {
    /// Create once in application composition and clone for IPC callers. No roots
    /// are watched until restore or explicit opt-in configuration.
    pub fn new(repo: LibraryRepository, scans: ScanService) -> Self {
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let runtime = Arc::new(Runtime {
            live: Mutex::new(LiveState::default()),
            sender,
            overflow: Arc::new(Mutex::new(HashSet::new())),
            dispatch: tokio::sync::Mutex::new(()),
            stopped: AtomicBool::new(false),
            #[cfg(test)]
            native_callbacks_enabled: Arc::new(AtomicBool::new(true)),
        });
        let service = Self {
            repo,
            scans,
            runtime,
        };
        tokio::spawn(worker(
            Arc::downgrade(&service.runtime),
            service.repo.clone(),
            service.scans.clone(),
            receiver,
        ));
        service
    }
    pub async fn restore(&self) -> Result<(), RepositoryError> {
        self.reconcile().await?;
        // A stopped process misses native events. Catch up with a fresh
        // scan for each current watched generation, never replay old operations.
        let live = self
            .runtime
            .live
            .lock()
            .map_err(|_| RepositoryError::Database)?;
        for handle in live.handles.values() {
            if self.runtime.sender.try_send(handle.signal).is_err() {
                record_live_overflow(&self.runtime.overflow, handle.signal, &handle.active);
            }
        }
        Ok(())
    }
    pub async fn configure(
        &self,
        root_id: Uuid,
        enabled: bool,
    ) -> Result<WatcherStatus, RepositoryError> {
        let _dispatch = self.runtime.dispatch.lock().await;
        if self.runtime.stopped.load(Ordering::Acquire) {
            return Err(RepositoryError::Conflict);
        }
        let root = self
            .repo
            .get_root(root_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        if enabled {
            ensure_ordinary(&self.repo, &root).await?;
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM watcher_roots WHERE opted_in=1 AND root_id<>?",
            )
            .bind(root_id.to_string())
            .fetch_one(self.repo.pool())
            .await?;
            if count >= MAX_WATCHERS as i64 {
                return Err(RepositoryError::Conflict);
            }
        }
        sqlx::query("INSERT INTO watcher_roots(root_id,opted_in,updated_at) VALUES(?,?,?) ON CONFLICT(root_id) DO UPDATE SET opted_in=excluded.opted_in,updated_at=excluded.updated_at")
            .bind(root_id.to_string()).bind(enabled).bind(Utc::now().timestamp_millis()).execute(self.repo.pool()).await?;
        reconcile_unlocked(&self.repo, &self.runtime).await?;
        self.list()
            .await?
            .into_iter()
            .find(|s| s.root_id == root_id)
            .ok_or(RepositoryError::NotFound)
    }
    pub async fn list(&self) -> Result<Vec<WatcherStatus>, RepositoryError> {
        let rows = sqlx::query("SELECT r.id AS root_id,coalesce(w.opted_in,r.kind IN ('movie','tv_series','anime'),0) AS opted_in,w.last_task_id FROM library_roots r LEFT JOIN watcher_roots w ON r.id=w.root_id WHERE r.kind IS NOT NULL AND r.kind!='adult_video' AND NOT EXISTS(SELECT 1 FROM media_files f JOIN media_items m ON m.id=f.media_item_id WHERE f.root_id=r.id AND (m.adult_isolated=1 OR m.kind='adult_video')) ORDER BY r.id LIMIT 200").fetch_all(self.repo.pool()).await?;
        let live = self
            .runtime
            .live
            .lock()
            .map_err(|_| RepositoryError::Database)?;
        rows.into_iter()
            .map(|row| {
                let id = Uuid::parse_str(row.try_get("root_id")?)
                    .map_err(|_| RepositoryError::CorruptData)?;
                let last: Option<String> = row.try_get("last_task_id")?;
                Ok(WatcherStatus {
                    root_id: id,
                    opted_in: row.try_get("opted_in")?,
                    active: live.handles.contains_key(&id),
                    pending_rescan: live.pending.contains(&id),
                    last_task_id: last
                        .map(|id| Uuid::parse_str(&id).map_err(|_| RepositoryError::CorruptData))
                        .transpose()?,
                    error_code: live.errors.get(&id).cloned(),
                })
            })
            .collect()
    }
    /// Stop handles for removed/disabled/reclassified roots and replace a handle
    /// when its Root snapshot changes. The worker also reconciles every second.
    pub async fn reconcile(&self) -> Result<(), RepositoryError> {
        reconcile(&self.repo, &self.runtime).await
    }
    pub async fn shutdown(&self) {
        // Acknowledgement waits for an in-flight dispatch. Once returned, no
        // queued event can start a scan, including while SQLite was busy.
        let _dispatch = self.runtime.dispatch.lock().await;
        self.runtime.stopped.store(true, Ordering::Release);
        if let Ok(mut live) = self.runtime.live.lock() {
            live.handles.clear();
            live.pending.clear();
        }
    }
}
async fn ensure_ordinary(
    repo: &LibraryRepository,
    root: &LibraryRoot,
) -> Result<(), RepositoryError> {
    if !root.enabled || root.kind.is_none() || root.kind == Some(MediaKind::AdultVideo) {
        return Err(RepositoryError::InvalidInput);
    }
    let isolated:i64=sqlx::query_scalar("SELECT count(*) FROM media_files f JOIN media_items m ON m.id=f.media_item_id WHERE f.root_id=? AND (m.adult_isolated=1 OR m.kind='adult_video')").bind(root.id.to_string()).fetch_one(repo.pool()).await?;
    if isolated > 0 {
        return Err(RepositoryError::NotFound);
    }
    Ok(())
}
async fn reconcile(repo: &LibraryRepository, runtime: &Runtime) -> Result<(), RepositoryError> {
    let _dispatch = runtime.dispatch.lock().await;
    reconcile_unlocked(repo, runtime).await
}
async fn reconcile_unlocked(
    repo: &LibraryRepository,
    runtime: &Runtime,
) -> Result<(), RepositoryError> {
    if runtime.stopped.load(Ordering::Acquire) {
        return Ok(());
    }
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT r.id FROM library_roots r LEFT JOIN watcher_roots w ON w.root_id=r.id WHERE r.enabled=1 AND r.kind IS NOT NULL AND r.kind<>'adult_video' AND coalesce(w.opted_in,r.kind IN ('movie','tv_series','anime'),0)=1 AND NOT EXISTS(SELECT 1 FROM media_files f JOIN media_items m ON m.id=f.media_item_id WHERE f.root_id=r.id AND (m.adult_isolated=1 OR m.kind='adult_video')) ORDER BY w.opted_in DESC,r.id LIMIT 32",
    )
    .fetch_all(repo.pool())
    .await?;
    let mut desired = HashMap::new();
    for id in ids {
        let id = Uuid::parse_str(&id).map_err(|_| RepositoryError::CorruptData)?;
        if let Some(root) = repo.get_root(id).await? {
            if ensure_ordinary(repo, &root).await.is_ok() {
                desired.insert(id, root);
            }
        }
    }
    let mut live = runtime.live.lock().map_err(|_| RepositoryError::Database)?;
    live.handles.retain(|id, handle| {
        desired.get(id) == Some(&handle.root)
            && root_stamp(Path::new(&handle.root.path)).as_ref() == Some(&handle.root_stamp)
    });
    live.pending.retain(|id| desired.contains_key(id));
    live.errors.retain(|id, _| desired.contains_key(id));
    for (id, root) in desired {
        if live.handles.contains_key(&id) {
            continue;
        }
        let Some(stamp) = root_stamp(Path::new(&root.path)) else {
            live.errors.insert(id, "watch_unavailable".into());
            continue;
        };
        let sender = runtime.sender.clone();
        let overflow = runtime.overflow.clone();
        let signal = Signal {
            root_id: id,
            generation: Uuid::new_v4(),
        };
        let active = Arc::new(AtomicBool::new(true));
        let callback_active = active.clone();
        let callback_overflow = overflow.clone();
        #[cfg(test)]
        let native_callbacks_enabled = runtime.native_callbacks_enabled.clone();
        let created = RecommendedWatcher::new(
            move |result: notify::Result<Event>| {
                #[cfg(test)]
                if !native_callbacks_enabled.load(Ordering::Acquire) {
                    return;
                }
                if !callback_active.load(Ordering::Acquire) {
                    return;
                }
                #[cfg(test)]
                if let Ok(event) = &result {
                    eprintln!("native watch event: kind={:?}, relevant={}", event.kind, event_relevant(event));
                }
                match result {
                    Ok(event) if event_relevant(&event) => {
                        if sender.try_send(signal).is_err() {
                            record_live_overflow(&callback_overflow, signal, &callback_active);
                        }
                    }
                    Err(_) => record_live_overflow(&callback_overflow, signal, &callback_active),
                    _ => {}
                }
            },
            notify::Config::default().with_follow_symlinks(false),
        )
        .and_then(|mut watcher| {
            watcher.watch(Path::new(&root.path), RecursiveMode::Recursive)?;
            if root_stamp(Path::new(&root.path)).as_ref() != Some(&stamp) {
                return Err(notify::Error::generic("Root changed during watcher setup"));
            }
            Ok(watcher)
        });
        match created {
            Ok(watcher) => {
                live.errors.remove(&id);
                live.handles.insert(
                    id,
                    LiveWatcher {
                        _watcher: watcher,
                        root,
                        signal,
                        root_stamp: stamp,
                        active,
                        overflow,
                    },
                );
            }
            Err(_) => {
                active.store(false, Ordering::Release);
                if let Ok(mut pending) = overflow.lock() {
                    pending.remove(&signal);
                }
                live.errors.insert(id, "watch_unavailable".into());
            }
        }
    }
    Ok(())
}
fn record_live_overflow(overflow: &Mutex<HashSet<Signal>>, signal: Signal, active: &AtomicBool) {
    if let Ok(mut roots) = overflow.lock() {
        // Drop stores false before acquiring this same mutex to remove its
        // generation. A retired callback cannot fill or replace a new epoch.
        if active.load(Ordering::Acquire) && roots.len() < QUEUE_CAPACITY {
            roots.insert(signal);
        }
    }
}
#[cfg(test)]
fn enqueue(sender: &mpsc::Sender<Signal>, overflow: &Mutex<HashSet<Signal>>, signal: Signal) {
    if sender.try_send(signal).is_err() {
        record_overflow(overflow, signal);
    }
}
#[cfg(test)]
fn record_overflow(overflow: &Mutex<HashSet<Signal>>, signal: Signal) {
    record_live_overflow(overflow, signal, &AtomicBool::new(true));
}
fn current(live: &LiveState, signal: Signal) -> bool {
    live.handles
        .get(&signal.root_id)
        .is_some_and(|handle| handle.signal == signal)
}
fn event_relevant(event: &Event) -> bool {
    // Access events caused by scanning are not changes. Hidden user content and
    // Git files keep the existing scanner/exclusion semantics.
    !matches!(event.kind, EventKind::Access(_))
        && (event.paths.is_empty() || event.paths.iter().any(|p| !private_path(p)))
}
fn private_path(path: &Path) -> bool {
    path.components().any(|component| {
        component.as_os_str().to_str().is_some_and(|name| {
            let name = name.to_ascii_lowercase();
            name.starts_with(".mediaatlas-") || name.starts_with(".mediaatlas_")
        })
    })
}
async fn worker(
    runtime: Weak<Runtime>,
    repo: LibraryRepository,
    scans: ScanService,
    mut receiver: mpsc::Receiver<Signal>,
) {
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut due = HashMap::<Signal, Instant>::new();
    let mut last_reconcile = Instant::now();
    loop {
        tokio::select! {
            event=receiver.recv()=> { if let Some(id)=event { due.insert(id,Instant::now()+DEBOUNCE); } else { break; } },
            _=tick.tick()=>{}
        }
        let Some(runtime) = runtime.upgrade() else {
            break;
        };
        if runtime.stopped.load(Ordering::Acquire) {
            break;
        }
        if last_reconcile.elapsed() >= Duration::from_secs(1) {
            let _ = reconcile(&repo, &runtime).await;
            last_reconcile = Instant::now();
        }
        if let Ok(mut overflow) = runtime.overflow.lock() {
            for signal in overflow.drain() {
                due.insert(signal, Instant::now() + DEBOUNCE);
            }
        }
        if let Ok(mut live) = runtime.live.lock() {
            due.retain(|signal, _| current(&live, *signal));
            live.pending = due.keys().map(|s| s.root_id).collect();
        }
        let ready: Vec<Signal> = due
            .iter()
            .filter(|(_, when)| **when <= Instant::now())
            .map(|(id, _)| *id)
            .collect();
        for signal in ready {
            let _dispatch = runtime.dispatch.lock().await;
            let id = signal.root_id;
            let expected = runtime.live.lock().ok().and_then(|live| {
                live.handles
                    .get(&id)
                    .filter(|handle| {
                        handle.signal == signal
                            && root_stamp(Path::new(&handle.root.path)).as_ref()
                                == Some(&handle.root_stamp)
                    })
                    .map(|handle| handle.root.clone())
            });
            let Some(expected) = expected else {
                due.remove(&signal);
                continue;
            };
            if runtime.stopped.load(Ordering::Acquire) {
                due.remove(&signal);
                continue;
            }
            // Manual previews wait for confirmation; automatic review results
            // must not block later filesystem changes from refreshing the root.
            let active:Result<i64,_>=sqlx::query_scalar("SELECT count(*) FROM scan_sessions s JOIN scrape_tasks t ON t.id=s.task_id WHERE s.root_id=? AND (t.state IN('queued','running') OR (t.state='waiting_for_user' AND s.mode<>'automatic'))").bind(id.to_string()).fetch_one(repo.pool()).await;
            if !matches!(active, Ok(0)) {
                continue;
            }
            match scans.start_watched(&expected).await {
                Ok(task) => {
                    due.remove(&signal);
                    let _ = sqlx::query(
                        "INSERT INTO watcher_roots(root_id,opted_in,last_task_id,updated_at) VALUES(?,1,?,?) ON CONFLICT(root_id) DO UPDATE SET last_task_id=excluded.last_task_id,updated_at=excluded.updated_at",
                    )
                    .bind(id.to_string())
                    .bind(task.id.to_string())
                    .bind(Utc::now().timestamp_millis())
                    .execute(repo.pool())
                    .await;
                }
                Err(RepositoryError::Conflict) => {}
                Err(_) => {
                    due.remove(&signal);
                    if let Ok(mut live) = runtime.live.lock() {
                        live.errors.insert(id, "watch_scan_failed".into());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
