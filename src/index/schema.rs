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
        }
    }
}

impl Default for IndexSchema {
    fn default() -> Self {
        Self::new()
    }
}
