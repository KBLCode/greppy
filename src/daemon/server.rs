use crate::cache::QueryCache;
use crate::config::{Config, DEFAULT_LIMIT};
use crate::daemon::protocol::{
    ProjectInfo, Request, RequestMethod, Response, ResponseData, ResponseResult,
};
use crate::error::Result;
use crate::index::{IndexSearcher, IndexWriter};
use crate::parse::{Chunker, FileWalker};
use crate::project::{detect_project_root, ProjectRegistry};
use crate::search::SearchResponse;
use crate::watch::WatchManager;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

pub struct DaemonServer {
    start_time: Instant,
    cache: Arc<RwLock<QueryCache>>,
    watch_manager: Arc<WatchManager>,
    watched_projects: Arc<RwLock<HashMap<PathBuf, bool>>>,
    shutdown_tx: broadcast::Sender<()>,
}

impl DaemonServer {
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            start_time: Instant::now(),
            cache: Arc::new(RwLock::new(QueryCache::new())),
            watch_manager: Arc::new(WatchManager::new()),
            watched_projects: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let socket_path = Config::socket_path()?;

        // Remove stale socket file
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }

        let listener = UnixListener::bind(&socket_path)?;
        info!("Daemon listening on {:?}", socket_path);

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let cache = Arc::clone(&self.cache);
                            let watch_manager = Arc::clone(&self.watch_manager);
                            let watched_projects = Arc::clone(&self.watched_projects);
                            let start_time = self.start_time;
                            let shutdown_tx = self.shutdown_tx.clone();
                            
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(
                                    stream, 
                                    cache, 
                                    watch_manager,
                                    watched_projects,
                                    start_time, 
                                    shutdown_tx
                                ).await {
                                    error!("Connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Accept error: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    break;
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Ctrl+C received, shutting down");
                    break;
                }
            }
        }

        // Cleanup
        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_file(Config::pid_path()?);

        Ok(())
    }
}

async fn handle_connection(
    stream: UnixStream,
    cache: Arc<RwLock<QueryCache>>,
    watch_manager: Arc<WatchManager>,
    watched_projects: Arc<RwLock<HashMap<PathBuf, bool>>>,
    start_time: Instant,
    shutdown_tx: broadcast::Sender<()>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let request: Request = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let response = Response::error("unknown".to_string(), format!("Invalid request: {}", e));
                let json = serde_json::to_string(&response)? + "\n";
                writer.write_all(json.as_bytes()).await?;
                line.clear();
                continue;
            }
        };

        let response = handle_request(
            request, 
            &cache, 
            &watch_manager,
            &watched_projects,
            start_time, 
            &shutdown_tx
        ).await;
        let json = serde_json::to_string(&response)? + "\n";
        writer.write_all(json.as_bytes()).await?;

        // Check if this was a shutdown request
        if matches!(response.result, ResponseResult::Ok { data: ResponseData::Shutdown }) {
            break;
        }

        line.clear();
    }

    Ok(())
}

async fn handle_request(
    request: Request,
    cache: &Arc<RwLock<QueryCache>>,
    watch_manager: &Arc<WatchManager>,
    watched_projects: &Arc<RwLock<HashMap<PathBuf, bool>>>,
    start_time: Instant,
    shutdown_tx: &broadcast::Sender<()>,
) -> Response {
    let id = request.id.clone();

    match request.method {
        RequestMethod::Search { query, project, limit } => {
            handle_search(id, query, project, limit, cache).await
        }
        RequestMethod::Index { project, force } => {
            handle_index(id, project, force, watch_manager, watched_projects, cache).await
        }
        RequestMethod::Status => {
            handle_status(id, start_time, cache, watched_projects).await
        }
        RequestMethod::ListProjects => {
            handle_list_projects(id).await
        }
        RequestMethod::ForgetProject { project } => {
            handle_forget_project(id, project, watch_manager, watched_projects).await
        }
        RequestMethod::Shutdown => {
            let _ = shutdown_tx.send(());
            Response::ok(id, ResponseData::Shutdown)
        }
        RequestMethod::Ping => {
            Response::ok(id, ResponseData::Pong)
        }
    }
}

