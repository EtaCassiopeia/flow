use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{Event as CtEvent, KeyCode, KeyEvent, KeyEventKind};
use futures::StreamExt;
use ratatui::layout::{Constraint, Direction, Layout};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;

use crate::cache::{
    self, CacheEntry, PRS_TTL, REPOS_TTL, Snapshot, TICKETS_TTL, WORKTREES_TTL,
};
use crate::config::Config;
use crate::domain::{
    BranchName, Pr, Repo, RepoFullName, SessionName, Ticket, Worktree,
};
use crate::msg::{
    AppMsg, BranchDeleteMode, Command, CreateWorkUnit, FetchId, FetchKind, FetchResult,
};
use crate::runtime::Registry;
use crate::services::git;
use crate::services::github::GithubClient;
use crate::services::jira::JiraClient;
use crate::services::tmux::Tmux;
use crate::ui::Tui;
use crate::ui::screens::{DataStore, LoadState, Screen, dashboard, setup};
use crate::ui::theme::Theme;
use crate::ui::widgets::toast::Toast;

pub struct AppState {
    pub config: Config,
    pub jira: Arc<dyn JiraClient>,
    pub github: Arc<dyn GithubClient>,
    pub tmux: Tmux,
    pub theme: Theme,
    pub data: DataStore,
    pub screen: Screen,
    pub help_open: bool,
    pub toast: Option<Toast>,
    pub tick: u64,
    pub pending_attach: Option<SessionName>,
    pub should_quit: bool,
    pub dirty: bool,

    // Caches
    pub tickets_cache: Option<CacheEntry<Vec<Ticket>>>,
    pub repos_cache: Option<CacheEntry<Vec<Repo>>>,
    pub worktrees_cache: Option<CacheEntry<Vec<Worktree>>>,
    pub prs_cache: HashMap<RepoFullName, CacheEntry<Vec<Pr>>>,

    // Per-kind in-flight tracking + cancellation registry
    pub registry: Registry,
    pub current_tickets_fetch: Option<FetchId>,
    pub current_repos_fetch: Option<FetchId>,
    pub current_worktrees_fetch: Option<FetchId>,
    pub current_prs_fetch: HashMap<RepoFullName, FetchId>,
}

impl AppState {
    pub fn new(
        config: Config,
        jira: Arc<dyn JiraClient>,
        github: Arc<dyn GithubClient>,
        screen: Screen,
    ) -> Self {
        let snap = cache::load();
        let mut data = DataStore::default();
        if let Some(e) = &snap.tickets {
            data.tickets = LoadState::Loaded(e.value.clone());
        }
        if let Some(e) = &snap.repos {
            data.repos = LoadState::Loaded(e.value.clone());
        }
        if let Some(e) = &snap.worktrees {
            data.worktrees = LoadState::Loaded(e.value.clone());
        }
        Self {
            config,
            jira,
            github,
            tmux: Tmux::new(),
            theme: Theme::dark(),
            data,
            screen,
            help_open: false,
            toast: None,
            tick: 0,
            pending_attach: None,
            should_quit: false,
            dirty: true,
            tickets_cache: snap.tickets,
            repos_cache: snap.repos,
            worktrees_cache: snap.worktrees,
            prs_cache: snap.prs,
            registry: Registry::new(),
            current_tickets_fetch: None,
            current_repos_fetch: None,
            current_worktrees_fetch: None,
            current_prs_fetch: HashMap::new(),
        }
    }

    pub fn in_flight_count(&self) -> usize {
        self.current_tickets_fetch.is_some() as usize
            + self.current_repos_fetch.is_some() as usize
            + self.current_worktrees_fetch.is_some() as usize
            + self.current_prs_fetch.len()
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            tickets: self.tickets_cache.clone(),
            repos: self.repos_cache.clone(),
            worktrees: self.worktrees_cache.clone(),
            prs: self.prs_cache.clone(),
        }
    }
}

