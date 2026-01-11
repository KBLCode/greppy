use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, FAST, STORED, STRING,
};

#[derive(Clone)]
pub struct IndexSchema {
    pub schema: Schema,
    pub id: Field,
    pub path: Field,
    pub content: Field,
    pub symbol_name: Field,
    pub symbol_type: Field,
    pub start_line: Field,
    pub end_line: Field,
    pub language: Field,
    pub file_hash: Field,
    pub embedding: Field,
}

impl IndexSchema {
    pub fn new() -> Self {
        let mut builder = Schema::builder();

        // Unique ID: "{path}:{start}:{end}"
        let id = builder.add_text_field("id", STRING | STORED);

        // File path
        let path = builder.add_text_field("path", STRING | STORED);

        // Main content - full text with positions
        let content_opts = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();
        let content = builder.add_text_field("content", content_opts);

        // Symbol name - boosted in search
        let symbol_opts = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqs),
            )
            .set_stored();
        let symbol_name = builder.add_text_field("symbol_name", symbol_opts);

        // Symbol type
        let symbol_type = builder.add_text_field("symbol_type", STRING | STORED);

        // Line numbers
        let start_line = builder.add_u64_field("start_line", FAST | STORED);
        let end_line = builder.add_u64_field("end_line", FAST | STORED);

        // Language
        let language = builder.add_text_field("language", STRING | STORED);

        // File hash for incremental indexing
        let file_hash = builder.add_text_field("file_hash", STRING | STORED);

        // Embedding vector (768 dimensions for BGE-Base-EN)
        // We use a bytes field for now as Tantivy 0.22's vector support is evolving
        // and we want to ensure compatibility.
        // Ideally, this should be `add_vector_field` if we want ANN.
        // Let's try to use `add_bytes_field` for storage and manual cosine similarity if needed,
        // or upgrade to use `add_vector_field` properly.
        // Given the goal is "True Semantic Search", we should use `add_bytes_field` and implement
        // brute-force or use `fastembed`'s utilities if available, OR use Tantivy's vector search.
        // Tantivy 0.22 supports `add_vector_field`.
        // Let's try to switch to `add_vector_field` to enable ANN.
        let _vector_options = TextOptions::default(); // Placeholder, vector options are different
                                                     // Actually, Tantivy 0.22 `SchemaBuilder` has `add_vector_field`.
                                                     // We need to import `VectorOptions`.
                                                     // But since I don't want to break the build with guessing, I'll stick to `bytes` for storage
                                                     // and we can implement a linear scan or use `add_vector_field` if I can confirm the API.
                                                     // Let's stick to `bytes` for now as it is safe and we can do exact NN search which is fine for < 100k chunks.
        let embedding = builder.add_bytes_field("embedding", FAST | STORED);

        Self {
            schema: builder.build(),
            id,
            path,
            content,
            symbol_name,
            symbol_type,
            start_line,
            end_line,
            language,
            file_hash,
            embedding,
        }
    }
}

impl Default for IndexSchema {
    fn default() -> Self {
        Self::new()
    }
}
