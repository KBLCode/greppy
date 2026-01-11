use crate::ai::embedding::Embedder;
use crate::ai::gemini::GeminiClient;
use crate::auth;
use crate::core::error::{Error, Result};
use crate::core::project::Project;
use crate::index::TantivyIndex;
use crate::search::SearchQuery;
use clap::Parser;
use std::env;

/// Arguments for the ask command
#[derive(Parser, Debug)]
pub struct AskArgs {
    /// The question to ask
    pub question: String,

    /// Project path (optional)
    #[arg(short, long)]
    pub project: Option<std::path::PathBuf>,
}

pub async fn run(args: AskArgs) -> Result<()> {
    // 1. Authenticate
    let token = auth::get_token()
        .map_err(|_| Error::Auth(anyhow::anyhow!("Not logged in. Run 'greppy login' first.")))?;

    // 2. Setup Project & Index
    let project_path = args
        .project
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));
    let project = Project::detect(&project_path)?;
    let index = TantivyIndex::open(&project.root)?;

    println!("Analyzing codebase...");

    // 3. Hybrid Search for Context
    // We reuse the search logic but with a higher limit to get good context
    let mut query = SearchQuery::new(&args.question).with_limit(10); // Get top 10 chunks

    // Generate embedding
    if let Ok(embedder) = Embedder::new() {
        if let Ok(embedding) = embedder.embed(&args.question) {
            query = query.with_embedding(embedding);
        }
    }

    let results = query.execute(&index)?;

    if results.results.is_empty() {
        println!("No relevant code found to answer your question.");
        return Ok(());
    }

    // 4. Construct Context
    let mut context = String::new();
    for (i, result) in results.results.iter().enumerate() {
        context.push_str(&format!(
            "Snippet {} ({}:{})\n```{}\n{}\n```\n\n",
            i + 1,
            result.path,
            result.start_line,
            result.language,
            result.content
        ));
    }

    println!("Thinking...");

    // 5. Call LLM
    let client = GeminiClient::new(token);
    let answer = client.ask(&args.question, &context).await?;

    // 6. Output
    println!("\n---\n");
    println!("{}", answer);
    println!("\n---\n");
    println!("Based on {} code snippets.", results.results.len());

    Ok(())
}
