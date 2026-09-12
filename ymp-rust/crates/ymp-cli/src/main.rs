use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{discovery, run_turn, ProviderEvent, TurnRequest};
use ymp_runtime::Engine;
use ymp_storage::Store;

#[derive(Parser)]
#[command(
    name = "ymp",
    version,
    about = "A local team of AI agents that works together and learns from experience"
)]
struct Cli {
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[arg(short = 'C', long = "cwd", global = true)]
    cwd: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// Create configuration and discover existing local agent installations.
    Init,
    /// Show executable availability. --probe sends a small read-only model request.
    Doctor {
        #[arg(long)]
        probe: bool,
        #[arg(long)]
        provider: Option<String>,
        /// Verify a real team-tool call in addition to model access.
        #[arg(long, requires = "probe")]
        team_tools: bool,
    },
    /// Run an autonomous team without opening the TUI.
    Run {
        prompt: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        no_memory: bool,
        #[arg(long)]
        no_adaptive: bool,
    },
    /// Resume an interrupted team session.
    Resume {
        session: String,
        #[arg(long)]
        headless: bool,
    },
    /// Send a request to a single configured agent (read-only by default).
    Ask {
        agent: String,
        prompt: String,
        #[arg(long)]
        write: bool,
    },
    /// List saved sessions for the current project.
    Sessions,
    /// Show configuration, profile information, or the configuration file path.
    Config {
        #[arg(long)]
        path: bool,
    },
    /// Inspect stored competence observations.
    Reputation,
    /// Search verified memory for the current project.
    Memory {
        query: Option<String>,
        #[arg(long)]
        forget: Option<String>,
    },
    /// Reassociate an existing project with a relocated source directory.
    Relocate { project: String, path: PathBuf },
    /// Run an entirely local demonstration with three deterministic test agents.
    Demo {
        #[arg(long)]
        tui: bool,
    },
    /// Internal MCP stdio bridge for agents.
    #[command(hide = true)]
    Mcp {
        #[arg(long)]
        socket: PathBuf,
    },
}

#[tokio::main]
async fn main() {
    if let Err(e) = entry().await {
        eprintln!("ymp: {e:#}");
        std::process::exit(1);
    }
}

