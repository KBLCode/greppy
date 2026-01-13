use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::project::{Project, ProjectEntry, Registry};
use crate::daemon::cache::QueryCache;
use crate::daemon::protocol::{Method, ProjectInfo, Request, Response, ResponseResult};
use crate::daemon::watcher::WatcherManager;
use crate::index::{IndexSearcher, IndexWriter, TantivyIndex};
use crate::parse::{chunk_file, walk_project};
use crate::search::SearchResponse;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

#[cfg(unix)]
use tokio::net::UnixListener;

#[cfg(windows)]
use tokio::net::TcpListener;

pub struct DaemonState {
    pub registry: RwLock<Registry>,
    pub searchers: RwLock<HashMap<String, IndexSearcher>>,
    pub cache: RwLock<QueryCache>,
    pub watcher: Mutex<WatcherManager>,
    pub shutdown: broadcast::Sender<()>,
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

impl DaemonState {
    pub fn new() -> Self {
        let (shutdown, _) = broadcast::channel(1);
        Self {
            registry: RwLock::new(Registry::load().unwrap_or_default()),
            searchers: RwLock::new(HashMap::new()),
            cache: RwLock::new(QueryCache::new()),
            watcher: Mutex::new(WatcherManager::new()),
            shutdown,
        }
    }

    /// Invalidate searcher cache for a project (called after incremental update)
    pub fn invalidate_project(&self, project_path: &PathBuf) {
        let path_str = project_path.to_string_lossy().to_string();

        // Remove cached searcher so it gets reloaded on next search
        {
            let mut searchers = self.searchers.write();
            searchers.remove(&path_str);
        }

        // Clear query cache for this project
        {
            let mut cache = self.cache.write();
            cache.clear_project(&path_str);
        }

        debug!(project = %path_str, "Invalidated caches after incremental update");
    }
}

/// Background watcher loop - runs independently, doesn't block requests
async fn run_watcher_loop(state: Arc<DaemonState>) {
    info!("Starting file watcher for incremental indexing");

    // Start watching all previously registered projects
    {
        let registry = state.registry.read();
        let mut watcher = state.watcher.lock();
        for entry in registry.list() {
            if entry.watching {
                if let Err(e) = watcher.watch(entry.path.clone()) {
                    warn!(project = %entry.path.display(), error = %e, "Failed to watch project");
                } else {
                    info!(project = %entry.path.display(), "Watching for changes");
                }
            }
        }
    }

    // Process events loop - use blocking task to avoid Send issues with MutexGuard
    loop {
        // Clone state for the blocking task
        let state_clone = Arc::clone(&state);

        // Process events in a blocking task (parking_lot mutex is not Send across await)
        let updated_projects = tokio::task::spawn_blocking(move || {
            let mut watcher = state_clone.watcher.lock();
            watcher.process_events_sync()
        })
        .await
        .unwrap_or_default();

        // Invalidate caches for updated projects
        for project_path in updated_projects {
            state.invalidate_project(&project_path);
        }

        // Sleep to prevent busy-waiting - watcher debounces internally
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Run the daemon server (Unix: Unix sockets)
#[cfg(unix)]
pub async fn run_server() -> Result<()> {
    let socket_path = Config::socket_path()?;

    // Remove old socket if exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    let state = Arc::new(DaemonState::new());

    info!("Daemon starting...");

    // Start watcher background task - processes file changes without blocking requests
    let watcher_state = Arc::clone(&state);
    tokio::spawn(async move {
        run_watcher_loop(watcher_state).await;
    });

    let mut shutdown_rx = state.shutdown.subscribe();

    info!("Daemon ready, listening for connections");

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let state = Arc::clone(&state);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, state).await {
                                eprintln!("Connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Accept error: {}", e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }

    // Cleanup
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    Ok(())
}

/// Run the daemon server (Windows: TCP on localhost)
#[cfg(windows)]
pub async fn run_server() -> Result<()> {
    let port = Config::daemon_port();
    let addr = format!("127.0.0.1:{}", port);

    let listener =
        TcpListener::bind(&addr)
            .await
            .map_err(|e| crate::core::error::Error::DaemonError {
                message: format!("Failed to bind to {}: {}", addr, e),
            })?;

    // Write port to file so clients know which port to connect to
    let port_path = Config::port_path()?;
    std::fs::write(&port_path, port.to_string())?;

    let state = Arc::new(DaemonState::new());

    info!("Daemon starting...");

    // Start watcher background task - processes file changes without blocking requests
    let watcher_state = Arc::clone(&state);
    tokio::spawn(async move {
        run_watcher_loop(watcher_state).await;
    });

    let mut shutdown_rx = state.shutdown.subscribe();

    info!("Daemon ready, listening on {}", addr);

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let state = Arc::clone(&state);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, state).await {
                                eprintln!("Connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Accept error: {}", e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }

    // Cleanup: remove port file
    if port_path.exists() {
        let _ = std::fs::remove_file(&port_path);
    }

    Ok(())
}

/// Handle a connection from any stream type
async fn handle_connection<S>(stream: S, state: Arc<DaemonState>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let response = Response {
                    id: "error".to_string(),
                    result: ResponseResult::Error {
                        message: e.to_string(),
                    },
                };
                let json = serde_json::to_string(&response)? + "\n";
                writer.write_all(json.as_bytes()).await?;
                line.clear();
                continue;
            }
        };

        let response = handle_request(request, &state).await;
        let json = serde_json::to_string(&response)? + "\n";
        writer.write_all(json.as_bytes()).await?;

        // Check for stop command
        if matches!(response.result, ResponseResult::Stop { success: true }) {
            let _ = state.shutdown.send(());
            break;
        }

        line.clear();
    }

