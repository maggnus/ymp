use anyhow::{bail, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::{
    collections::BTreeMap,
    io::{self, IsTerminal},
    path::PathBuf,
    time::Duration,
};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_runtime::{Engine, RunOutcome};
use ymp_storage::Store;

const COMMANDS: &[&str] = &[
    "/help",
    "/providers",
    "/agents",
    "/agent",
    "/team",
    "/new",
    "/sessions",
    "/resume",
    "/tasks",
    "/diff",
    "/reputation",
    "/memory",
    "/pause",
    "/stop",
    "/limits",
    "/quit",
];

struct Screen {
    input: String,
    cursor: usize,
    history: Vec<String>,
    history_index: usize,
    messages: Vec<Message>,
    local: Vec<(String, String)>,
    streams: BTreeMap<String, String>,
    statuses: BTreeMap<String, String>,
    tasks: Vec<Task>,
    status: String,
    session: Option<String>,
    scroll: usize,
    view: String,
}
impl Screen {
    fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            history: vec![],
            history_index: 0,
            messages: vec![],
            local: vec![],
            streams: BTreeMap::new(),
            statuses: BTreeMap::new(),
            tasks: vec![],
            status: "Ready".into(),
            session: None,
            scroll: 0,
            view: "chat".into(),
        }
    }
    fn notice(&mut self, text: impl Into<String>) {
        self.local.push(("ymp".into(), text.into()));
        self.scroll = 0;
    }
    fn event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Message(m) => {
                self.session = Some(m.session_id.clone());
                self.streams.remove(&m.author);
                if !self.messages.iter().any(|old| old.seq == m.seq) {
                    self.messages.push(m);
                }
            }
            UiEvent::Delta { agent, text } => {
                let s = self.streams.entry(agent).or_default();
                s.push_str(&text);
                if s.len() > 100_000 {
                    let at = s
                        .char_indices()
                        .map(|(i, _)| i)
                        .find(|&i| i >= s.len() - 80_000)
                        .unwrap_or(0);
                    s.drain(..at);
                }
            }
            UiEvent::AgentStatus { agent, status } => {
                self.statuses.insert(agent, status);
            }
            UiEvent::Task(task) => {
                if let Some(old) = self.tasks.iter_mut().find(|t| t.id == task.id) {
                    *old = task;
                } else {
                    self.tasks.push(task);
                }
            }
            UiEvent::Status(s) => self.status = s,
            UiEvent::Finished { session_id, status } => {
                self.session = Some(session_id);
                self.status = status;
                self.streams.clear();
            }
        }
    }
    fn insert(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }
    fn backspace(&mut self) {
        if let Some((i, _)) = self.input[..self.cursor].char_indices().next_back() {
            self.input.drain(i..self.cursor);
            self.cursor = i;
        }
    }
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            event::DisableBracketedPaste
        );
    }
}

