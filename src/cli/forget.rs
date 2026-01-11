//! Forget command implementation

use crate::cli::ForgetArgs;
use crate::core::error::Result;
use crate::core::project::Project;
use crate::index::TantivyIndex;

/// Run the forget command
pub fn run(args: ForgetArgs) -> Result<()> {
    let project = Project::from_path(&args.project)?;

    if TantivyIndex::exists(&project.root)? {
        TantivyIndex::delete(&project.root)?;
        println!("Removed index for: {}", project.root.display());
    } else {
        println!("No index found for: {}", project.root.display());
    }

    Ok(())
}