pub async fn run(mut tui: Tui, mut app: AppState) -> crate::error::Result<()> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AppMsg>();
    spawn_input_reader(tx.clone());
    spawn_tick(tx.clone());
    let result = event_loop(&mut tui, &mut app, tx, rx).await;
    if let Err(e) = cache::save(&app.snapshot()) {
        tracing::warn!(error=%e, "failed to save cache snapshot");
    }
    result
}

fn spawn_input_reader(tx: UnboundedSender<AppMsg>) {
    tokio::spawn(async move {
        let mut events = crossterm::event::EventStream::new();
        while let Some(Ok(ev)) = events.next().await {
            if tx.send(AppMsg::Input(ev)).is_err() {
                break;
            }
        }
    });
}

fn spawn_tick(tx: UnboundedSender<AppMsg>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        interval.tick().await;
        loop {
            interval.tick().await;
            if tx.send(AppMsg::Tick).is_err() {
                break;
            }
        }
    });
}

async fn event_loop(
    tui: &mut Tui,
    app: &mut AppState,
    tx: UnboundedSender<AppMsg>,
    mut rx: UnboundedReceiver<AppMsg>,
) -> crate::error::Result<()> {
    redraw(tui, app)?;
    while let Some(msg) = rx.recv().await {
        handle(msg, app, &tx).await;
        if let Some(t) = &app.toast
            && t.expired()
        {
            app.toast = None;
            app.dirty = true;
        }
        if app.should_quit {
            break;
        }
        if let Some(name) = app.pending_attach.take() {
            attach(tui, app, &name).await?;
            app.dirty = true;
        }
        if app.dirty {
            redraw(tui, app)?;
            app.dirty = false;
        }
    }
    Ok(())
}

fn redraw(tui: &mut Tui, app: &AppState) -> std::io::Result<()> {
    tui.terminal.draw(|frame| draw(frame, app))?;
    Ok(())
}

fn draw(frame: &mut crate::ui::Frame, app: &AppState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    crate::ui::widgets::status_bar::render(
        frame,
        chunks[0],
        &app.theme,
        crate::ui::widgets::status_bar::StatusBarProps {
            mode: app.screen.name(),
            in_flight: app.in_flight_count(),
            tick: app.tick,
            right: None,
        },
    );

    match &app.screen {
        Screen::Setup(s) => setup::draw(frame, chunks[1], &app.theme, s),
        Screen::Dashboard => dashboard::draw(frame, chunks[1], &app.theme, app),
        Screen::Tickets(s) => crate::ui::screens::tickets::draw(frame, chunks[1], &app.theme, s, app),
        Screen::TicketDetail(s) => {
            crate::ui::screens::ticket_detail::draw(frame, chunks[1], &app.theme, s, app)
        }
        Screen::ConfirmCreate(s) => {
            crate::ui::screens::confirm_create::draw(frame, chunks[1], &app.theme, s)
        }
        Screen::Worktrees(s) => {
            crate::ui::screens::worktrees::draw(frame, chunks[1], &app.theme, s, app)
        }
        Screen::Prs(s) => crate::ui::screens::prs::draw(frame, chunks[1], &app.theme, s, app),
        Screen::Pending => {}
    }

    if app.help_open {
        crate::ui::screens::help::render(frame, area, &app.theme);
    }
    if let Some(toast) = &app.toast {
        crate::ui::widgets::toast::render(frame, area, &app.theme, toast);
    }
}

async fn handle(msg: AppMsg, app: &mut AppState, tx: &UnboundedSender<AppMsg>) {
    match msg {
        AppMsg::Tick => {
            app.tick = app.tick.wrapping_add(1);
            if app.in_flight_count() > 0 {
                app.dirty = true;
            }
        }
        AppMsg::Input(CtEvent::Key(key)) => {
            if key.kind != KeyEventKind::Press {
                return;
            }
            handle_key(key, app, tx);
            app.dirty = true;
        }
        AppMsg::Input(CtEvent::Resize(_, _)) => app.dirty = true,
        AppMsg::Input(_) => {}
        AppMsg::FetchDone(id, result) => {
            // Drop late results from cancelled fetches.
            if !app.registry.complete(id) {
                return;
            }
            apply_fetch(id, result, app);
            app.dirty = true;
        }
        AppMsg::Command(cmd) => {
            run_command(cmd, app, tx).await;
            app.dirty = true;
        }
        AppMsg::Quit => app.should_quit = true,
    }
}

