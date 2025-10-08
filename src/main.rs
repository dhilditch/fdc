use clap::Parser;
use colored::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "fdc")]
#[command(about = "Find Dead Code - Identifies unused files in WordPress plugin projects")]
struct Cli {
    #[arg(help = "Path to scan (default: current directory)")]
    path: Option<PathBuf>,

    #[arg(short, long, help = "Delete found dead files")]
    delete: bool,

    #[arg(short, long, help = "Show verbose output")]
    verbose: bool,
}

#[derive(Debug, Clone)]
struct FileInfo {
    path: PathBuf,
    file_type: FileType,
    referenced_by: Vec<PathBuf>,
    referenced_in_comments: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
enum FileType {
    Php,
    JavaScript,
    Css,
}

impl FileType {
    fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "php" => Some(Self::Php),
            "js" => Some(Self::JavaScript),
            "css" => Some(Self::Css),
            _ => None,
        }
    }

    fn extensions(&self) -> &[&str] {
        match self {
            Self::Php => &["php"],
            Self::JavaScript => &["js"],
            Self::Css => &["css"],
        }
    }
}

struct DeadCodeFinder {
    root_path: PathBuf,
    files: HashMap<PathBuf, FileInfo>,
    php_files: Vec<PathBuf>,
}

impl DeadCodeFinder {
    fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            files: HashMap::new(),
            php_files: Vec::new(),
        }
    }

    fn discover_files(&mut self, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
        for entry in WalkDir::new(&self.root_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if let Some(file_type) = FileType::from_extension(ext) {
                        let file_info = FileInfo {
                            path: path.to_path_buf(),
                            file_type: file_type.clone(),
                            referenced_by: Vec::new(),
                            referenced_in_comments: Vec::new(),
                        };

                        if file_type == FileType::Php {
                            self.php_files.push(path.to_path_buf());
                        }

                        if verbose {
                            if let Ok(relative) = path.strip_prefix(&self.root_path) {
                                let icon = match file_type {
                                    FileType::Php => "🐘",
                                    FileType::JavaScript => "📜",
                                    FileType::Css => "🎨",
                                };
                                println!("  {} Found: {}", icon, relative.display().to_string().dimmed());
                            }
                        }

                        self.files.insert(path.to_path_buf(), file_info);
                    }
                }
            }
        }
        Ok(())
    }

    fn find_references(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Create a list of all filenames to search for
        let filenames: Vec<(PathBuf, String)> = self.files.keys()
            .filter_map(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| (path.clone(), name.to_string()))
            })
            .collect();

        // Patterns to detect comments
        let single_line_comment = Regex::new(r"//.*")?;
        let multi_line_comment = Regex::new(r"(?s)/\*.*?\*/")?;
        let hash_comment = Regex::new(r"#.*")?;

        for php_file in &self.php_files.clone() {
            let content = fs::read_to_string(php_file)?;
            
            // Remove comments to get clean content
            let mut clean_content = multi_line_comment.replace_all(&content, "").to_string();
            clean_content = single_line_comment.replace_all(&clean_content, "").to_string();
            clean_content = hash_comment.replace_all(&clean_content, "").to_string();

            // Extract only comment content
            let mut comment_content = String::new();
            for cap in single_line_comment.captures_iter(&content) {
                comment_content.push_str(&cap[0]);
                comment_content.push('\n');
            }
            for cap in multi_line_comment.captures_iter(&content) {
                comment_content.push_str(&cap[0]);
                comment_content.push('\n');
            }
            for cap in hash_comment.captures_iter(&content) {
                comment_content.push_str(&cap[0]);
                comment_content.push('\n');
            }

            // Check each filename
            for (file_path, filename) in &filenames {
                if file_path == php_file {
                    continue; // Skip self-reference
                }

                let found_in_clean = clean_content.contains(filename);
                let found_in_comments = comment_content.contains(filename);

                if let Some(file_info) = self.files.get_mut(file_path) {
                    if found_in_clean {
                        file_info.referenced_by.push(php_file.clone());
                    } else if found_in_comments {
                        file_info.referenced_in_comments.push(php_file.clone());
                    }
                }
            }
        }
        
        Ok(())
    }

    fn resolve_path(&self, base_file: &Path, referenced_path: &str) -> PathBuf {
        // First try relative to the base file's directory
        let base_dir = base_file.parent().unwrap_or(&self.root_path);
        let mut resolved = base_dir.join(referenced_path);
        
        if resolved.exists() {
            return resolved.canonicalize().unwrap_or(resolved);
        }
        
        // If not found, try relative to the project root
        resolved = self.root_path.join(referenced_path);
        if resolved.exists() {
            return resolved.canonicalize().unwrap_or(resolved);
        }
        
        // If still not found, look for files with matching basenames
        let referenced_basename = Path::new(referenced_path).file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(referenced_path);
        
        for (existing_path, _) in &self.files {
            if let Some(existing_basename) = existing_path.file_name().and_then(|name| name.to_str()) {
                if existing_basename == referenced_basename {
                    return existing_path.clone();
                }
            }
        }
        
        // Fall back to the original resolved path
        base_dir.join(referenced_path)
    }

    fn find_root_files(&self) -> HashSet<PathBuf> {
        let mut roots = HashSet::new();

        // Look for WordPress plugin header (the plugin root file)
        // Plugin Name: is a required field in WordPress plugin headers
        let plugin_header_pattern = Regex::new(r"(?m)^\s*\*\s*Plugin Name:").unwrap();

        for (path, file_info) in &self.files {
            if file_info.file_type == FileType::Php {
                if let Ok(content) = fs::read_to_string(path) {
                    if plugin_header_pattern.is_match(&content) {
                        roots.insert(path.clone());
                        return roots; // WordPress plugins have only one root file
                    }
                }
            }
        }

        roots
    }

    fn find_dead_files(&self) -> (Vec<&FileInfo>, Vec<&FileInfo>) {
        let roots = self.find_root_files();
        let mut dead_files = Vec::new();
        let mut commented_dead_files = Vec::new();

        for (path, file_info) in &self.files {
            if roots.contains(path) {
                continue; // Skip root files
            }

            let is_referenced = !file_info.referenced_by.is_empty();
            let is_commented = !file_info.referenced_in_comments.is_empty();

            if !is_referenced {
                if is_commented {
                    commented_dead_files.push(file_info);
                } else {
                    dead_files.push(file_info);
                }
            }
        }

        (dead_files, commented_dead_files)
    }

    fn print_results(&self, dead_files: &[&FileInfo], commented_dead_files: &[&FileInfo], verbose: bool) {
        let roots = self.find_root_files();

        if verbose {
            println!("\n{}", "=== Analysis Results ===".cyan().bold());
            println!();
            println!("{}", "Root files (not considered dead):".cyan().bold());
            for root in &roots {
                if let Ok(relative) = root.strip_prefix(&self.root_path) {
                    println!("  📁 {}", relative.display().to_string().blue());
                }
            }
            println!();

            // Show all alive files (referenced and not dead)
            let dead_paths: HashSet<&PathBuf> = dead_files.iter()
                .chain(commented_dead_files.iter())
                .map(|f| &f.path)
                .collect();

            let alive_files: Vec<&FileInfo> = self.files.values()
                .filter(|f| !roots.contains(&f.path) && !dead_paths.contains(&f.path))
                .collect();

            if !alive_files.is_empty() {
                println!("{}", "Alive files (referenced in code):".green().bold());
                for file in alive_files {
                    if let Ok(relative) = file.path.strip_prefix(&self.root_path) {
                        let icon = match file.file_type {
                            FileType::Php => "🐘",
                            FileType::JavaScript => "📜",
                            FileType::Css => "🎨",
                        };
                        println!("  {} {} (referenced by {} file(s))",
                            icon,
                            relative.display().to_string().green(),
                            file.referenced_by.len());
                    }
                }
                println!();
            }
        }

        if !dead_files.is_empty() {
            println!("{}", "Dead files (not referenced):".red().bold());
            for file in dead_files {
                if let Ok(relative) = file.path.strip_prefix(&self.root_path) {
                    let icon = match file.file_type {
                        FileType::Php => "🐘",
                        FileType::JavaScript => "📜",
                        FileType::Css => "🎨",
                    };
                    println!("  {} {}", icon, relative.display().to_string().red());
                }
            }
            println!();
        }

        if !commented_dead_files.is_empty() {
            println!("{}", "Files only referenced in comments (possibly temporarily dead):".yellow().bold());
            for file in commented_dead_files {
                if let Ok(relative) = file.path.strip_prefix(&self.root_path) {
                    let icon = match file.file_type {
                        FileType::Php => "🐘",
                        FileType::JavaScript => "📜",
                        FileType::Css => "🎨",
                    };
                    println!("  {} {}", icon, relative.display().to_string().yellow());
                    if verbose {
                        for ref_file in &file.referenced_in_comments {
                            if let Ok(ref_relative) = ref_file.strip_prefix(&self.root_path) {
                                println!("    💬 Referenced in: {}", ref_relative.display().to_string().dimmed());
                            }
                        }
                    }
                }
            }
            println!();
        }

        if dead_files.is_empty() && commented_dead_files.is_empty() {
            println!("{}", "✅ No dead files found!".green().bold());
        } else {
            println!("Found {} dead files and {} files only in comments", 
                dead_files.len().to_string().red().bold(), 
                commented_dead_files.len().to_string().yellow().bold());
        }
    }

    fn delete_files(&self, files: &[&FileInfo]) -> Result<(), Box<dyn std::error::Error>> {
        for file in files {
            println!("Deleting: {}", file.path.display().to_string().red());
            fs::remove_file(&file.path)?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let root_path = cli.path.unwrap_or_else(|| std::env::current_dir().unwrap());

    if !root_path.exists() {
        eprintln!("Error: Path '{}' does not exist", root_path.display());
        std::process::exit(1);
    }

    println!("🔍 Scanning for dead code in: {}", root_path.display().to_string().cyan());

    if cli.verbose {
        println!("\n{}", "Discovering files...".dimmed());
    }

    let mut finder = DeadCodeFinder::new(root_path);

    finder.discover_files(cli.verbose)?;
    println!("\n📊 Found {} files to analyze", finder.files.len());
    
    finder.find_references()?;
    
    let (dead_files, commented_dead_files) = finder.find_dead_files();
    
    finder.print_results(&dead_files, &commented_dead_files, cli.verbose);
    
    if cli.delete && (!dead_files.is_empty() || !commented_dead_files.is_empty()) {
        println!("\n{}", "⚠️  DELETE MODE ENABLED".red().bold());
        println!("This will permanently delete the identified dead files.");
        println!("Press Enter to continue or Ctrl+C to cancel...");
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        finder.delete_files(&dead_files)?;
        println!("🗑️  Deleted {} dead files", dead_files.len());
        
        if !commented_dead_files.is_empty() {
            println!("Note: Files only referenced in comments were not deleted for safety.");
        }
    }
    
    Ok(())
}