    Ok(())
}

async fn handle_request(request: Request, state: &DaemonState) -> Response {
    let result = match request.method {
        Method::Search {
            query,
            project,
            limit,
        } => handle_search(&query, &project, limit, state).await,

        Method::Index { project, force } => handle_index(&project, force, state).await,

        Method::IndexWatch { project } => handle_index_watch(&project, state).await,

        Method::Status => handle_status(state),

        Method::List => handle_list(state),

        Method::Forget { project } => handle_forget(&project, state).await,

        Method::Stop => ResponseResult::Stop { success: true },
    };

    Response {
        id: request.id,
        result,
    }
}

async fn handle_search(
    query: &str,
    project_path: &str,
    limit: usize,
    state: &DaemonState,
) -> ResponseResult {
    let start = Instant::now();
    let path = PathBuf::from(project_path);

    // Check cache
    let cache_key = format!("{}:{}:{}", project_path, query, limit);
    {
        let mut cache = state.cache.write();
        if let Some(cached) = cache.get(&cache_key) {
            return ResponseResult::Search(cached.clone());
        }
    }

    // Get or create searcher
    let searcher = {
        let searchers = state.searchers.read();
        searchers.get(project_path).cloned()
    };

    let searcher = match searcher {
        Some(s) => s,
        None => {
            // Try to open existing index
            match IndexSearcher::open(&path) {
                Ok(s) => {
                    let mut searchers = state.searchers.write();
                    searchers.insert(project_path.to_string(), s.clone());
                    s
                }
                Err(_) => {
                    // Need to index first
                    match do_index(&path, false, state).await {
                        Ok(_) => match IndexSearcher::open(&path) {
                            Ok(s) => {
                                let mut searchers = state.searchers.write();
                                searchers.insert(project_path.to_string(), s.clone());
                                s
                            }
                            Err(e) => {
                                return ResponseResult::Error {
                                    message: e.to_string(),
                                }
                            }
                        },
                        Err(e) => {
                            return ResponseResult::Error {
                                message: e.to_string(),
                            }
                        }
                    }
                }
            }
        }
    };

    // Search
    match searcher.search(query, limit) {
        Ok(results) => {
            let elapsed = start.elapsed();
            let response = SearchResponse {
                results,
                query: query.to_string(),
                elapsed_ms: elapsed.as_secs_f64() * 1000.0,
                project: project_path.to_string(),
            };

            // Cache result
            {
                let mut cache = state.cache.write();
                cache.put(cache_key, response.clone());
            }

            ResponseResult::Search(response)
        }
        Err(e) => ResponseResult::Error {
            message: e.to_string(),
        },
    }
}