fn handle_key(key: KeyEvent, app: &mut AppState, tx: &UnboundedSender<AppMsg>) {
    if key.code == KeyCode::Char('?') {
        app.help_open = !app.help_open;
        return;
    }
    if app.help_open {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
            app.help_open = false;
        }
        return;
    }

    let original = std::mem::replace(&mut app.screen, Screen::Pending);
    let (untouched, cmd) = match original {
        Screen::Setup(mut s) => {
            let cmd = setup::handle(key, &mut s, app);
            (Screen::Setup(s), cmd)
        }
        Screen::Dashboard => {
            let cmd = dashboard::handle(key, app);
            (Screen::Dashboard, cmd)
        }
        Screen::Tickets(mut s) => {
            let cmd = crate::ui::screens::tickets::handle(key, &mut s, app);
            (Screen::Tickets(s), cmd)
        }
        Screen::TicketDetail(mut s) => {
            let cmd = crate::ui::screens::ticket_detail::handle(key, &mut s, app);
            (Screen::TicketDetail(s), cmd)
        }
        Screen::ConfirmCreate(mut s) => {
            let cmd = crate::ui::screens::confirm_create::handle(key, &mut s, app);
            (Screen::ConfirmCreate(s), cmd)
        }
        Screen::Worktrees(mut s) => {
            let cmd = crate::ui::screens::worktrees::handle(key, &mut s, app);
            (Screen::Worktrees(s), cmd)
        }
        Screen::Prs(mut s) => {
            let cmd = crate::ui::screens::prs::handle(key, &mut s, app);
            (Screen::Prs(s), cmd)
        }
        Screen::Pending => (Screen::Dashboard, None),
    };

    if matches!(app.screen, Screen::Pending) {
        app.screen = untouched;
    }

    if let Some(c) = cmd {
        let _ = tx.send(AppMsg::Command(c));
    }
}

async fn run_command(cmd: Command, app: &mut AppState, tx: &UnboundedSender<AppMsg>) {
    match cmd {
        Command::Refresh { kind, force } => spawn_fetch(kind, force, app, tx),
        Command::RefreshAll => {
            spawn_fetch(FetchKind::Tickets, true, app, tx);
            spawn_fetch(FetchKind::Repos, true, app, tx);
            // PRs need a per-repo loop; trigger only if we have repos already.
            if let Some(repos) = app.data.repos.loaded().cloned() {
                for r in repos {
                    spawn_fetch(FetchKind::Prs(r.full_name), true, app, tx);
                }
            }
        }
        Command::AttachSession(name) => {
            app.pending_attach = Some(name);
        }
        Command::CreateWorkUnit(spec) => create_work_unit(spec, app).await,
        Command::DeleteWorktree {
            repo: _,
            path,
            branch,
            delete_branch,
            kill_session,
        } => {
            let main_repo = locate_main_repo(&path).unwrap_or_else(|| path.clone());
            if let Err(e) = git::remove_worktree(&main_repo, &path, true).await {
                app.toast = Some(Toast::error(format!("worktree remove failed: {e}")));
                return;
            }
            match delete_branch {
                BranchDeleteMode::Keep => {}
                BranchDeleteMode::DeleteIfMerged => {
                    if let Err(e) = git::delete_branch(&main_repo, &branch, false).await {
                        app.toast =
                            Some(Toast::info(format!("branch kept (not merged): {e}")));
                    }
                }
                BranchDeleteMode::Force => {
                    if let Err(e) = git::delete_branch(&main_repo, &branch, true).await {
                        app.toast = Some(Toast::error(format!("force-delete failed: {e}")));
                    }
                }
            }
            if app.config.tmux.kill_on_remove_worktree
                && let Some(name) = kill_session
            {
                let _ = app.tmux.kill_session(&name).await;
            }
            // Force a worktree rescan so the listing reflects the deletion.
            spawn_fetch(FetchKind::Repos, true, app, tx);
        }
        Command::CheckoutPr { repo, number } => {
            let pr = match find_pr(&app.data, &repo, number.0) {
                Some(p) => p,
                None => {
                    app.toast = Some(Toast::error("PR not found in current cache"));
                    return;
                }
            };
            checkout_pr(pr, app).await;
        }
        Command::OpenUrl(url) => {
            if !url.is_empty() {
                let _ = open_url(&url);
            }
        }
    }
}

