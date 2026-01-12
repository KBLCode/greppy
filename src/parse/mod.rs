pub mod chunker;
pub mod factory;
pub mod parser;
pub mod treesitter;
pub mod walker;

pub use chunker::{chunk_file, Chunk};
pub use factory::get_parser;
pub use parser::{CodeParser, HeuristicParser};
pub use treesitter::TreeSitterParser;
pub use walker::{detect_language, is_code_file, walk_project, FileInfo};