async fn entry() -> Result<()> {
    let argv: Vec<_> = std::env::args_os().collect();
    if argv.get(1).is_some_and(|arg| arg == "_supervise") {
        return ymp_providers::supervisor::run(argv.into_iter().skip(2).collect()).await;
    }
    let cli = Cli::parse();
    if let Some(Command::Mcp { socket }) = &cli.command {
        return ymp_runtime::mcp::stdio_bridge(socket).await;
    }
    let home = cli.home.unwrap_or(default_home()?);
    let path = cli.cwd.unwrap_or(std::env::current_dir()?).canonicalize()?;
    let store = Store::open(&home)?;
    if !home.join("config.toml").exists() {
        let mut config = Config::default();
        let _ = discovery::discover_glm(&mut config)?;
        for provider in &mut config.providers {
            if discovery::executable(&provider.command).is_none() {
                provider.enabled = false;
            }
        }
        config.save(&home)?;
    }
    let config = Config::load(&home)?;
    match cli.command {
        None => ymp_tui::run(store, config, path, None).await?,
        Some(Command::Mcp { .. }) => unreachable!(),
        Some(Command::Init) => {
            println!(
                "Configuration: {}\nData: {}",
                home.join("config.toml").display(),
                home.display()
            );
            print_health(&config);
        }
        Some(Command::Doctor {
            probe,
            provider,
            team_tools,
        }) => {
            print_health(&config);
            if probe {
                let mut failures = 0;
                for profile in config
                    .agents
                    .iter()
                    .filter(|a| a.enabled && provider.as_ref().is_none_or(|p| &a.provider == p))
                {
                    println!("Probing {}…", profile.name);
                    let result = if team_tools {
                        probe_team_tools(&config, &store, &path, profile).await
                    } else {
                        ask(
                            &config,
                            &store,
                            &path,
                            profile,
                            "Reply with exactly YMP_OK. Do not modify files or run commands.",
                            false,
                        )
                        .await
                    };
                    match result {
                        Ok(text) => println!("{}: {}", profile.name, text.trim()),
                        Err(e) => {
                            failures += 1;
                            eprintln!("{}: {e:#}", profile.name);
                        }
                    }
                }
                if failures > 0 {
                    bail!("{failures} provider probe(s) failed");
                }
            }
        }
        Some(Command::Run {
            prompt,
            json,
            no_memory,
            no_adaptive,
        }) => {
            headless(
                store,
                config,
                path,
                &prompt,
                None,
                json,
                !no_memory,
                !no_adaptive,
            )
            .await?
        }
        Some(Command::Resume {
            session,
            headless: headless_mode,
        }) => {
            if headless_mode {
                headless(store, config, path, "", Some(&session), false, true, true).await?;
            } else {
                ymp_tui::run(store, config, path, Some(session)).await?;
            }
        }
        Some(Command::Ask {
            agent,
            prompt,
            write,
        }) => {
            let text = ask(
                &config,
                &store,
                &path,
                config.agent(&agent)?,
                &prompt,
                write,
            )
            .await?;
            println!("{text}");
        }
        Some(Command::Sessions) => {
            let p = store.project(&path)?;
            for s in store.sessions(Some(&p.id))? {
                println!("{}  {:10}  {}", s.id, s.status, s.title);
            }
        }
        Some(Command::Config { path: only_path }) => {
            if only_path {
                println!("{}", home.join("config.toml").display());
            } else {
                println!("{}", toml::to_string_pretty(&config)?);
            }
        }
        Some(Command::Reputation) => {
            println!("{}", serde_json::to_string_pretty(&store.observations()?)?);
        }
        Some(Command::Memory { query, forget }) => {
            if let Some(id) = forget {
                store.forget_memory(&id)?;
                println!("Memory entry retired.");
            } else {
                let p = store.project(&path)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &store.memory(Some(&p.id), query.as_deref().unwrap_or(""))?
                    )?
                );
            }
        }
        Some(Command::Relocate { project, path }) => {
            store.relocate_project(&project, &path)?;
            println!("Project path updated; history and memory retained.");
        }
        Some(Command::Demo { tui }) => {
            let config = demo_config();
            if tui {
                ymp_tui::run(store, config, path, None).await?;
            } else {
                headless(
                    store,
                    config,
                    path,
                    "Create greeting.txt containing Hello from ymp, and verify it.",
                    None,
                    false,
                    true,
                    true,
                )
                .await?;
            }
        }
    }
    Ok(())
}

fn print_health(config: &Config) {
    for health in discovery::inspect(config) {
        println!(
            "{:<10} {:<12} {}",
            health.id,
            if health.available {
                "installed"
            } else {
                "unavailable"
            },
            health.detail
        );
    }
}