fn find_pr(data: &DataStore, repo: &RepoFullName, number: u64) -> Option<Pr> {
    data.prs
        .loaded()?
        .iter()
        .find(|p| p.repo == *repo && p.number.0 == number)
        .cloned()
}

fn open_url(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
}

async fn create_work_unit(spec: CreateWorkUnit, app: &mut AppState) {
    if let Err(e) = git::create_worktree(
        &spec.repo_path,
        &spec.worktree_path,
        &spec.branch,
        spec.start_point.as_ref(),
    )
    .await
    {
        app.toast = Some(Toast::error(format!("worktree creation failed: {e}")));
        return;
    }
    if !app.tmux.has_session(&spec.session).await.unwrap_or(false)
        && let Err(e) = app
            .tmux
            .new_session_detached(&spec.session, &spec.worktree_path)
            .await
    {
        app.toast = Some(Toast::error(format!("tmux session failed: {e}")));
        return;
    }
    app.toast = Some(Toast::info(format!(
        "created {}",
        spec.worktree_path.display()
    )));
    app.pending_attach = Some(spec.session);
}

async fn checkout_pr(pr: Pr, app: &mut AppState) {
    let repos = match app.data.repos.loaded() {
        Some(v) => v.clone(),
        None => {
            app.toast = Some(Toast::error("no repos loaded"));
            return;
        }
    };
    let repo = match repos.iter().find(|r| r.full_name == pr.repo).cloned() {
        Some(r) => r,
        None => {
            app.toast = Some(Toast::error(format!(
                "repo {} not on disk under code_root — clone it first",
                pr.repo
            )));
            return;
        }
    };
    let parent = match repo.local_path.parent() {
        Some(p) => p.to_path_buf(),
        None => {
            app.toast = Some(Toast::error("repo has no parent dir"));
            return;
        }
    };
    let leaf = format!("{}-pr-{}", repo.full_name.repo(), pr.number.0);
    let worktree_path = parent.join(&leaf);
    let temp_branch_str = format!("pr-{}-{}", pr.number.0, pr.head_ref);
    let branch = match BranchName::new(temp_branch_str.clone()) {
        Ok(b) => b,
        Err(_) => {
            app.toast = Some(Toast::error("could not derive branch name for PR"));
            return;
        }
    };
    if let Err(e) = git::create_worktree(
        &repo.local_path,
        &worktree_path,
        &branch,
        Some(&repo.default_branch),
    )
    .await
    {
        app.toast = Some(Toast::error(format!("worktree creation failed: {e}")));
        return;
    }
    if let Err(e) = app.github.checkout_pr(&worktree_path, pr.number.0).await {
        app.toast = Some(Toast::error(format!("gh pr checkout failed: {e}")));
        return;
    }
    let session_str = format!("pr-{}-{}", pr.repo.repo(), pr.number.0);
    let session = match SessionName::new(session_str) {
        Ok(s) => s,
        Err(_) => {
            app.toast = Some(Toast::error("invalid session name for PR"));
            return;
        }
    };
    let _ = app.tmux.new_session_detached(&session, &worktree_path).await;
    app.pending_attach = Some(session);
}

