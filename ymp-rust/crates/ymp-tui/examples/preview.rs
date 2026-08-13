//! Render every surface of the interface to stdout, without a live terminal.
//!
//! Run with `cargo run -p ymp-tui --example preview [width] [height]`. Each screen is built
//! from a domain run in `ymp_tui::scenario` and drawn by the production render path, so what
//! this prints is what the product draws for that state.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ymp_tui::journal::Model;
use ymp_tui::projection::{ContractFacts, Environment, VerifierFacts};
use ymp_tui::state::{App, Modal, PageKind, Surface};
use ymp_tui::{scenario, theme, ui};

fn environment() -> Environment {
    Environment::detect(std::path::Path::new(".ymp-data"))
}

fn contract() -> ContractFacts {
    ContractFacts {
        contract_id: "preview-contract".into(),
        contract_digest: scenario::digest(0xc0),
        source: std::path::PathBuf::from("./source"),
        prompt: "keep the replay path idempotent under concurrent requests".into(),
        verifier: Some(VerifierFacts {
            program: std::path::PathBuf::from("./verify.sh"),
            oracle_digest: scenario::digest(0x0a),
            negative_control: std::path::PathBuf::from("./negative-control"),
            wall_time_ms: 60_000,
        }),
        budget: Some(ymp_domain::Budget::new(1, 1)),
        run_id: Some("run-preview".into()),
        blocked: None,
        previously_authorized: false,
    }
}

fn app_for(run: Option<scenario::Run>, contracts: Vec<ContractFacts>) -> App {
    let mut model = Model::cold(environment(), contracts);
    if let Some(run) = run {
        model.absorb(&run.state, &run.events);
    }
    App::new(model.projection(None))
}

fn main() -> anyhow::Result<()> {
    let mut arguments = std::env::args().skip(1);
    let width: u16 = arguments
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(120);
    let height: u16 = arguments
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(40);

    let mut screens: Vec<(String, App)> = vec![
        ("transcript · cold start".to_owned(), app_for(None, vec![])),
        (
            "transcript · run in progress".to_owned(),
            app_for(Some(scenario::running()), vec![contract()]),
        ),
        ("transcript · scrolled back".to_owned(), {
            let mut app = app_for(Some(scenario::running()), vec![contract()]);
            app.scroll_up(4);
            app
        }),
    ];

    for (title, run) in [
        ("accepted", scenario::accepted()),
        ("exhausted", scenario::exhausted()),
        ("abstained", scenario::abstained()),
        ("cancelled", scenario::cancelled()),
        ("infrastructure_error", scenario::infrastructure_error()),
    ] {
        screens.push((
            format!("transcript · terminal {title}"),
            app_for(Some(run), vec![contract()]),
        ));
    }

    for kind in PageKind::ALL {
        let mut app = app_for(Some(scenario::running()), vec![contract()]);
        if kind == PageKind::Describe {
            app.describe_index = Some(0);
            let mut model = Model::cold(environment(), vec![contract()]);
            let run = scenario::running();
            model.absorb(&run.state, &run.events);
            let mut projection = model.projection(None);
            if let Some(page) = model.describe_candidate(0) {
                projection.pages.push((PageKind::Describe, page));
            }
            app.adopt(projection);
        }
        app.surface = Surface::Page(kind);
        screens.push((format!("page · :{}", kind.command_name()), app));
    }

    screens.push(("modal · command palette".to_owned(), {
        let mut app = app_for(Some(scenario::running()), vec![contract()]);
        app.prompt.suspended = Some("command palette open".into());
        app.modal = Modal::Palette(app.open_palette());
        app
    }));
    screens.push(("modal · authorize contract".to_owned(), {
        let mut app = app_for(Some(scenario::running()), vec![contract()]);
        app.open_authorize();
        app
    }));
    screens.push(("modal · confirm cancel".to_owned(), {
        let mut app = app_for(Some(scenario::running()), vec![contract()]);
        app.open_cancel_confirm();
        app
    }));
    screens.push(("overlay · keys".to_owned(), {
        let mut app = app_for(Some(scenario::running()), vec![contract()]);
        app.modal = Modal::Keys;
        app
    }));

    for (title, app) in &screens {
        print_screen(title, app, width, height)?;
    }
    print_screen(
        "guard · below the minimum size",
        &app_for(None, vec![]),
        72,
        20,
    )?;

    Ok(())
}

fn print_screen(title: &str, app: &App, width: u16, height: u16) -> anyhow::Result<()> {
    println!("\n{title}  ({width}x{height})");
    println!("+{}+", "-".repeat(width as usize));
    for line in draw(app, width, height)? {
        println!("|{line:<width$}|", width = width as usize);
    }
    println!("+{}+", "-".repeat(width as usize));
    Ok(())
}

fn draw(app: &App, width: u16, height: u16) -> anyhow::Result<Vec<String>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| ui::render(frame, app, &theme::UNICODE))?;
    let buffer = terminal.backend().buffer().clone();
    Ok((0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect())
}