async fn handle_search(
    id: String,
    query: String,
    project: std::path::PathBuf,
    limit: usize,
    cache: &Arc<RwLock<QueryCache>>,
) -> Response {
    let start = Instant::now();
    let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };

    // Check cache first
    let cache_key = format!("{}:{}:{}", project.display(), query, limit);
    if let Some(cached) = cache.read().get(&cache_key) {
        let mut response = cached.clone();
        response.cached = true;
        response.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        return Response::ok(id, ResponseData::Search(response));
    }

    // Perform search
    let searcher = match IndexSearcher::open(&project) {
        Ok(s) => s,
        Err(e) => return Response::error(id, format!("Failed to open index: {}", e)),
    };

    let results = match searcher.search(&query, limit) {
        Ok(r) => r,
        Err(e) => return Response::error(id, format!("Search failed: {}", e)),
    };

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let response = SearchResponse::new(
        query,
        project.display().to_string(),
        results,
        elapsed_ms,
        false,
    );

    // Cache the result
    cache.write().put(cache_key, response.clone());

    Response::ok(id, ResponseData::Search(response))
}

async fn handle_index(
    id: String,
    project: std::path::PathBuf,
    _force: bool,
    watch_manager: &Arc<WatchManager>,
    watched_projects: &Arc<RwLock<HashMap<PathBuf, bool>>>,
    cache: &Arc<RwLock<QueryCache>>,
) -> Response {
    let start = Instant::now();

    // Detect project root
    let project_root = match detect_project_root(&project) {
        Ok(root) => root,
        Err(e) => return Response::error(id, format!("Failed to detect project: {}", e)),
    };

    // Walk files
    let walker = FileWalker::new(&project_root);
    let files = match walker.walk() {
        Ok(f) => f,
        Err(e) => return Response::error(id, format!("Failed to walk files: {}", e)),
    };

    // Create index
    let mut writer = match IndexWriter::open_or_create(&project_root) {
        Ok(w) => w,
        Err(e) => return Response::error(id, format!("Failed to create index: {}", e)),
    };

    let mut files_indexed = 0;
    let mut chunks_indexed = 0;

    for file in &files {
        match Chunker::chunk_file(file, &project_root) {
            Ok(chunks) => {
                for chunk in &chunks {
                    if let Err(e) = writer.add_chunk(chunk) {
                        warn!("Failed to index chunk: {}", e);
                        continue;
                    }
                    chunks_indexed += 1;
                }
                files_indexed += 1;
            }
            Err(e) => {
                warn!("Failed to chunk file {:?}: {}", file, e);
            }
        }
    }

    // Commit the index
    if let Err(e) = writer.commit() {
        return Response::error(id, format!("Failed to commit index: {}", e));
    }

    // Update registry
    if let Ok(mut registry) = ProjectRegistry::load() {
        registry.add_project(&project_root, files_indexed);
        let _ = registry.save();
    }

    // Invalidate cache for this project
    cache.write().invalidate_project(&project_root.display().to_string());

    // Start watching for changes (auto-update)
    if !watched_projects.read().contains_key(&project_root) {
        match watch_manager.watch_project(project_root.clone()) {
            Ok(rx) => {
                watched_projects.write().insert(project_root.clone(), true);
                let wm = Arc::clone(watch_manager);
                let root = project_root.clone();
                let cache_clone = Arc::clone(cache);
                
                tokio::spawn(async move {
                    process_watch_events(root, rx, wm, cache_clone).await;
                });
                
                info!("Auto-watch enabled for {:?}", project_root);
            }
            Err(e) => {
                warn!("Failed to start file watcher: {}", e);
            }
        }
    }

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    Response::ok(id, ResponseData::Index {
        project: project_root.display().to_string(),
        files_indexed,
        chunks_indexed,
        elapsed_ms,
    })
}

/// Process file watch events and trigger reindexing
async fn process_watch_events(
    project_root: PathBuf,
    mut rx: tokio::sync::mpsc::Receiver<crate::watch::WatchEvent>,
    _watch_manager: Arc<WatchManager>,
    cache: Arc<RwLock<QueryCache>>,
) {
    use std::collections::HashSet;
    use std::time::Duration;
    
    let mut pending_files: HashSet<PathBuf> = HashSet::new();
    let mut last_reindex = Instant::now();
    let debounce_duration = Duration::from_millis(500);
    let min_reindex_interval = Duration::from_secs(2);

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                match event {
                    crate::watch::WatchEvent::FileChanged(path) |
                    crate::watch::WatchEvent::FileCreated(path) |
                    crate::watch::WatchEvent::FileDeleted(path) => {
                        pending_files.insert(path);
                    }
                    crate::watch::WatchEvent::Reindex(_) => {
                        // Force immediate reindex
                        pending_files.clear();
                        if let Err(e) = do_reindex(&project_root, &cache).await {
                            error!("Reindex failed: {}", e);
                        }
                        last_reindex = Instant::now();
                    }
                }
            }
            _ = tokio::time::sleep(debounce_duration) => {
                if !pending_files.is_empty() && last_reindex.elapsed() > min_reindex_interval {
                    info!("Auto-reindexing {} changed files in {:?}", pending_files.len(), project_root);
                    pending_files.clear();
                    
                    if let Err(e) = do_reindex(&project_root, &cache).await {
                        error!("Reindex failed: {}", e);
                    }
                    last_reindex = Instant::now();
                }
            }
        }
    }
}