async fn handle_index(project_path: &str, force: bool, state: &DaemonState) -> ResponseResult {
    let path = PathBuf::from(project_path);
    match do_index(&path, force, state).await {
        Ok((file_count, chunk_count, elapsed_ms)) => ResponseResult::Index {
            project: project_path.to_string(),
            file_count,
            chunk_count,
            elapsed_ms,
        },
        Err(e) => ResponseResult::Error {
            message: e.to_string(),
        },
    }
}

async fn do_index(
    path: &PathBuf,
    _force: bool,
    state: &DaemonState,
) -> Result<(usize, usize, f64)> {
    let start = Instant::now();

    // Walk and chunk files
    let files = walk_project(path)?;
    let file_count = files.len();

    let index = TantivyIndex::open_or_create(path)?;
    let mut writer = IndexWriter::new(&index)?;
    let mut chunk_count = 0;

    for file in &files {
        let chunks = chunk_file(&file.path, &file.content);
        for chunk in chunks {
            writer.add_chunk(&chunk)?;
            chunk_count += 1;
        }
    }

    writer.commit()?;

    let elapsed = start.elapsed();

    // Update registry
    {
        let project = Project::from_path(path)?;
        let entry = ProjectEntry {
            path: path.clone(),
            name: project.name,
            indexed_at: SystemTime::now(),
            file_count,
            chunk_count,
            watching: false,
        };

        let mut registry = state.registry.write();
        registry.upsert(entry);
        let _ = registry.save();
    }

    // Reload searcher
    {
        let mut searchers = state.searchers.write();
        searchers.remove(&path.to_string_lossy().to_string());
    }

    // Clear cache for this project
    {
        let mut cache = state.cache.write();
        cache.clear_project(&path.to_string_lossy());
    }

    Ok((file_count, chunk_count, elapsed.as_secs_f64() * 1000.0))
}

async fn handle_index_watch(project_path: &str, state: &DaemonState) -> ResponseResult {
    let path = PathBuf::from(project_path);

    // First index
    let result = handle_index(project_path, false, state).await;

    // Start watching this project
    {
        let mut watcher = state.watcher.lock();
        if let Err(e) = watcher.watch(path.clone()) {
            warn!(project = %project_path, error = %e, "Failed to start watcher");
        } else {
            info!(project = %project_path, "Started watching for changes");
        }
    }

    // Mark as watching in registry
    {
        let mut registry = state.registry.write();
        registry.set_watching(&path, true);
        let _ = registry.save();
    }

    result
}

fn handle_status(state: &DaemonState) -> ResponseResult {
    let registry = state.registry.read();
    let projects: Vec<ProjectInfo> = registry
        .list()
        .iter()
        .map(|e| ProjectInfo {
            path: e.path.to_string_lossy().to_string(),
            name: e.name.clone(),
            chunk_count: e.chunk_count,
            watching: e.watching,
        })
        .collect();

    ResponseResult::Status {
        running: true,
        pid: std::process::id(),
        projects,
    }
}

fn handle_list(state: &DaemonState) -> ResponseResult {
    let registry = state.registry.read();
    let projects: Vec<ProjectInfo> = registry
        .list()
        .iter()
        .map(|e| ProjectInfo {
            path: e.path.to_string_lossy().to_string(),
            name: e.name.clone(),
            chunk_count: e.chunk_count,
            watching: e.watching,
        })
        .collect();

    ResponseResult::List { projects }
}

async fn handle_forget(project_path: &str, state: &DaemonState) -> ResponseResult {
    let path = PathBuf::from(project_path);

    // Remove from registry
    {
        let mut registry = state.registry.write();
        registry.remove(&path);
        let _ = registry.save();
    }

    // Remove searcher
    {
        let mut searchers = state.searchers.write();
        searchers.remove(project_path);
    }

    // Delete index directory
    if let Ok(index_dir) = Config::index_dir(&path) {
        if index_dir.exists() {
            let _ = std::fs::remove_dir_all(&index_dir);
        }
    }

    // Clear cache
    {
        let mut cache = state.cache.write();
        cache.clear_project(project_path);
    }

    ResponseResult::Forget {
        project: project_path.to_string(),
        success: true,
    }
}
