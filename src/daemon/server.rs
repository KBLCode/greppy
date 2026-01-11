use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::project::{Project, ProjectEntry, Registry};
use crate::daemon::cache::QueryCache;
use crate::daemon::protocol::{Method, ProjectInfo, Request, Response, ResponseResult};
use crate::index::{IndexSearcher, IndexWriter, TantivyIndex};
use crate::parse::{chunk_file, walk_project};
use crate::search::SearchResponse;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

pub struct DaemonState {
    pub registry: RwLock<Registry>,
    pub searchers: RwLock<HashMap<String, IndexSearcher>>,
    pub cache: RwLock<QueryCache>,
    pub shutdown: broadcast::Sender<()>,
}

impl DaemonState {
    pub fn new() -> Self {
        let (shutdown, _) = broadcast::channel(1);
        Self {
            registry: RwLock::new(Registry::load().unwrap_or_default()),
            searchers: RwLock::new(HashMap::new()),
            cache: RwLock::new(QueryCache::new()),
            shutdown,
        }
    }
}

/// Run the daemon server
pub async fn run_server() -> Result<()> {
    let socket_path = Config::socket_path()?;

    // Remove old socket if exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    let state = Arc::new(DaemonState::new());

    let mut shutdown_rx = state.shutdown.subscribe();

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

async fn handle_connection(stream: UnixStream, state: Arc<DaemonState>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
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
            // TODO: Generate embeddings in daemon mode too
            writer.add_chunk(&chunk, None)?;
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
    // First index
    let result = handle_index(project_path, false, state).await;

    // Mark as watching
    {
        let path = PathBuf::from(project_path);
        let mut registry = state.registry.write();
        registry.set_watching(&path, true);
        let _ = registry.save();
    }

    // TODO: Start file watcher

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