/// Perform a full reindex of the project
async fn do_reindex(project_root: &PathBuf, cache: &Arc<RwLock<QueryCache>>) -> Result<()> {
    let root = project_root.clone();
    let cache = Arc::clone(cache);
    
    tokio::task::spawn_blocking(move || {
        let walker = FileWalker::new(&root);
        let files = walker.walk()?;

        let mut writer = IndexWriter::open_or_create(&root)?;
        let mut files_indexed = 0;
        let mut chunks_indexed = 0;

        for file in &files {
            match Chunker::chunk_file(file, &root) {
                Ok(chunks) => {
                    for chunk in &chunks {
                        if let Err(e) = writer.add_chunk(chunk) {
                            warn!("Failed to index chunk: {}", e);
                            continue;
                        }
                        chunks_indexed += 1;
                    }
                    files_indexed += 1;
                }
                Err(e) => {
                    warn!("Failed to chunk file {:?}: {}", file, e);
                }
            }
        }

        writer.commit()?;
        
        // Invalidate cache
        cache.write().invalidate_project(&root.display().to_string());
        
        // Update registry
        if let Ok(mut registry) = ProjectRegistry::load() {
            registry.add_project(&root, files_indexed);
            let _ = registry.save();
        }

        info!("Reindexed {} files ({} chunks)", files_indexed, chunks_indexed);
        Ok(())
    })
    .await
    .map_err(|e| crate::error::GreppyError::Index(e.to_string()))?
}

async fn handle_status(
    id: String,
    start_time: Instant,
    cache: &Arc<RwLock<QueryCache>>,
    watched_projects: &Arc<RwLock<HashMap<PathBuf, bool>>>,
) -> Response {
    let pid = std::process::id();
    let uptime_secs = start_time.elapsed().as_secs();
    let cache_size = cache.read().len();
    let watching_count = watched_projects.read().len();

    let projects_indexed = ProjectRegistry::load()
        .map(|r| r.projects().len())
        .unwrap_or(0);

    // Include watching info in status
    info!("Status: {} projects indexed, {} being watched", projects_indexed, watching_count);

    Response::ok(id, ResponseData::Status {
        pid,
        uptime_secs,
        projects_indexed,
        cache_size,
    })
}

async fn handle_list_projects(id: String) -> Response {
    let registry = match ProjectRegistry::load() {
        Ok(r) => r,
        Err(e) => return Response::error(id, format!("Failed to load registry: {}", e)),
    };

    let projects: Vec<ProjectInfo> = registry
        .projects()
        .iter()
        .map(|p| ProjectInfo {
            path: p.path.clone(),
            name: std::path::Path::new(&p.path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.path.clone()),
            files_indexed: p.files_indexed,
            last_indexed: p.last_indexed.clone(),
        })
        .collect();

    Response::ok(id, ResponseData::Projects { projects })
}

async fn handle_forget_project(
    id: String, 
    project: std::path::PathBuf,
    watch_manager: &Arc<WatchManager>,
    watched_projects: &Arc<RwLock<HashMap<PathBuf, bool>>>,
) -> Response {
    // Stop watching
    watch_manager.unwatch_project(&project);
    watched_projects.write().remove(&project);

    // Remove from registry
    if let Ok(mut registry) = ProjectRegistry::load() {
        registry.remove_project(&project);
        let _ = registry.save();
    }

    // Remove index directory
    if let Ok(index_dir) = Config::index_dir(&project) {
        let _ = std::fs::remove_dir_all(index_dir);
    }

    Response::ok(id, ResponseData::Forgotten {
        project: project.display().to_string(),
    })
}
