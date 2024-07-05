use std::{collections::{HashMap, HashSet, VecDeque}, fs, io::{Read, Write}, primitive};
use indicatif::ParallelProgressIterator;
use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use reqwest;
use clap::{builder::Str, Parser};

mod terminal_interface;

fn main() {
    let args = terminal_interface::Args::parse();

    rayon::ThreadPoolBuilder::new().num_threads(args.threads).build_global().unwrap();

    if let Some(url) = args.load_url {
        let max_depth = args.max_depth;
        println!("Scanning: {} with max depth: {} and {} threads", url, max_depth, args.threads);
        parallel_list(&url, max_depth);
    } else if let Some(name) = args.shortest_path {
        let path = format!("data/{}/", name);
        let graph = Graph::from_path(&path);

        println!("Graph mit {} Nodes und {} Kanten geladen", graph.vertices.len(), graph.edges.iter().map(|(_, v)| v.len()).sum::<usize>());

        loop {
            print!("Start: ");
            std::io::stdout().flush().unwrap();
            let mut start_buffer = String::new();
            let start = std::io::stdin().read_line(&mut start_buffer).unwrap();

            if graph.vertices.iter().find(|&v| v == start_buffer.trim()).is_none() {
                println!("Vertex not found");
                continue;
            }
            
            print!("End: ");
            std::io::stdout().flush().unwrap();
            let mut end_buffer = String::new();
            let end = std::io::stdin().read_line(&mut end_buffer).unwrap();

            if graph.vertices.iter().find(|&v| v == end_buffer.trim()).is_none() {
                println!("Vertex not found");
                continue;
            }
            
            println!("Path: {:?}", graph.path(start_buffer.trim(), end_buffer.trim()));
        }
    } 
        
    
}

struct Graph {
    vertices: Vec<String>,
    edges: HashMap<usize, HashSet<usize>>,
}

impl Graph {
    fn from_path(path: &str) -> Self {
        let vertices = fs::read_to_string(format!("{}/vertices.txt", path))
            .unwrap()
            .split("\n")
            .par_bridge()
            .map(|s| s.to_owned())
            .collect::<Vec<_>>();
        
        let index_map = vertices.iter().enumerate().map(|(i, s)| (s.as_str(), i)).collect::<HashMap<_, _>>();

        let edges = fs::read_to_string(format!("{}/edges.txt", path))
            .unwrap()
            .lines()
            .par_bridge()
            .map(|s| { 
                let mut split = s.split(": ");
                let url = split.next().unwrap();
                (index_map[url], split.next().unwrap().split(", ").filter_map(|link| index_map.get(link).cloned()).collect())
            })
            .collect::<HashMap<_,_>>();
        
        Self {
            vertices,
            edges,
        }                
    }

    fn path(&self, start: &str, end: &str) -> Vec<String> {
        let start_index = self.vertices.iter().position(|v| v == start).unwrap();
        let end_index = self.vertices.iter().position(|v| v == end).unwrap();

        let mut queue = VecDeque::new();
        queue.push_back((start_index, vec![start_index]));
        let mut visited = HashSet::new();

        while let Some((index, distance)) = queue.pop_front() {
            if index == end_index {
                return distance.iter().map(|&i| self.vertices.get(i).cloned().unwrap()).collect();
            }

            visited.insert(index);

            let children = self.edges.get(&index).unwrap();
            for child in children {
                if visited.contains(child) {
                    continue;
                }
                queue.push_back((*child, distance.iter().cloned().chain(std::iter::once(*child)).collect()));
            }
        }
        Vec::new()
    }

    fn get_children(&self, s: &str) -> Vec<usize> {
        let index = self.vertices.iter().position(|v| v == s).unwrap();
        self.edges.get(&index).map(|links| links.iter().cloned().collect()).unwrap_or_default()
    }

    fn get_parents(&self, s: &str) -> Vec<usize> {
        let index = self.vertices.iter().position(|v| v == s).unwrap();
        self.edges.iter().filter_map(|(i, links)| if links.contains(&index) { Some(*i) } else { None }).collect()
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