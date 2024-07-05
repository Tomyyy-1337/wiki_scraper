use clap::Parser;

/// Wikipedia Scraping Tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Load all links reachable from the given URL 
    #[arg(short, long, value_name = "PAGE_NAME")]
    pub scan_url: Option<String>,

    /// The number of threads to use for parallel processing
    #[arg(short, long, default_value = "250")]
    pub threads: usize,

    /// max depth of the search
    #[arg(short, long, default_value = "1024")]
    pub max_depth: usize,
}