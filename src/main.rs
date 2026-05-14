use std::{
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = ".devtree.yml";
const STATE_FILE: &str = "state.json";

#[derive(Parser)]
#[command(name = "devtree")]
#[command(about = "Create runnable Git worktrees with managed dev servers and local URLs.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Write a starter .devtree.yml in the current repo.
    Init(InitArgs),
    /// Create a Git worktree and copy configured env files.
    Create(CreateArgs),
    /// Run configured setup commands in a worktree.
    Setup(BranchArgs),
    /// Start an app in a worktree and expose its URL.
    Start(StartArgs),
    /// Show tracked worktrees and app processes.
    Status(StatusArgs),
    /// Print the managed log for an app process.
    Logs(AppTargetArgs),
    /// Stop a managed app process.
    Stop(AppTargetArgs),
    /// Remove a worktree after its apps are stopped.
    Clean(CleanArgs),
    /// Run create, setup, and start in one command.
    Up(UpArgs),
    /// Print paths and tool availability.
    Doctor,
}

#[derive(Args)]
struct InitArgs {
    /// Overwrite an existing .devtree.yml.
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct CreateArgs {
    /// Branch name for the new worktree.
    branch: String,
    /// Create the branch from this ref. Defaults to HEAD.
    #[arg(long)]
    from: Option<String>,
}

#[derive(Args)]
struct BranchArgs {
    branch: String,
}

#[derive(Args)]
struct StartArgs {
    branch: String,
    /// App key from .devtree.yml. Defaults to "default" if present, otherwise the first app.
    app: Option<String>,
}

#[derive(Args)]
struct AppTargetArgs {
    branch: String,
    app: String,
}

#[derive(Args)]
struct CleanArgs {
    branch: String,
    /// Remove even when the worktree has local changes.
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct UpArgs {
    branch: String,
    /// App key from .devtree.yml. Defaults to "default" if present, otherwise the first app.
    app: Option<String>,
    /// Create the branch from this ref. Defaults to HEAD.
    #[arg(long)]
    from: Option<String>,
}

#[derive(Args)]
struct StatusArgs {
    /// Emit JSON instead of a human-readable table.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    name: String,
    worktrees_root: PathBuf,
    #[serde(default)]
    env: EnvConfig,
    #[serde(default)]
    setup: Vec<SetupStep>,
    #[serde(default)]
    apps: BTreeMap<String, AppConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct EnvConfig {
    #[serde(default)]
    copy: Vec<PathBuf>,
    #[serde(default, rename = "optionalCopy")]
    optional_copy: Vec<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupStep {
    name: String,
    run: String,
    #[serde(default = "default_cwd")]
    cwd: PathBuf,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    if_file_exists: Option<PathBuf>,
    #[serde(default)]
    if_file_missing: Option<PathBuf>,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    command: String,
    #[serde(default = "default_cwd")]
    cwd: PathBuf,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    url: UrlConfig,
    #[serde(default)]
    health_url: Option<String>,
    #[serde(default)]
    health_timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "provider", rename_all = "kebab-case")]
enum UrlConfig {
    Portless { name: Option<String> },
    RawPort { url: String },
    None,
}

impl Default for UrlConfig {
    fn default() -> Self {
        Self::Portless { name: None }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct State {
    repos: BTreeMap<String, RepoState>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct RepoState {
    path: PathBuf,
    worktrees: BTreeMap<String, WorktreeState>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct WorktreeState {
    path: PathBuf,
    branch: String,
    created_at: DateTime<Utc>,
    apps: BTreeMap<String, AppState>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppState {
    pid: u32,
    url: String,
    log_path: PathBuf,
    started_at: DateTime<Utc>,
    command: String,
}

struct ContextState {
    repo_root: PathBuf,
    repo_key: String,
    config: Config,
    state_path: PathBuf,
    state: State,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => init_config(args),
        Commands::Doctor => doctor(),
        Commands::Create(args) => {
            let mut ctx = ContextState::load()?;
            create_worktree(&mut ctx, &args.branch, args.from.as_deref())?;
            ctx.save()
        }
        Commands::Setup(args) => {
            let ctx = ContextState::load()?;
            run_setup(&ctx, &args.branch)
        }
        Commands::Start(args) => {
            let mut ctx = ContextState::load()?;
            let app = resolve_app_name(&ctx.config, args.app.as_deref())?;
            start_app(&mut ctx, &args.branch, &app)?;
            ctx.save()
        }
        Commands::Status(args) => {
            let ctx = ContextState::load()?;
            status(&ctx, args.json)
        }
        Commands::Logs(args) => {
            let ctx = ContextState::load()?;
            logs(&ctx, &args.branch, &args.app)
        }
        Commands::Stop(args) => {
            let mut ctx = ContextState::load()?;
            stop_app(&mut ctx, &args.branch, &args.app)?;
            ctx.save()
        }
        Commands::Clean(args) => {
            let mut ctx = ContextState::load()?;
            clean_worktree(&mut ctx, &args.branch, args.force)?;
            ctx.save()
        }
        Commands::Up(args) => {
            let mut ctx = ContextState::load()?;
            create_worktree(&mut ctx, &args.branch, args.from.as_deref())?;
            run_setup(&ctx, &args.branch)?;
            let app = resolve_app_name(&ctx.config, args.app.as_deref())?;
            start_app(&mut ctx, &args.branch, &app)?;
            ctx.save()
        }
    }
}

fn init_config(args: InitArgs) -> Result<()> {
    let repo_root = git_root()?;
    let config_path = repo_root.join(CONFIG_FILE);
    if config_path.exists() && !args.force {
        bail!(
            "{} already exists. Use --force to overwrite it.",
            config_path.display()
        );
    }

    let repo_name = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("myapp");
    let template = format!(
        r#"name: {repo_name}
worktreesRoot: ../{repo_name}-worktrees

env:
  copy: []
  optionalCopy:
    - .env.local

setup:
  - name: install dependencies
    run: pnpm install --frozen-lockfile

apps:
  web:
    cwd: .
    command: pnpm dev
    url:
      provider: portless
      name: {repo_name}
    healthUrl: /
"#
    );

    fs::write(&config_path, template)
        .with_context(|| format!("write {}", config_path.display()))?;
    println!("Wrote {}", config_path.display());
    Ok(())
}

impl ContextState {
    fn load() -> Result<Self> {
        let repo_root = git_root()?;
        let repo_key = git_common_dir(&repo_root)?;
        let config = load_config(&repo_root)?;
        let state_path = state_dir()?.join(STATE_FILE);
        let state = if state_path.exists() {
            let raw = fs::read_to_string(&state_path)
                .with_context(|| format!("read {}", state_path.display()))?;
            serde_json::from_str(&raw).with_context(|| format!("parse {}", state_path.display()))?
        } else {
            State::default()
        };

        Ok(Self {
            repo_root,
            repo_key,
            config,
            state_path,
            state,
        })
    }

    fn save(&self) -> Result<()> {
        let parent = self
            .state_path
            .parent()
            .ok_or_else(|| anyhow!("state path has no parent"))?;
        fs::create_dir_all(parent)?;
        let raw = serde_json::to_string_pretty(&self.state)?;
        fs::write(&self.state_path, raw)
            .with_context(|| format!("write {}", self.state_path.display()))
    }
}

fn load_config(repo_root: &Path) -> Result<Config> {
    let path = repo_root.join(CONFIG_FILE);
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read {}. Run `devtree init` first.", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn create_worktree(ctx: &mut ContextState, branch: &str, from: Option<&str>) -> Result<()> {
    let worktree_path = worktree_path(ctx, branch);
    if worktree_path.exists() {
        println!("Worktree already exists: {}", worktree_path.display());
    } else {
        ensure_parent(&worktree_path)?;
        let mut cmd = Command::new("git");
        cmd.current_dir(&ctx.repo_root)
            .args(["worktree", "add", "-b", branch])
            .arg(&worktree_path);
        if let Some(from) = from {
            cmd.arg(from);
        }
        run_command(&mut cmd, None)?;
    }

    copy_env_files(ctx, &worktree_path)?;
    let repo_state = ctx
        .state
        .repos
        .entry(ctx.repo_key.clone())
        .or_insert_with(|| RepoState {
            path: ctx.repo_root.clone(),
            ..RepoState::default()
        });
    repo_state
        .worktrees
        .entry(branch.to_string())
        .or_insert_with(|| WorktreeState {
            path: worktree_path.clone(),
            branch: branch.to_string(),
            created_at: Utc::now(),
            apps: BTreeMap::new(),
        });

    println!("Worktree: {}", worktree_path.display());
    Ok(())
}

fn copy_env_files(ctx: &ContextState, worktree_path: &Path) -> Result<()> {
    for source in &ctx.config.env.copy {
        copy_env_file(&ctx.repo_root, worktree_path, source, false)?;
    }
    for source in &ctx.config.env.optional_copy {
        copy_env_file(&ctx.repo_root, worktree_path, source, true)?;
    }
    Ok(())
}

fn copy_env_file(
    repo_root: &Path,
    worktree_path: &Path,
    relative_path: &Path,
    optional: bool,
) -> Result<()> {
    let source = repo_root.join(relative_path);
    if !source.exists() {
        if optional {
            return Ok(());
        }
        bail!("required env file does not exist: {}", source.display());
    }
    let destination = worktree_path.join(relative_path);
    if destination.exists() {
        println!("Env exists, leaving unchanged: {}", destination.display());
        return Ok(());
    }
    ensure_parent(&destination)?;
    fs::copy(&source, &destination)
        .with_context(|| format!("copy {} to {}", source.display(), destination.display()))?;
    println!("Copied env file: {}", relative_path.display());
    Ok(())
}

fn run_setup(ctx: &ContextState, branch: &str) -> Result<()> {
    let worktree_path = tracked_worktree_path(ctx, branch)?;
    if ctx.config.setup.is_empty() {
        println!("No setup commands configured.");
        return Ok(());
    }

    for step in &ctx.config.setup {
        if should_skip_step(&worktree_path, step) {
            println!("Skipping setup: {}", step.name);
            continue;
        }
        println!("Running setup: {}", step.name);
        let cwd = worktree_path.join(&step.cwd);
        let mut cmd = shell_command(&step.run);
        cmd.current_dir(&cwd);
        for (key, value) in &step.env {
            cmd.env(key, value);
        }
        run_command(&mut cmd, step.timeout_seconds)?;
    }
    Ok(())
}

fn start_app(ctx: &mut ContextState, branch: &str, app_name: &str) -> Result<()> {
    let worktree_path = tracked_worktree_path(ctx, branch)?;
    let app = ctx
        .config
        .apps
        .get(app_name)
        .ok_or_else(|| anyhow!("app `{app_name}` is not configured"))?;
    let log_path = log_path(&ctx.config.name, branch, app_name)?;
    ensure_parent(&log_path)?;

    if let Some(existing) = ctx
        .state
        .repos
        .get(&ctx.repo_key)
        .and_then(|repo| repo.worktrees.get(branch))
        .and_then(|worktree| worktree.apps.get(app_name))
        .filter(|existing| process_alive(existing.pid))
    {
        println!("Already running: {} ({})", existing.url, existing.pid);
        return Ok(());
    }

    let url = app_url(&ctx.config, branch, app);
    let cwd = worktree_path.join(&app.cwd);
    let command = command_for_app(&ctx.config, app);
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("open {}", log_path.display()))?;
    let err = log.try_clone()?;

    let mut child_cmd = shell_exec_command(&command);
    child_cmd
        .current_dir(&cwd)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err))
        .stdin(Stdio::null());
    for (key, value) in &app.env {
        child_cmd.env(key, value);
    }
    let child = child_cmd
        .spawn()
        .with_context(|| format!("start `{}` in {}", command, cwd.display()))?;

    let repo_state = ctx
        .state
        .repos
        .entry(ctx.repo_key.clone())
        .or_insert_with(|| RepoState {
            path: ctx.repo_root.clone(),
            ..RepoState::default()
        });
    let worktree_state = repo_state
        .worktrees
        .entry(branch.to_string())
        .or_insert_with(|| WorktreeState {
            path: worktree_path.clone(),
            branch: branch.to_string(),
            created_at: Utc::now(),
            apps: BTreeMap::new(),
        });
    worktree_state.apps.insert(
        app_name.to_string(),
        AppState {
            pid: child.id(),
            url: url.clone(),
            log_path: log_path.clone(),
            started_at: Utc::now(),
            command,
        },
    );

    if let Some(health_path) = &app.health_url {
        wait_for_health(&url, health_path, app.health_timeout_seconds.unwrap_or(45))?;
    }

    println!("App: {app_name}");
    println!("URL: {url}");
    println!("PID: {}", child.id());
    println!("Logs: {}", log_path.display());
    Ok(())
}

fn status(ctx: &ContextState, json: bool) -> Result<()> {
    let repo_state = ctx.state.repos.get(&ctx.repo_key);
    if json {
        println!("{}", serde_json::to_string_pretty(&repo_state)?);
        return Ok(());
    }
    let Some(repo_state) = repo_state else {
        println!("No devtree state for {}", ctx.repo_root.display());
        return Ok(());
    };
    for (branch, worktree) in &repo_state.worktrees {
        println!("{} -> {}", branch, worktree.path.display());
        for (app, state) in &worktree.apps {
            let live = if process_alive(state.pid) {
                "running"
            } else {
                "stopped"
            };
            println!("  {app}: {live} pid={} {}", state.pid, state.url);
        }
    }
    Ok(())
}

fn logs(ctx: &ContextState, branch: &str, app: &str) -> Result<()> {
    let state = tracked_app_state(ctx, branch, app)?;
    let mut file = File::open(&state.log_path)
        .with_context(|| format!("open {}", state.log_path.display()))?;
    std::io::copy(&mut file, &mut std::io::stdout())?;
    Ok(())
}

fn stop_app(ctx: &mut ContextState, branch: &str, app: &str) -> Result<()> {
    let state = tracked_app_state(ctx, branch, app)?;
    if process_alive(state.pid) {
        let mut cmd = Command::new("kill");
        cmd.arg(state.pid.to_string());
        run_command(&mut cmd, None)?;
    }
    if let Some(worktree) = ctx
        .state
        .repos
        .get_mut(&ctx.repo_key)
        .and_then(|repo| repo.worktrees.get_mut(branch))
    {
        worktree.apps.remove(app);
    }
    println!("Stopped {branch}/{app}");
    Ok(())
}

fn clean_worktree(ctx: &mut ContextState, branch: &str, force: bool) -> Result<()> {
    let worktree_path = tracked_worktree_path(ctx, branch)?;
    let running_apps: Vec<String> = ctx
        .state
        .repos
        .get(&ctx.repo_key)
        .and_then(|repo| repo.worktrees.get(branch))
        .map(|worktree| {
            worktree
                .apps
                .iter()
                .filter_map(|(name, app)| process_alive(app.pid).then_some(name.clone()))
                .collect()
        })
        .unwrap_or_default();
    if !running_apps.is_empty() {
        bail!(
            "stop running apps before clean: {}",
            running_apps.join(", ")
        );
    }
    if !force && worktree_dirty(&worktree_path)? {
        bail!("worktree has local changes. Use --force to remove it anyway.");
    }

    let mut cmd = Command::new("git");
    cmd.current_dir(&ctx.repo_root)
        .args(["worktree", "remove"])
        .arg(&worktree_path);
    if force {
        cmd.arg("--force");
    }
    run_command(&mut cmd, None)?;

    if let Some(repo) = ctx.state.repos.get_mut(&ctx.repo_key) {
        repo.worktrees.remove(branch);
    }
    println!("Removed {}", worktree_path.display());
    Ok(())
}

fn doctor() -> Result<()> {
    let repo_root = git_root()?;
    println!("Repo: {}", repo_root.display());
    println!("Repo key: {}", git_common_dir(&repo_root)?);
    println!("Config: {}", repo_root.join(CONFIG_FILE).display());
    println!("State: {}", state_dir()?.join(STATE_FILE).display());
    check_command("git");
    check_command("portless");
    Ok(())
}

fn wait_for_health(base_url: &str, health_path: &str, timeout_seconds: u64) -> Result<()> {
    let health_url = join_url(base_url, health_path);
    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(5))
        .build()?;
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    while Instant::now() < deadline {
        match client.get(&health_url).send() {
            Ok(response) if response.status().is_success() => {
                println!("Health: healthy ({health_url})");
                return Ok(());
            }
            _ => thread::sleep(Duration::from_millis(750)),
        }
    }
    bail!("health check did not pass within {timeout_seconds}s: {health_url}");
}

fn join_url(base: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

fn should_skip_step(worktree_path: &Path, step: &SetupStep) -> bool {
    if let Some(path) = &step.if_file_exists
        && !worktree_path.join(path).exists()
    {
        return true;
    }
    if let Some(path) = &step.if_file_missing
        && worktree_path.join(path).exists()
    {
        return true;
    }
    false
}

fn worktree_dirty(path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["status", "--porcelain"])
        .output()
        .with_context(|| format!("git status in {}", path.display()))?;
    if !output.status.success() {
        bail!("git status failed in {}", path.display());
    }
    Ok(!output.stdout.is_empty())
}

fn app_url(config: &Config, branch: &str, app: &AppConfig) -> String {
    match &app.url {
        UrlConfig::Portless { name } => {
            let app_name = name.as_deref().unwrap_or(&config.name);
            format!("https://{}.{}.localhost", sanitize_branch(branch), app_name)
        }
        UrlConfig::RawPort { url } => url.clone(),
        UrlConfig::None => "none".to_string(),
    }
}

fn command_for_app(config: &Config, app: &AppConfig) -> String {
    match &app.url {
        UrlConfig::Portless { name } => {
            let app_name = name.as_deref().unwrap_or(&config.name);
            format!("portless {app_name} {}", app.command)
        }
        UrlConfig::RawPort { .. } | UrlConfig::None => app.command.clone(),
    }
}

fn resolve_app_name(config: &Config, requested: Option<&str>) -> Result<String> {
    if let Some(app) = requested {
        return Ok(app.to_string());
    }
    if config.apps.contains_key("default") {
        return Ok("default".to_string());
    }
    config
        .apps
        .keys()
        .next()
        .cloned()
        .ok_or_else(|| anyhow!("no apps configured"))
}

fn tracked_worktree_path(ctx: &ContextState, branch: &str) -> Result<PathBuf> {
    ctx.state
        .repos
        .get(&ctx.repo_key)
        .and_then(|repo| repo.worktrees.get(branch))
        .map(|worktree| worktree.path.clone())
        .ok_or_else(|| anyhow!("unknown worktree `{branch}`. Run `devtree create {branch}` first."))
}

fn tracked_app_state<'a>(ctx: &'a ContextState, branch: &str, app: &str) -> Result<&'a AppState> {
    ctx.state
        .repos
        .get(&ctx.repo_key)
        .and_then(|repo| repo.worktrees.get(branch))
        .and_then(|worktree| worktree.apps.get(app))
        .ok_or_else(|| anyhow!("unknown app `{branch}/{app}`"))
}

fn worktree_path(ctx: &ContextState, branch: &str) -> PathBuf {
    let root = resolve_repo_relative(&ctx.repo_root, &ctx.config.worktrees_root);
    root.join(sanitize_branch(branch))
}

fn log_path(repo: &str, branch: &str, app: &str) -> Result<PathBuf> {
    Ok(state_dir()?
        .join("logs")
        .join(repo)
        .join(sanitize_branch(branch))
        .join(format!("{app}.log")))
}

fn state_dir() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("com", "hanifcarroll", "devtree")
        .ok_or_else(|| anyhow!("could not resolve user state directory"))?;
    Ok(dirs
        .state_dir()
        .unwrap_or_else(|| dirs.data_local_dir())
        .to_path_buf())
}