/// Decide whether to fire a network/disk fetch.
///
/// - If the cache has a fresh entry and `force == false`, surface it as Loaded
///   and return without spawning.
/// - If the cache has a stale entry, surface it as Loaded immediately and fall
///   through to spawn a refresh in the background.
/// - If a fetch of the same kind is already in flight and `force == false`,
///   return (avoid stampedes).
/// - On `force == true`, cancel the prior fetch (if any) and always spawn.
fn spawn_fetch(
    kind: FetchKind,
    force: bool,
    app: &mut AppState,
    tx: &UnboundedSender<AppMsg>,
) {
    match kind {
        FetchKind::Tickets => spawn_tickets(force, app, tx),
        FetchKind::Repos => {
            spawn_repos(force, app, tx);
            spawn_worktrees(force, app, tx);
        }
        FetchKind::Prs(repo) => spawn_prs(repo, force, app, tx),
    }
}

fn spawn_tickets(force: bool, app: &mut AppState, tx: &UnboundedSender<AppMsg>) {
    // Surface cached value (fresh or stale) into the UI.
    if let Some(entry) = &app.tickets_cache {
        let fresh = entry.is_fresh(TICKETS_TTL);
        if !matches!(app.data.tickets, LoadState::Loaded(_)) {
            app.data.tickets = LoadState::Loaded(entry.value.clone());
        }
        if fresh && !force {
            return;
        }
    } else if !matches!(app.data.tickets, LoadState::Loaded(_)) {
        app.data.tickets = LoadState::Loading;
    }

    // Avoid stampedes unless the user asked for force.
    if !force && app.current_tickets_fetch.is_some() {
        return;
    }
    if let Some(prev) = app.current_tickets_fetch.take() {
        app.registry.cancel(prev);
    }

    let (id, token) = app.registry.issue();
    app.current_tickets_fetch = Some(id);
    let jira = app.jira.clone();
    let jql = app.config.jira.jql_my_open.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        run_with_cancel(token, id, &tx, async move {
            let r = jira.search(&jql, 100).await.map_err(Into::into);
            FetchResult::Tickets(r)
        })
        .await;
    });
}

fn spawn_repos(force: bool, app: &mut AppState, tx: &UnboundedSender<AppMsg>) {
    if let Some(entry) = &app.repos_cache {
        let fresh = entry.is_fresh(REPOS_TTL);
        if !matches!(app.data.repos, LoadState::Loaded(_)) {
            app.data.repos = LoadState::Loaded(entry.value.clone());
        }
        if fresh && !force {
            return;
        }
    } else if !matches!(app.data.repos, LoadState::Loaded(_)) {
        app.data.repos = LoadState::Loading;
    }

    if !force && app.current_repos_fetch.is_some() {
        return;
    }
    if let Some(prev) = app.current_repos_fetch.take() {
        app.registry.cancel(prev);
    }

    let (id, token) = app.registry.issue();
    app.current_repos_fetch = Some(id);
    let github = app.github.clone();
    let cfg = app.config.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        run_with_cancel(token, id, &tx, async move {
            let r = match github.list_my_repos(50).await {
                Ok(remotes) => Ok(merge_local_repos(&cfg, remotes).await),
                Err(e) => Err(crate::error::Error::from(e)),
            };
            FetchResult::Repos(r)
        })
        .await;
    });
}

fn spawn_worktrees(force: bool, app: &mut AppState, tx: &UnboundedSender<AppMsg>) {
    if let Some(entry) = &app.worktrees_cache {
        let fresh = entry.is_fresh(WORKTREES_TTL);
        if !matches!(app.data.worktrees, LoadState::Loaded(_)) {
            app.data.worktrees = LoadState::Loaded(entry.value.clone());
        }
        if fresh && !force {
            return;
        }
    } else if !matches!(app.data.worktrees, LoadState::Loaded(_)) {
        app.data.worktrees = LoadState::Loading;
    }

    if !force && app.current_worktrees_fetch.is_some() {
        return;
    }
    if let Some(prev) = app.current_worktrees_fetch.take() {
        app.registry.cancel(prev);
    }

    let (id, token) = app.registry.issue();
    app.current_worktrees_fetch = Some(id);
    let cfg = app.config.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        run_with_cancel(token, id, &tx, async move {
            let res = scan_worktrees(&cfg).await;
            FetchResult::Worktrees(res)
        })
        .await;
    });
}

