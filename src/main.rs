use std::{collections::HashSet, fs};
use indicatif::ParallelProgressIterator;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reqwest;
use clap::Parser;

mod terminal_interface;

fn main() {
    let args = terminal_interface::Args::parse();

    rayon::ThreadPoolBuilder::new().num_threads(args.threads).build_global().unwrap();

    if let Some(url) = args.scan_url {
        let max_depth = args.max_depth;
        println!("Scanning: {} with max depth: {} and {} threads", url, max_depth, args.threads);
        parallel_list(&url, max_depth);
    } 
}

fn parallel_list(url: &str, max_depth: usize) {
    let dir_name = format!("data/{}", url.replace("/", "_"));
    fs::create_dir_all(&dir_name).unwrap_or_else(|_| println!("Directory already exists"));

    let client = reqwest::blocking::Client::new();
    
    let mut edges = Vec::new();

    let mut vertices = HashSet::new();
    vertices.insert(url.to_owned());
    
    let mut next_links = Vec::new();
    next_links.push(url.to_owned());

    let mut i = 0;
    while !next_links.is_empty() && i < max_depth{
        let start = std::time::Instant::now();
        let number_of_links = next_links.len();

        let mut page_links = next_links
            .into_par_iter()
            .map(|url| {
                let full_url = format!("https://de.wikipedia.org/wiki/{}", url);
                let links = match client.get(&full_url).send() {
                    Ok(response) => response.text()
                        .unwrap_or_else(|_| {
                            println!("No text from: {}", url);
                            String::new()
                        })
                        .split("<p>")
                        .map(|s| s.split("</p>").next().unwrap())
                        .flat_map(|p|  p.split("<a href=\"/wiki/")
                            .skip(1)
                            .map(|s| s.split("\"").next().unwrap()))
                        .filter(|&s| 
                               !s.starts_with("Datei") 
                            && !s.starts_with("Kategorie") 
                            && !s.starts_with("Hilfe")
                            && !s.starts_with("Benutzer")
                            && !s.starts_with("Spezial")
                            && !s.starts_with("Wikipedia")
                            && !s.starts_with("Diskussion")
                        )
                        .map(|s| s.to_owned())
                        .collect::<Vec<_>>(),
                    Err(_) => {
                        println!("No response from: {}", url);
                        Vec::new()
                    },
                };
                (url, links)
            })
            .progress()
            .collect::<Vec<(String, Vec<String>)>>();
            
        next_links = page_links.iter()
            .flat_map(|(_, links)| links.iter())
            .filter(|&s| vertices.insert(s.to_owned()))
            .map(|s| s.to_owned())
            .collect::<Vec<_>>();

        edges.append(&mut page_links);

        println!("{}: {} links in {}s", i, number_of_links, start.elapsed().as_secs_f64());
        println!("   Links per Second: {}", number_of_links as f64 / start.elapsed().as_secs_f64());
        println!("   Total Links: {}", vertices.len());
        i += 1;
    }

    let file_name = format!("{}/vertices.txt", dir_name);
    fs::write(file_name, vertices.into_iter().collect::<Vec<_>>().join("\n")).unwrap();
    let file_name = format!("{}/edges.txt", dir_name);
    fs::write(
        file_name, 
        edges.iter()
        .map(|(url, links)| format!("{}: {}", url, links.join(", ")))
        .collect::<Vec<_>>()
        .join("\n")
    ).unwrap();
}