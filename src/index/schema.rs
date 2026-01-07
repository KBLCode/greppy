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
    // New AST-aware fields
    pub signature: Field,
    pub parent_symbol: Field,
    pub doc_comment: Field,
    pub is_exported: Field,
    pub is_test: Field,
}

impl IndexSchema {
    pub fn new() -> Self {
        let mut builder = Schema::builder();

        let id = builder.add_text_field("id", STRING | STORED);
        let path = builder.add_text_field("path", STRING | STORED);

        let content_opts = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();
        let content = builder.add_text_field("content", content_opts);

        let symbol_opts = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqs),
            )
            .set_stored();
        let symbol_name = builder.add_text_field("symbol_name", symbol_opts.clone());
        let symbol_type = builder.add_text_field("symbol_type", STRING | STORED);

        let start_line = builder.add_u64_field("start_line", FAST | STORED);
        let end_line = builder.add_u64_field("end_line", FAST | STORED);
        let language = builder.add_text_field("language", STRING | STORED);
        let file_hash = builder.add_text_field("file_hash", STRING | STORED);

        // New AST-aware fields
        // Signature is searchable (for finding functions by parameter types)
        let signature = builder.add_text_field("signature", symbol_opts.clone());
        // Parent symbol for hierarchical search (e.g., find methods of a class)
        let parent_symbol = builder.add_text_field("parent_symbol", symbol_opts);
        // Doc comments are searchable for semantic matching
        let doc_comment_opts = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqs),
            )
            .set_stored();
        let doc_comment = builder.add_text_field("doc_comment", doc_comment_opts);
        // Boolean flags stored as u64 for fast filtering
        let is_exported = builder.add_u64_field("is_exported", FAST | STORED);
        let is_test = builder.add_u64_field("is_test", FAST | STORED);

        Self {
            schema: builder.build(),
            id, path, content, symbol_name, symbol_type,
            start_line, end_line, language, file_hash,
            signature, parent_symbol, doc_comment, is_exported, is_test,
        }
    }
}

impl Default for IndexSchema {
    fn default() -> Self { Self::new() }
}
