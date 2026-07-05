use rhai::{Engine, Scope, AST, Dynamic};
use rustyline::DefaultEditor;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::fs;

// --- 1. CONFIGURATION & STATE SYSTEM ---

/// The custom Cargo.toml-like metadata file
#[derive(Debug, Deserialize, Clone)]
struct VaporToml {
    package: PackageMeta,
}

#[derive(Debug, Deserialize, Clone)]
struct PackageMeta {
    name: String,
    kind: String, // e.g., "workspace", "crate", "machinery"
}

/// Paths defining your scoped system
#[derive(Debug, Clone)]
struct ScopedPaths {
    user_data_root: PathBuf,   // Persistent data / source code
    machinery_root: PathBuf,   // Replaceable / rebuildable runtime app root
}

/// The state that gets mutated via commands or scripts
#[derive(Debug, Clone)]
struct ShellState {
    paths: ScopedPaths,
    current_dir: PathBuf,
    current_workspace: String,
    current_project: String,
    mode: String,              // "default", "sdk", "launcher", etc.
}

impl ShellState {
    fn update_context_from_toml(&mut self) {
        let vapor_path = self.current_dir.join("Vapor.toml");
        if vapor_path.exists() {
            if let Ok(content) = fs::read_to_string(vapor_path) {
                if let Ok(toml_data) = toml::from_str::<VaporToml>(&content) {
                    self.current_project = toml_data.package.name;
                    if toml_data.package.kind == "workspace" {
                        self.current_workspace = self.current_project.clone();
                    }
                    return;
                }
            }
        }
        // Fallback if no Vapor.toml exists or parsing fails
        self.current_project = "none".to_string();
    }
}

// --- 2. MAIN APPLICATION ENTRYPOINT ---

fn main() {
    // Define your real hardcoded scoped paths
    let paths = ScopedPaths {
        user_data_root: PathBuf::from("/home/user/src"),
        machinery_root: PathBuf::from("/opt/sdk/machinery"),
    };

    // Initialize shell state pointing initially to the source code root
    let state = Arc::new(Mutex::new(ShellState {
        current_dir: paths.user_data_root.clone(),
        paths,
        current_workspace: "root".to_string(),
        current_project: "none".to_string(),
        mode: "default".to_string(),
    }));

    // Trigger initial detection
    state.lock().unwrap().update_context_from_toml();

    // Set up Rhai Engine & Scope
    let mut engine = Engine::new();
    let mut scope = Scope::new();

    // Register state mutators into Rhai so script commands alter the terminal state
    register_shell_builtins(&mut engine, Arc::clone(&state));

    // Handle startup script hooks / specialized entry points
    run_entrypoint_scripts(&engine, &mut scope, &state);

    // Launch the fully interactive contextual REPL
    run_repl_loop(engine, scope, state);
}

// --- 3. RHAI ENGINE INTERACTION ---

fn register_shell_builtins(engine: &mut Engine, state: Arc<Mutex<ShellState>>) {
    let state_cd = Arc::clone(&state);
    // Custom `cd` function exposed directly to Rhai scripting environment
    engine.register_fn("cd", move |target_dir: &str| {
        let mut s = state_cd.lock().unwrap();
        let new_path = s.current_dir.join(target_dir);

        // Ensure path resolution stays normalized
        if let Ok(canonical) = fs::canonicalize(new_path) {
            s.current_dir = canonical;
            s.update_context_from_toml();
            println!("Moved to: {}", s.current_dir.display());
        } else {
            println!("Error: Path not found.");
        }
    });

    let state_mode = Arc::clone(&state);
    // State switcher to update shell mode (e.g. going into SDK or launcher contexts)
    engine.register_fn("set_mode", move |new_mode: &str| {
        let mut s = state_mode.lock().unwrap();
        s.mode = new_mode.to_string();
        println!("Shell mode switched to: [{}]", s.mode.to_uppercase());
    });
}

// --- 4. SCRIPT SCRIPTING & ENTRYPOINTS ---

fn run_entrypoint_scripts(engine: &Engine, scope: &mut Scope, state: &Arc<Mutex<ShellState>>) {
    let s = state.lock().unwrap();

    // Check for a general startup script file inside the persistent user directory
    let startup_script = s.paths.user_data_root.join("startup.rhai");
    if startup_script.exists() {
        println!("Executing startup hook script...");
        if let Err(e) = engine.run_file_with_scope(scope, startup_script) {
            println!("Startup script error: {}", e);
        }
    }
}

// --- 5. THE INTERACTIVE REPL LOOP ---

fn run_repl_loop(engine: Engine, mut scope: Scope, state: Arc<Mutex<ShellState>>) {
    let mut rl = DefaultEditor::new().expect("Failed to initialize terminal interface");

    loop {
        // Construct a highly-scannable contextual prompt dynamically
        let prompt = {
            let s = state.lock().unwrap();
            format!(
                "({mode}) [{ws}::{proj}] ❯ ",
                mode = s.mode,
                ws = s.current_workspace,
                proj = s.current_project
            )
        };

        match rl.readline(&prompt) {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() { continue; }
                let _ = rl.add_history_entry(input);

                // Check for hardcoded intercept commands first
                if input == "exit" || input == "quit" {
                    println!("Exiting sub-shell.");
                    break;
                }

                // If user runs a specialized mode script call explicitly
                if input == "sdk-mode" {
                    let path = state.lock().unwrap().paths.user_data_root.join("go_sdk.rhai");
                    eval_file_or_msg(&engine, &mut scope, &path);
                    continue;
                }

                // Treat everything else as live Rhai evaluation code statements
                match engine.eval_with_scope::<Dynamic>(&mut scope, input) {
                    Ok(result) => {
                        if !result.is_unit() {
                            println!("=> {:?}", result);
                        }
                    }
                    Err(e) => {
                        println!("Script Error: {}", e);
                    }
                }
            }
            Err(_) => break, // Gracefully catch Ctrl+C / Ctrl+D
        }
    }
}

fn eval_file_or_msg(engine: &Engine, scope: &mut Scope, path: &Path) {
    if path.exists() {
        if let Err(e) = engine.run_file_with_scope(scope, path.to_path_buf()) {
            println!("Script execution error: {}", e);
        }
    } else {
        println!("Script entrypoint not found at: {}", path.display());
    }
}