async fn ask(
    config: &Config,
    store: &Store,
    path: &std::path::Path,
    profile: &AgentProfile,
    prompt: &str,
    write: bool,
) -> Result<String> {
    let (ui, _) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let engine = Engine::new(store.clone(), config.clone(), ui, cancel.clone())?;
    let provider = config.provider(&profile.provider)?.clone();
    let request = TurnRequest {
        profile: profile.clone(),
        provider,
        cwd: path.into(),
        prompt: prompt.into(),
        purpose: "ask".into(),
        read_only: !write,
        resume: None,
        usage_baseline: None,
        mcp: None,
        timeout_secs: config.limits.turn_timeout_secs,
        bridge: engine.bridge,
    };
    let (tx, mut rx) = mpsc::unbounded_channel();
    let future = run_turn(request, cancel.clone(), tx);
    tokio::pin!(future);
    loop {
        tokio::select! {
            result=&mut future=>return Ok(result?.text),
            Some(event)=rx.recv()=>if let ProviderEvent::Tool(name)=event{eprintln!("{name}");},
            _=tokio::signal::ctrl_c()=>cancel.cancel(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn headless(
    store: Store,
    config: Config,
    path: PathBuf,
    prompt: &str,
    resume: Option<&str>,
    json_output: bool,
    memory: bool,
    adaptive: bool,
) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let mut engine = Engine::new(store, config, tx, cancel.clone())?;
    engine.use_memory = memory;
    engine.adaptive = adaptive;
    let future = engine.run(&path, prompt, resume);
    tokio::pin!(future);
    let outcome = loop {
        tokio::select! {
            result=&mut future=>break result?,
            Some(event)=rx.recv()=>match event{
                UiEvent::Status(text)=>if !json_output{eprintln!("{text}");},
                UiEvent::Message(m)=>if !json_output{eprintln!("\n[{} · {}]\n{}",m.author,m.kind,m.text);},
                _=>{},
            },
            _=tokio::signal::ctrl_c()=>{cancel.cancel();},
        }
    };
    if json_output {
        println!(
            "{}",
            json!({"session":outcome.session,"workspace":outcome.workspace,"summary":outcome.summary})
        );
    } else {
        println!(
            "\n{}\n\nSession: {}\nStatus: {}\nWorkspace: {}",
            outcome.summary,
            outcome.session.id,
            outcome.session.status,
            outcome.workspace.display()
        );
    }
    if outcome.session.status != "completed" {
        bail!(
            "Session {} is {}",
            outcome.session.id,
            outcome.session.status
        );
    }
    Ok(())
}

fn demo_config() -> Config {
    let mut config = Config::default();
    config.providers = vec![ProviderConfig {
        id: "demo".into(),
        kind: ProviderKind::Mock,
        command: "internal".into(),
        args: vec![],
        env_refs: Default::default(),
        enabled: true,
    }];
    config.agents = ["Atlas", "Boreal", "Cygnus"]
        .into_iter()
        .map(|name| AgentProfile {
            id: name.to_lowercase(),
            name: name.into(),
            provider: "demo".into(),
            model: None,
            instructions: format!("You are {name}."),
            enabled: true,
        })
        .collect();
    config.team = config.agents.iter().map(|a| a.id.clone()).collect();
    config
}

async fn probe_team_tools(
    config: &Config,
    store: &Store,
    path: &std::path::Path,
    profile: &AgentProfile,
) -> Result<String> {
    let project = store.project(path)?;
    let mut session = Session {
        id: new_id(),
        project_id: project.id,
        title: format!("Team-tool probe: {}", profile.name),
        status: "diagnostic".into(),
        created_at: now(),
        team: vec![profile.clone()],
        turns_used: 1,
    };
    store.save_session(&session)?;
    let (ui, _rx) = mpsc::unbounded_channel();
    let server = ymp_runtime::mcp::TeamServer::start(store.clone(), &session, ui.clone()).await?;
    let cancel = CancellationToken::new();
    let engine = Engine::new(store.clone(), config.clone(), ui, cancel.clone())?;
    let marker = format!("YMP_TOOL_OK_{}", new_id());
    let request = TurnRequest {
        profile: profile.clone(), provider: config.provider(&profile.provider)?.clone(), cwd: path.into(),
        prompt: format!("Call the ymp MCP tool team_post with text exactly {marker}. Then return YMP_OK. This is an explicitly authorized local team-chat write. Do not modify files or use other tools. Respond in English."),
        purpose: "probe".into(), read_only: true, resume: None, usage_baseline: None, timeout_secs: 120, bridge: engine.bridge,
        mcp: Some(ymp_providers::McpEndpoint { command: engine.executable.to_string_lossy().into(), args: vec!["mcp".into(), "--socket".into(), server.socket.to_string_lossy().into()], token: server.tokens[&profile.id].clone() }),
    };
    store.begin_usage(&session.id, 1, &profile.id)?;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let future = run_turn(request, cancel.clone(), tx);
    tokio::pin!(future);
    let result = loop {
        tokio::select! {
            result = &mut future => break result,
            Some(event) = rx.recv() => if let ProviderEvent::Usage(snapshot) = event {
                store.update_usage(&session.id, 1, &snapshot)?;
            },
            _ = tokio::signal::ctrl_c() => cancel.cancel(),
        }
    };
    while let Ok(event) = rx.try_recv() {
        if let ProviderEvent::Usage(snapshot) = event {
            store.update_usage(&session.id, 1, &snapshot)?;
        }
    }
    store.finish_usage(
        &session.id,
        1,
        if result.is_ok() {
            "completed"
        } else if cancel.is_cancelled() {
            "cancelled"
        } else {
            "failed"
        },
    )?;
    let seen = store
        .messages(&session.id, 0, 100)?
        .iter()
        .any(|m| m.author == profile.id && m.kind == "chat" && m.text == marker);
    session.status = if seen && result.is_ok() {
        "completed"
    } else {
        "blocked"
    }
    .into();
    store.save_session(&session)?;
    result?;
    if !seen {
        bail!("Agent returned without actually invoking team_post");
    }
    Ok("YMP_OK; team_post delivery independently confirmed".into())
}