pub async fn run(
    store: Store,
    mut config: Config,
    path: PathBuf,
    resume: Option<String>,
) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("An interactive terminal is required. Use `ymp run` for headless execution.");
    }
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        event::EnableBracketedPaste
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        previous(info);
    }));
    let mut screen = Screen::new();
    screen.notice("Welcome to ymp. Your team works together, verifies results, and accumulates experience.\nType a task to begin. /team configures participants; /help lists commands.\nChanges are produced in an isolated workspace; your original files stay intact.");
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut running: Option<JoinHandle<Result<RunOutcome>>> = None;
    let mut cancel = CancellationToken::new();
    if let Some(id) = resume {
        load_session(&store, &mut screen, &id)?;
    }
    let mut tick = tokio::time::interval(Duration::from_millis(50));
    loop {
        tokio::select! {
            Some(e)=rx.recv()=>screen.event(e),
            _=tick.tick()=>{},
        }
        if running.as_ref().is_some_and(|h| h.is_finished()) {
            if let Some(handle) = running.take() {
                match handle.await {
                    Ok(Ok(outcome)) => {
                        screen.status = outcome.session.status;
                    }
                    Ok(Err(e)) => screen.notice(format!("Error: {e:#}")),
                    Err(e) => screen.notice(format!("Runtime failed: {e}")),
                }
            }
        }
        let mut quit = false;
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Paste(text) => {
                    screen.input.insert_str(screen.cursor, &text);
                    screen.cursor += text.len();
                }
                Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if running.is_some() {
                                cancel.cancel();
                                screen.status = "Stopping active turns…".into();
                            } else {
                                quit = true;
                            }
                        }
                        KeyCode::Char('d')
                            if key.modifiers.contains(KeyModifiers::CONTROL)
                                && screen.input.is_empty() =>
                        {
                            quit = true
                        }
                        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            screen.insert('\n')
                        }
                        KeyCode::Enter
                            if key
                                .modifiers
                                .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT) =>
                        {
                            screen.insert('\n')
                        }
                        KeyCode::Enter => {
                            let input = screen.input.trim().to_owned();
                            screen.input.clear();
                            screen.cursor = 0;
                            if input.is_empty() {
                                continue;
                            }
                            screen.history.push(input.clone());
                            screen.history_index = screen.history.len();
                            screen.scroll = 0;
                            if input == "/quit" {
                                quit = true;
                                continue;
                            }
                            if input == "/pause" || input == "/stop" {
                                cancel.cancel();
                                screen.notice("Stopping active turns. Resume later with /resume.");
                                continue;
                            }
                            if input == "/resume" || input.starts_with("/resume ") {
                                if running.is_some() {
                                    screen.notice("A run is already active.");
                                    continue;
                                }
                                let id = input
                                    .split_whitespace()
                                    .nth(1)
                                    .map(str::to_owned)
                                    .or_else(|| screen.session.clone());
                                if let Some(id) = id {
                                    if let Err(e) = load_session(&store, &mut screen, &id) {
                                        screen.notice(e.to_string());
                                        continue;
                                    }
                                    cancel = CancellationToken::new();
                                    let engine = Engine::new(
                                        store.clone(),
                                        config.clone(),
                                        tx.clone(),
                                        cancel.clone(),
                                    )?;
                                    let cwd = path.clone();
                                    running = Some(tokio::spawn(async move {
                                        engine.run(&cwd, "", Some(&id)).await
                                    }));
                                } else {
                                    screen.notice("Use /sessions to choose a session.");
                                }
                                continue;
                            }
                            if input.starts_with('/') {
                                let previous_config = config.clone();
                                if let Err(e) = command(
                                    &store,
                                    &mut config,
                                    &path,
                                    &mut screen,
                                    &input,
                                    running.is_some(),
                                )
                                .await
                                {
                                    config = previous_config;
                                    screen.notice(format!("Error: {e:#}"));
                                }
                            } else if running.is_some() {
                                if let Some(id) = &screen.session {
                                    let m = store.message(id, "you", None, "user", &input)?;
                                    screen.event(UiEvent::Message(m));
                                    screen.status="Message saved; agents receive it at the next turn boundary".into();
                                }
                            } else {
                                screen.messages.clear();
                                screen.tasks.clear();
                                screen.streams.clear();
                                screen.local.clear();
                                screen.view = "chat".into();
                                cancel = CancellationToken::new();
                                let engine = Engine::new(
                                    store.clone(),
                                    config.clone(),
                                    tx.clone(),
                                    cancel.clone(),
                                )?;
                                let cwd = path.clone();
                                running = Some(tokio::spawn(async move {
                                    engine.run(&cwd, &input, None).await
                                }));
                            }
                        }
                        KeyCode::Backspace => screen.backspace(),
                        KeyCode::Delete => {
                            if let Some(ch) = screen.input[screen.cursor..].chars().next() {
                                screen
                                    .input
                                    .drain(screen.cursor..screen.cursor + ch.len_utf8());
                            }
                        }
                        KeyCode::Left => {
                            if let Some((i, _)) =
                                screen.input[..screen.cursor].char_indices().next_back()
                            {
                                screen.cursor = i;
                            }
                        }
                        KeyCode::Right => {
                            if let Some(ch) = screen.input[screen.cursor..].chars().next() {
                                screen.cursor += ch.len_utf8();
                            }
                        }
                        KeyCode::Home => screen.cursor = 0,
                        KeyCode::End => screen.cursor = screen.input.len(),
                        KeyCode::Up => {
                            if screen.history_index > 0 {
                                screen.history_index -= 1;
                                screen.input = screen.history[screen.history_index].clone();
                                screen.cursor = screen.input.len();
                            }
                        }
                        KeyCode::Down => {
                            screen.history_index =
                                (screen.history_index + 1).min(screen.history.len());
                            screen.input = screen
                                .history
                                .get(screen.history_index)
                                .cloned()
                                .unwrap_or_default();
                            screen.cursor = screen.input.len();
                        }
                        KeyCode::PageUp => screen.scroll = screen.scroll.saturating_add(12),
                        KeyCode::PageDown => screen.scroll = screen.scroll.saturating_sub(12),
                        KeyCode::Tab => {
                            let matches = COMMANDS
                                .iter()
                                .filter(|c| c.starts_with(&screen.input))
                                .collect::<Vec<_>>();
                            if matches.len() == 1 {
                                screen.input = format!("{} ", matches[0]);
                                screen.cursor = screen.input.len();
                            }
                        }
                        KeyCode::Esc => {
                            screen.view = "chat".into();
                        }
                        KeyCode::Char(ch)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            screen.insert(ch)
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        terminal.draw(|frame| draw(frame, &screen, &config, running.is_some(), &path))?;
        if quit {
            cancel.cancel();
            if let Some(handle) = running.take() {
                let _ = tokio::time::timeout(Duration::from_secs(10), handle).await;
            }
            break;
        }
    }
    Ok(())
}