fn spawn_prs(repo: RepoFullName, force: bool, app: &mut AppState, tx: &UnboundedSender<AppMsg>) {
    if let Some(entry) = app.prs_cache.get(&repo) {
        let fresh = entry.is_fresh(PRS_TTL);
        if !matches!(app.data.prs, LoadState::Loaded(_)) {
            app.data.prs = LoadState::Loaded(entry.value.clone());
        }
        if fresh && !force {
            return;
        }
    } else if !matches!(app.data.prs, LoadState::Loaded(_)) {
        app.data.prs = LoadState::Loading;
    }

    if !force {
        if app.current_prs_fetch.contains_key(&repo) {
            return;
        }
    } else if let Some(prev) = app.current_prs_fetch.remove(&repo) {
        app.registry.cancel(prev);
    }

    let (id, token) = app.registry.issue();
    app.current_prs_fetch.insert(repo.clone(), id);
    let github = app.github.clone();
    let tx = tx.clone();
    let r = repo.clone();
    tokio::spawn(async move {
        run_with_cancel(token, id, &tx, async move {
            let res = github.list_open_prs(&r).await.map_err(Into::into);
            FetchResult::Prs(repo, res)
        })
        .await;
    });
}

/// Race the fetch future against cancellation. On cancel we drop the work
/// silently — the registry has already removed `id`, so any late `FetchDone`
/// would be discarded by `complete()`.
async fn run_with_cancel<Fut>(
    token: CancellationToken,
    id: FetchId,
    tx: &UnboundedSender<AppMsg>,
    fut: Fut,
) where
    Fut: std::future::Future<Output = FetchResult>,
{
    tokio::select! {
        _ = token.cancelled() => {}
        result = fut => {
            let _ = tx.send(AppMsg::FetchDone(id, result));
        }
    }
}

async fn scan_worktrees(cfg: &Config) -> crate::error::Result<Vec<Worktree>> {
    let root = crate::config::expand(&cfg.code_root)?;
    let mut out = Vec::new();
    let Ok(orgs) = std::fs::read_dir(&root) else {
        return Ok(out);
    };
    for org_entry in orgs.flatten() {
        let Ok(repos) = std::fs::read_dir(org_entry.path()) else {
            continue;
        };
        for repo_entry in repos.flatten() {
            let path = repo_entry.path();
            if !path.is_dir() {
                continue;
            }
            if !path.join(".git").exists() && !path.join("HEAD").exists() {
                continue;
            }
            let Some(owner) = org_entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(repo_name) =
                path.file_name().and_then(|s| s.to_str()).map(str::to_owned)
            else {
                continue;
            };
            let Ok(full) = RepoFullName::new(format!("{owner}/{repo_name}")) else {
                continue;
            };
            let mut listed = match git::list_worktrees(&path, &full).await {
                Ok(v) => v,
                Err(_) => continue,
            };
            for w in listed.iter_mut() {
                if let Ok(s) = git::worktree_status(&w.path).await {
                    w.status = s;
                }
            }
            out.append(&mut listed);
        }
    }
    Ok(out)
}

async fn merge_local_repos(
    cfg: &Config,
    remotes: Vec<crate::services::github::RepoSummary>,
) -> Vec<Repo> {
    let root = match crate::config::expand(&cfg.code_root) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for s in remotes {
        let local = root.join(s.full_name.owner()).join(s.full_name.repo());
        out.push(Repo {
            full_name: s.full_name,
            local_path: local,
            default_branch: s.default_branch,
            source: crate::domain::RepoSource::Watched,
        });
    }
    out.sort_by(|a, b| a.full_name.as_str().cmp(b.full_name.as_str()));
    out
}

