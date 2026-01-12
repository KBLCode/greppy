use crate::ai::embedding::Embedder;
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
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::broadcast;
use tokio::sync::OnceCell;

#[cfg(unix)]
use tokio::net::UnixListener;

#[cfg(windows)]
use tokio::net::TcpListener;

pub struct DaemonState {
    pub registry: RwLock<Registry>,
    pub searchers: RwLock<HashMap<String, IndexSearcher>>,
    pub cache: RwLock<QueryCache>,
    pub shutdown: broadcast::Sender<()>,
    pub embedder: Arc<OnceCell<Arc<Embedder>>>,
}

impl DaemonState {
    pub fn new() -> Self {
        let (shutdown, _) = broadcast::channel(1);
        Self {
            registry: RwLock::new(Registry::load().unwrap_or_default()),
            searchers: RwLock::new(HashMap::new()),
            cache: RwLock::new(QueryCache::new()),
            shutdown,
            embedder: Arc::new(OnceCell::new()),
        }
    }
}

/// Run the daemon server (Unix implementation using Unix sockets)
#[cfg(unix)]
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

/// Run the daemon server (Windows implementation using TCP on localhost)
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
    let mut shutdown_rx = state.shutdown.subscribe();

    println!("Daemon listening on {}", addr);

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

/// Handle a connection from any stream type (Unix socket or TCP)
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

use crossbeam_channel::{bounded, Receiver, Sender};
use std::thread;

async fn do_index(
    path: &PathBuf,
    _force: bool,
    state: &DaemonState,
) -> Result<(usize, usize, f64)> {
    let start = Instant::now();
    let path_clone = path.clone();

    // Get or initialize embedder
    let embedder = state
        .embedder
        .get_or_try_init(|| async {
            println!("Initializing embedding model (once)...");
            let emb = Embedder::new()?;
            println!("Model initialized.");
            Ok::<Arc<Embedder>, anyhow::Error>(Arc::new(emb))
        })
        .await
        .map_err(|e| crate::core::error::Error::DaemonError {
            message: format!("Failed to load embedding model: {}", e),
        })?
        .clone();

    // Offload heavy indexing work to a blocking thread
    let (file_count, chunk_count) = tokio::task::spawn_blocking(move || {
        // Walk and chunk files
        let files = walk_project(&path_clone)?;
        let file_count = files.len();

        let index = TantivyIndex::open_or_create(&path_clone)?;
        let mut writer = IndexWriter::new(&index)?;
        let mut chunk_count = 0;

        // Channels for pipeline
        let (path_tx, path_rx): (
            Sender<crate::parse::walker::FileInfo>,
            Receiver<crate::parse::walker::FileInfo>,
        ) = bounded(1000);
        let (doc_tx, doc_rx): (
            Sender<(crate::parse::Chunk, Vec<f32>)>,
            Receiver<(crate::parse::Chunk, Vec<f32>)>,
        ) = bounded(1000);

        // Spawn Feeder Thread
        let files_to_feed = files;
        let feeder_tx = path_tx.clone();
        thread::spawn(move || {
            for file in files_to_feed {
                let _ = feeder_tx.send(file);
            }
        });

        // Spawn Worker Threads
        let num_workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        for _ in 0..num_workers {
            let p_rx = path_rx.clone();
            let d_tx = doc_tx.clone();
            let emb = Arc::clone(&embedder);

            thread::spawn(move || {
                for file in p_rx {
                    let chunks = chunk_file(&file.path, &file.content);

                    let mut batch_chunks = Vec::new();
                    let mut batch_texts = Vec::new();

                    for chunk in chunks {
                        let text_to_embed = format!(
                            "{}: {}",
                            chunk.symbol_name.as_deref().unwrap_or("code"),
                            chunk.content
                        );

                        batch_chunks.push(chunk);
                        batch_texts.push(text_to_embed);

                        if batch_chunks.len() >= 64 {
                            if let Ok(embeddings) = emb.embed_batch(batch_texts.clone()) {
                                for (c, e) in batch_chunks.drain(..).zip(embeddings) {
                                    let _ = d_tx.send((c, e));
                                }
                            } else {
                                // Fallback if embedding fails - just clear the batch
                                // Chunks without embeddings are skipped (semantic search won't work for them)
                                batch_chunks.clear();
                            }
                            batch_texts.clear();
                        }
                    }

                    // Flush remaining
                    if !batch_chunks.is_empty() {
                        if let Ok(embeddings) = emb.embed_batch(batch_texts) {
                            for (c, e) in batch_chunks.into_iter().zip(embeddings) {
                                let _ = d_tx.send((c, e));
                            }
                        }
                    }
                }
            });
        }

        // Drop original senders to close channel when workers are done
        drop(path_tx);
        drop(doc_tx);

        // Main Thread: Write to Index
        for (chunk, embedding) in doc_rx {
            writer.add_chunk(&chunk, Some(&embedding))?;
            chunk_count += 1;
        }

        writer.commit()?;
        Ok::<(usize, usize), crate::core::error::Error>((file_count, chunk_count))
    })
    .await
    .map_err(|e| crate::core::error::Error::DaemonError {
        message: format!("Indexing task failed: {}", e),
    })??;

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