fn load_session(store: &Store, screen: &mut Screen, id: &str) -> Result<()> {
    let session = store.session(id)?;
    screen.messages = store.messages(id, 0, 10000)?;
    screen.tasks = store.tasks(id)?;
    screen.session = Some(id.into());
    screen.status = session.status;
    screen.streams.clear();
    screen.local.clear();
    screen.scroll = 0;
    Ok(())
}

async fn command(
    store: &Store,
    config: &mut Config,
    path: &std::path::Path,
    screen: &mut Screen,
    input: &str,
    active: bool,
) -> Result<()> {
    let parts = input.split_whitespace().collect::<Vec<_>>();
    match parts[0]{
        "/help"=>screen.notice("/providers                 Configured local providers\n/agents                    Agent profiles\n/agent add ID PROVIDER [MODEL]\n/agent model ID MODEL      Change a model (use default to inherit)\n/agent instructions ID TEXT\n/team                      Current team\n/team add ID | remove ID   Set membership\n/new                       Clear the current view\n/sessions                  Saved sessions\n/resume [SESSION_ID]       Inspect interrupted work and continue\n/tasks                     Task graph and outcomes\n/diff                      Integrated patch\n/reputation                Evidence behind competence estimates\n/memory [QUERY]            Search project and global memory\n/memory forget ID          Retire a memory entry\n/limits [turns N|parallel N]\n/pause, /stop              Stop active turns and save state\n/quit                      Exit\n\nEnter sends · Ctrl+J inserts a newline · Tab completes commands\nPageUp/PageDown scroll · Ctrl+C stops a run, or exits when idle"),
        "/providers"=>screen.notice(config.providers.iter().map(|p|format!("{} · {:?} · {} · {}",p.id,p.kind,p.command,if p.enabled{"enabled"}else{"disabled"})).collect::<Vec<_>>().join("\n")),
        "/agents"=>screen.notice(config.agents.iter().map(|a|format!("{} · {} · model {} · {}",a.id,a.provider,a.model.as_deref().unwrap_or("provider default"),if a.enabled{"enabled"}else{"disabled"})).collect::<Vec<_>>().join("\n")),
        "/agent"=>{
            match parts.get(1).copied(){
                Some("add") if parts.len()>=4=>{config.provider(parts[3])?;config.agents.push(AgentProfile{id:parts[2].into(),name:parts[2].into(),provider:parts[3].into(),model:parts.get(4).map(|s|s.to_string()),instructions:String::new(),enabled:true});},
                Some("model") if parts.len()==4=>{let a=config.agents.iter_mut().find(|a|a.id==parts[2]).ok_or_else(||anyhow::anyhow!("Unknown profile"))?;a.model=(parts[3]!="default").then(||parts[3].into());},
                Some("instructions") if parts.len()>=4=>{let a=config.agents.iter_mut().find(|a|a.id==parts[2]).ok_or_else(||anyhow::anyhow!("Unknown profile"))?;a.instructions=parts[3..].join(" ");},
                Some(id)=>{screen.notice(serde_json::to_string_pretty(config.agent(id)?)?);return Ok(());},
                _=>bail!("Use /agent add, /agent model, /agent instructions, or /agent ID"),
            }
            config.save(&store.home)?;screen.notice("Profile saved. Existing sessions retain their profile snapshot.");
        },
        "/team"=>{
            match parts.get(1).copied(){
                Some("add") if parts.len()==3=>{let a=config.agent(parts[2])?.clone();if let Some(p)=config.providers.iter_mut().find(|p|p.id==a.provider){p.enabled=true;}
if let Some(a)=config.agents.iter_mut().find(|a|a.id==parts[2]){a.enabled=true;}
if !config.team.contains(&parts[2].to_string()){config.team.push(parts[2].into());}config.save(&store.home)?;},
                Some("remove") if parts.len()==3=>{config.team.retain(|a|a!=parts[2]);config.save(&store.home)?;},
                Some(_)=>bail!("Use /team add ID or /team remove ID"),None=>{},
            }
            screen.notice(format!("Team: {}",config.members().iter().map(|a|a.name.as_str()).collect::<Vec<_>>().join(", ")));
        },
        "/new"=>{if active{bail!("Stop the active run first");}screen.messages.clear();screen.tasks.clear();screen.local.clear();screen.session=None;screen.notice("Ready for a new task.");},
        "/sessions"=>{let p=store.project(path)?;screen.notice(store.sessions(Some(&p.id))?.iter().map(|s|format!("{} · {} · {}",s.id,s.status,s.title)).collect::<Vec<_>>().join("\n"));},
        "/tasks"=>{screen.view="tasks".into();},
        "/diff"=>{if let Some(id)=&screen.session{let s=store.session(id)?;let file=store.session_dir(&s).join("artifacts/result.patch");screen.notice(std::fs::read_to_string(file).unwrap_or_else(|_|"The patch is saved when the run stops.".into()));}},
        "/reputation"=>screen.notice(store.observations()?.iter().take(30).map(|o|format!("{} · {} / {} · {}\n  {}",o.agent_name,o.competence,o.difficulty,if o.success{"accepted"}else{"rejected"},o.evidence)).collect::<Vec<_>>().join("\n")),
        "/memory"=>{if parts.get(1)==Some(&"forget")&&parts.len()==3{store.forget_memory(parts[2])?;screen.notice("Memory entry retired.");}else{let p=store.project(path)?;let entries=store.memory(Some(&p.id),&parts[1..].join(" "))?;screen.notice(if entries.is_empty(){"No matching verified memory yet.".into()}else{entries.iter().map(|m|format!("{} · {} · {}\n{}",m.id,if m.project_id.is_some(){"project"}else{"global"},m.title,m.content)).collect::<Vec<_>>().join("\n")});}},
        "/limits"=>{if parts.len()==3{let n:usize=parts[2].parse()?;match parts[1]{"turns"=>config.limits.turns=n,"parallel"=>config.limits.parallel=n,_=>bail!("Use turns or parallel")};config.save(&store.home)?;}screen.notice(format!("{} parallel turns · {} total turns · {}s per turn. Changes apply to the next run.",config.limits.parallel,config.limits.turns,config.limits.turn_timeout_secs));},
        _=>bail!("Unknown command. Use /help."),
    }
    Ok(())
}