fn git_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("git rev-parse --show-toplevel")?;
    if !output.status.success() {
        bail!("current directory is not inside a Git repository");
    }
    let path = String::from_utf8(output.stdout)?.trim().to_string();
    Ok(PathBuf::from(path))
}

fn git_common_dir(repo_root: &Path) -> Result<String> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .context("git rev-parse --git-common-dir")?;
    if !output.status.success() {
        bail!(
            "could not resolve git common dir for {}",
            repo_root.display()
        );
    }
    let raw = String::from_utf8(output.stdout)?.trim().to_string();
    let path = if Path::new(&raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        repo_root.join(raw)
    };
    Ok(path
        .canonicalize()
        .unwrap_or(path)
        .to_string_lossy()
        .to_string())
}

fn resolve_repo_relative(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    Ok(())
}

fn shell_command(command: &str) -> Command {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut cmd = Command::new(shell);
    cmd.arg("-lc").arg(command);
    cmd
}

fn shell_exec_command(command: &str) -> Command {
    shell_command(&format!("exec {command}"))
}

fn run_command(command: &mut Command, timeout_seconds: Option<u64>) -> Result<()> {
    if let Some(timeout_seconds) = timeout_seconds {
        let mut child = command.spawn().context("spawn command")?;
        let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
        while Instant::now() < deadline {
            if let Some(status) = child.try_wait()? {
                if status.success() {
                    return Ok(());
                }
                bail!("command exited with {status}");
            }
            thread::sleep(Duration::from_millis(200));
        }
        let _ = child.kill();
        bail!("command timed out after {timeout_seconds}s");
    }

    let status = command.status().context("run command")?;
    if !status.success() {
        bail!("command exited with {status}");
    }
    Ok(())
}

fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn check_command(name: &str) {
    match Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {name}"))
        .output()
    {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout);
            println!("{name}: {}", path.trim());
        }
        _ => println!("{name}: missing"),
    }
}

fn sanitize_branch(branch: &str) -> String {
    branch
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn default_cwd() -> PathBuf {
    PathBuf::from(".")
}
