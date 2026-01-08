use clap::CommandFactory;

// Include the CLI definition
include!("src/cli.rs");

fn main() -> std::io::Result<()> {
    // Only generate man pages if we're not running in a cross-compilation environment
    // where we might not want to run this, or standard build.
    
    println!("cargo:rerun-if-changed=src/cli.rs");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    
    let cmd = Cli::command();
    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    
    // Render the man page
    man.render(&mut buffer)?;
    
    // Write to file
    std::fs::write(out_dir.join("greppy.1"), buffer)?;
    
    Ok(())
}