fn draw(
    frame: &mut ratatui::Frame<'_>,
    s: &Screen,
    config: &Config,
    running: bool,
    path: &std::path::Path,
) {
    let area = frame.area();
    let input_height = (s.input.lines().count().max(1) + 2).min(7) as u16;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(input_height),
            Constraint::Length(1),
        ])
        .split(area);
    let header = Line::from(vec![
        Span::styled(
            " ymp ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  a team that learns  "),
        Span::styled(
            path.file_name().unwrap_or_default().to_string_lossy(),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(header), rows[0]);
    let columns = if area.width >= 100 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(50), Constraint::Length(26)])
            .split(rows[1])
    } else {
        Layout::default()
            .constraints([Constraint::Percentage(100)])
            .split(rows[1])
    };
    let mut lines = Vec::new();
    if s.view == "tasks" {
        for task in &s.tasks {
            lines.push(Line::styled(
                format!("{:?}  {}", task.state, task.title),
                Style::default().fg(Color::Cyan),
            ));
            lines.push(Line::raw(format!(
                "  {} · attempt {} · {}",
                task.assignee.as_deref().unwrap_or("unassigned"),
                task.attempts,
                task.description
            )));
            if let Some(result) = &task.result {
                lines.push(Line::raw(result.clone()));
            }
            lines.push(Line::raw(""));
        }
    } else {
        for m in &s.messages {
            let color = if m.author == "you" {
                Color::Green
            } else if m.author == "ymp" {
                Color::DarkGray
            } else {
                Color::Cyan
            };
            lines.push(Line::styled(
                format!("{}  · {}", m.author, m.kind),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
            let text = if ["plan", "bid", "review", "review_plan", "final_review"]
                .contains(&m.kind.as_str())
            {
                serde_json::from_str::<serde_json::Value>(&m.text)
                    .ok()
                    .and_then(|v| {
                        v["summary"]
                            .as_str()
                            .or_else(|| v["reason"].as_str())
                            .or_else(|| v["approach"].as_str())
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| m.text.clone())
            } else {
                m.text.clone()
            };
            lines.extend(text.lines().map(|l| Line::raw(l.to_owned())));
            lines.push(Line::raw(""));
        }
        for (agent, text) in &s.streams {
            lines.push(Line::styled(
                format!("{agent} · streaming"),
                Style::default().fg(Color::Yellow),
            ));
            lines.extend(text.lines().map(|l| Line::raw(l.to_owned())));
        }
        for (author, text) in &s.local {
            lines.push(Line::styled(author, Style::default().fg(Color::DarkGray)));
            lines.extend(text.lines().map(|l| Line::raw(l.to_owned())));
            lines.push(Line::raw(""));
        }
    }
    let width = columns[0].width.saturating_sub(3).max(1) as usize;
    let height: usize = lines
        .iter()
        .map(|line| line.width().max(1).div_ceil(width))
        .sum();
    let offset = height
        .saturating_sub(columns[0].height.saturating_sub(2) as usize)
        .saturating_sub(s.scroll)
        .min(u16::MAX as usize) as u16;
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .scroll((offset, 0))
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .title(if s.view == "tasks" {
                        " Tasks · Esc returns to chat "
                    } else {
                        " Shared conversation "
                    }),
            ),
        columns[0],
    );
    if columns.len() > 1 {
        let mut side = vec![
            Line::styled("TEAM", Style::default().fg(Color::DarkGray)),
            Line::raw(""),
        ];
        for a in config.members() {
            let status = s.statuses.get(&a.id).map(String::as_str).unwrap_or("ready");
            side.push(Line::styled(a.name, Style::default().fg(Color::Cyan)));
            side.push(Line::raw(format!("  {status}")));
            side.push(Line::raw(""));
        }
        side.push(Line::raw(format!(
            "Tasks: {} / {}",
            s.tasks
                .iter()
                .filter(|t| t.state == TaskState::Accepted)
                .count(),
            s.tasks.len()
        )));
        if let Some(id) = &s.session {
            side.push(Line::raw(format!("Session: {}", &id[..8])));
        }
        side.push(Line::raw(""));
        side.push(Line::raw("/team  /tasks"));
        side.push(Line::raw("/memory  /reputation"));
        frame.render_widget(
            Paragraph::new(side).block(Block::default().borders(Borders::LEFT)),
            columns[1],
        );
    }
    let title = if s.input.starts_with('/') {
        COMMANDS
            .iter()
            .filter(|c| c.starts_with(s.input.split_whitespace().next().unwrap_or("")))
            .copied()
            .collect::<Vec<_>>()
            .join("  ")
    } else if running {
        " Message the team ".into()
    } else {
        " What should your team do? ".into()
    };
    frame.render_widget(
        Paragraph::new(s.input.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(title),
        ),
        rows[2],
    );
    let prefix = &s.input[..s.cursor];
    let row = prefix.chars().filter(|&c| c == '\n').count() as u16;
    let col =
        unicode_width::UnicodeWidthStr::width(prefix.rsplit('\n').next().unwrap_or("")) as u16;
    frame.set_cursor_position((
        rows[2].x + 1 + col.min(rows[2].width.saturating_sub(3)),
        rows[2].y + 1 + row.min(rows[2].height.saturating_sub(2)),
    ));
    frame.render_widget(
        Paragraph::new(Line::styled(
            format!(" {}  ·  Enter send  ·  Ctrl+J newline  ·  /help", s.status),
            Style::default().fg(Color::DarkGray),
        )),
        rows[3],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_editing_preserves_boundaries() {
        let mut s = Screen::new();
        s.insert('🦀');
        s.insert('界');
        s.backspace();
        assert_eq!(s.input, "🦀");
        s.backspace();
        assert_eq!(s.cursor, 0);
        assert!(s.input.is_empty());
    }
    #[test]
    fn renders_small_and_wide_terminals_without_losing_input() {
        for (width, height) in [(30, 8), (80, 24), (120, 35)] {
            let backend = ratatui::backend::TestBackend::new(width, height);
            let mut terminal = ratatui::Terminal::new(backend).unwrap();
            let mut screen = Screen::new();
            screen.input = "hello".into();
            screen.cursor = 5;
            screen.notice("Welcome");
            terminal
                .draw(|f| {
                    draw(
                        f,
                        &screen,
                        &Config::default(),
                        false,
                        std::path::Path::new("/project"),
                    )
                })
                .unwrap();
            let content = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(
                content.contains("hello"),
                "input disappeared at {width}x{height}"
            );
            assert!(content.contains("ymp"));
        }
    }
}