fn apply_fetch(id: FetchId, result: FetchResult, app: &mut AppState) {
    match result {
        FetchResult::Tickets(r) => {
            if app.current_tickets_fetch == Some(id) {
                app.current_tickets_fetch = None;
            }
            match r {
                Ok(v) => {
                    app.tickets_cache = Some(CacheEntry::new(v.clone()));
                    app.data.tickets = LoadState::Loaded(v);
                }
                Err(e) => {
                    let msg = e.to_string();
                    if app.tickets_cache.is_some() {
                        // Keep stale data visible; surface the error as a toast.
                        app.toast = Some(Toast::error(format!("tickets: {msg}")));
                    } else {
                        app.data.tickets = LoadState::Failed(msg);
                    }
                }
            }
        }
        FetchResult::Repos(r) => {
            if app.current_repos_fetch == Some(id) {
                app.current_repos_fetch = None;
            }
            match r {
                Ok(v) => {
                    app.repos_cache = Some(CacheEntry::new(v.clone()));
                    app.data.repos = LoadState::Loaded(v);
                }
                Err(e) => {
                    let msg = e.to_string();
                    if app.repos_cache.is_some() {
                        app.toast = Some(Toast::error(format!("repos: {msg}")));
                    } else {
                        app.data.repos = LoadState::Failed(msg);
                    }
                }
            }
        }
        FetchResult::Prs(repo, r) => {
            if app.current_prs_fetch.get(&repo) == Some(&id) {
                app.current_prs_fetch.remove(&repo);
            }
            match r {
                Ok(v) => {
                    app.prs_cache.insert(repo.clone(), CacheEntry::new(v.clone()));
                    app.data.prs = LoadState::Loaded(v);
                }
                Err(e) => {
                    let msg = e.to_string();
                    if app.prs_cache.contains_key(&repo) {
                        app.toast = Some(Toast::error(format!("prs: {msg}")));
                    } else {
                        app.data.prs = LoadState::Failed(msg);
                    }
                }
            }
        }
        FetchResult::Worktrees(r) => {
            if app.current_worktrees_fetch == Some(id) {
                app.current_worktrees_fetch = None;
            }
            match r {
                Ok(v) => {
                    app.worktrees_cache = Some(CacheEntry::new(v.clone()));
                    app.data.worktrees = LoadState::Loaded(v);
                }
                Err(e) => {
                    let msg = e.to_string();
                    if app.worktrees_cache.is_some() {
                        app.toast = Some(Toast::error(format!("worktrees: {msg}")));
                    } else {
                        app.data.worktrees = LoadState::Failed(msg);
                    }
                }
            }
        }
    }
}

async fn attach(tui: &mut Tui, app: &mut AppState, name: &SessionName) -> std::io::Result<()> {
    if Tmux::inside_session() {
        if let Err(e) = app.tmux.switch_client(name).await {
            app.toast = Some(Toast::error(format!("switch-client failed: {e}")));
        }
        return Ok(());
    }
    let mut cmd = app.tmux.attach_command(name);
    tui.suspend_for(|| {
        let status = cmd.status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!("tmux attach exited {status}")));
        }
        Ok(())
    })?;
    Ok(())
}

/// Heuristic: given a worktree path, look in its parent directory for a sibling
/// dir containing a real `.git/` dir — that's the main clone.
fn locate_main_repo(worktree_path: &Path) -> Option<PathBuf> {
    let parent = worktree_path.parent()?;
    let read = std::fs::read_dir(parent).ok()?;
    for entry in read.flatten() {
        let p = entry.path();
        if p == *worktree_path {
            continue;
        }
        if p.join(".git").is_dir() {
            return Some(p);
        }
    }
    None
}
