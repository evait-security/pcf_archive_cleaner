use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::env;
use std::sync::Mutex;
use rusqlite::{Connection, Result, params};
use serde::Deserialize;
use log::{info, debug, error, LevelFilter, Log, Record, Metadata};
use simple_logger::SimpleLogger;
use chrono::Local;

#[derive(Deserialize)]
struct Config {
    workflows: Vec<TableColumn>,
    file_paths: HashMap<String, FilePath>,
}

#[derive(Deserialize)]
struct TableColumn {
    table: String,
    column: String,
    where_clause: String,
    params: String,
    parent: String
}

#[derive(Deserialize)]
struct FilePath {
    path: PathBuf,
}

struct CombinedLogger {
    file: Mutex<File>,
    console: SimpleLogger,
}

impl CombinedLogger {
    fn new(log_path: &Path) -> std::io::Result<Self> {
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(log_path)?;
        Ok(CombinedLogger {
            file: Mutex::new(file),
            console: SimpleLogger::new().with_level(LevelFilter::Debug),
        })
    }
}

impl Log for CombinedLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true  // Always enable logging for the file
    }

    fn log(&self, record: &Record) {
        // Log to console (respects console log level)
        self.console.log(record);

        // Always log to file
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let message = format!("{} - {} - {}\n", timestamp, record.level(), record.args());
        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(message.as_bytes());
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

fn is_valid_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn delete_file(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path)?;
    info!("File deleted: {:?}", path);
    Ok(())
}

fn delete_db_entries(conn: &Connection, table: &str, where_clause: &str, param: &str) -> Result<usize> {
    if !is_valid_identifier(table) || !is_valid_identifier(where_clause) {
        return Err(rusqlite::Error::InvalidParameterName(String::from("Invalid table or column name")));
    }
    
    let query = format!("DELETE FROM {} WHERE {} = ?", table, where_clause);
    debug!("Executing delete query: {}", query);
    debug!("Param: {:?}", param);
    
    let rows_affected = conn.execute(&query, params![param])?;
    info!("Deleted {} entries from table {}", rows_affected, table);
    Ok(rows_affected)
}

fn get_value_list_from(conn: &Connection, workflow: &TableColumn, where_clause: &str, param: &str) -> Result<Vec<String>> {
    if !is_valid_identifier(&workflow.table) {
        return Err(rusqlite::Error::InvalidParameterName(format!("Invalid table name: {}", workflow.table)));
    }
    if !is_valid_identifier(&workflow.column) {
        return Err(rusqlite::Error::InvalidParameterName(format!("Invalid column name: {}", workflow.column)));
    }
    if !is_valid_identifier(where_clause) {
        return Err(rusqlite::Error::InvalidParameterName(format!("Invalid where clause: {}", where_clause)));
    }

    let query = format!("SELECT {} FROM {} WHERE {} = ?", workflow.column, workflow.table, where_clause);
    debug!("Executing query: {}", query);
    debug!("Param: {:?}", param);

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map(params![param], |row| row.get::<_, String>(0))?;
    let result: Result<Vec<String>> = rows.collect();
    debug!("Query result length: {:?}", result.as_ref().map(|v| v.len()));
    result
}

fn ensure_path_within_project(base_path: &Path, path: &Path) -> PathBuf {
    let full_path = base_path.join(path);
    if full_path.starts_with(base_path) {
        full_path
    } else {
        base_path.to_path_buf()
    }
}

fn process_workflows(
    conn: &Connection,
    workflows: &[TableColumn],
    file_paths: &HashMap<String, FilePath>
) -> Result<()> {
    debug!("Starting process_workflows");
    let root_workflow = workflows.iter().find(|w| w.parent.is_empty()).expect("No root workflow found");
    process_workflow(conn, workflows, file_paths, root_workflow, None)?;
    debug!("Finished process_workflows");
    Ok(())
}

fn process_workflow(
    conn: &Connection,
    workflows: &[TableColumn],
    file_paths: &HashMap<String, FilePath>,
    workflow: &TableColumn,
    parent_id: Option<String>
) -> Result<()> {
    debug!("Processing workflow: table={}, column={}, where_clause={}, params={}, parent={}",
             workflow.table, workflow.column, workflow.where_clause, workflow.params, workflow.parent);
    debug!("Parent ID: {:?}", parent_id);

    let param = if let Some(id) = parent_id {
        id
    } else {
        workflow.params.clone()
    };

    debug!("Where clause: {}", workflow.where_clause);
    debug!("Param: {:?}", param);

    let ids = get_value_list_from(conn, workflow, &workflow.where_clause, &param)?;
    debug!("Found IDs: {:?}", ids);

    for id in &ids {
        // Process file deletions if applicable
        if let Some(file_path) = file_paths.get(&workflow.table) {
            debug!("Processing files for table: {}", workflow.table);
            let path = file_path.path.join(id);
            debug!("Attempting to delete file: {:?}", path);
            if let Err(e) = delete_file(&path) {
                error!("Failed to delete file {:?}: {}", path, e);
            }
        }

        // Process child workflows
        for child_workflow in workflows.iter().filter(|w| w.parent == workflow.table) {
            process_workflow(conn, workflows, file_paths, child_workflow, Some(id.clone()))?;
        }
    }

    // Delete all entries for this workflow in one operation
    if !ids.is_empty() {
        delete_db_entries(conn, &workflow.table, &workflow.where_clause, &param)?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <path_to_pcf_folder>", args[0]);
        std::process::exit(1);
    }

    let pcf_path = PathBuf::from(&args[1]);
    
    let exe_path = env::current_exe().expect("Failed to get executable path");
    let exe_dir = exe_path.parent().expect("Failed to get executable directory");
    let config_path = exe_dir.join("config.yaml");
    let log_path = exe_dir.join("pcf_del_archive.log");

    let logger = Box::new(CombinedLogger::new(&log_path).expect("Failed to create logger"));
    log::set_boxed_logger(logger).expect("Failed to set logger");
    log::set_max_level(LevelFilter::Info); // Set to Debug for debug output

    let start_time = Local::now();
    info!("Program started at {}", start_time.format("%Y-%m-%d %H:%M:%S"));

    let config_content = fs::read_to_string(&config_path)
        .expect("Failed to read config file");
    let config: Config = serde_yaml::from_str(&config_content)
        .expect("Failed to parse config file");

    let file_paths: HashMap<String, FilePath> = config.file_paths.into_iter()
        .map(|(key, value)| {
            let safe_path = ensure_path_within_project(&pcf_path, Path::new(&value.path));
            (key, FilePath { path: safe_path })
        })
        .collect();

    debug!("Opening database connection");
    let conn = Connection::open(&file_paths["DataBase"].path)?;
    debug!("Database connection opened successfully");

    process_workflows(&conn, &config.workflows, &file_paths)?;

    let end_time = Local::now();
    info!("Program ended at {}", end_time.format("%Y-%m-%d %H:%M:%S"));
    info!("Total execution time: {} seconds", (end_time - start_time).num_seconds());

    Ok(())
}