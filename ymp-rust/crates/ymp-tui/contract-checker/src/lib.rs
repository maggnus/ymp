#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ymp_tui::screen_contract::{
        CandidateProjection, RootStatus, RunScenario, RuntimeProfileKind, RuntimeProjection,
        RuntimeReadiness, Screen, ScreenHarness, ScreenProjection,
    };
    use ymp_tui::{ScreenFocus, ScreenKeyAction, ViewStateKind};

    const SIZES: [(u16, u16); 2] = [(80, 24), (120, 40)];

    fn mixed_runtimes() -> Vec<RuntimeProjection> {
        vec![
            RuntimeProjection::new(
                "runtime-fake",
                RuntimeProfileKind::Fake,
                Some("0.1.0".to_owned()),
                RuntimeReadiness::Ready,
                "deterministic in-process runtime",
            ),
            RuntimeProjection::new(
                "runtime-codex",
                RuntimeProfileKind::Codex,
                None,
                RuntimeReadiness::NotInstalled,
                "executable not found",
            ),
            RuntimeProjection::new(
                "runtime-claude",
                RuntimeProfileKind::ClaudeCode,
                None,
                RuntimeReadiness::Unauthenticated,
                "authentication unavailable",
            ),
        ]
    }

    fn candidates() -> Vec<CandidateProjection> {
        (0..4096)
            .map(|index| {
                let verification = if index == 318 {
                    "failed (candidate)"
                } else {
                    "unverified"
                };
                CandidateProjection::new(format!("c-{index:04}"), verification)
            })
            .collect()
    }

    fn scenario_projection(name: &str) -> ScreenProjection {
        match name {
            "first launch" => ScreenProjection::new(Screen::Run, RunScenario::FirstLaunch),
            "mixed runtime readiness" => {
                ScreenProjection::new(Screen::Runtimes, RunScenario::FirstLaunch)
                    .with_runtimes(mixed_runtimes(), "runtime-fake")
            }
            "running" => ScreenProjection::new(Screen::Run, RunScenario::Running),
            "cancellation" => ScreenProjection::new(
                Screen::Run,
                RunScenario::Cancelled {
                    reason: "operator cancellation".to_owned(),
                },
            ),
            "acceptance" => ScreenProjection::new(
                Screen::Run,
                RunScenario::Accepted {
                    candidate_id: "c-0007".to_owned(),
                },
            ),
            "candidate failure" => ScreenProjection::new(
                Screen::Run,
                RunScenario::CandidateFailure {
                    candidate_id: "c-0318".to_owned(),
                },
            ),
            "budget exhaustion" => ScreenProjection::new(
                Screen::Run,
                RunScenario::BudgetExhausted {
                    reason: "attempt budget exhausted".to_owned(),
                },
            ),
            "infrastructure failure" => ScreenProjection::new(
                Screen::Candidates,
                RunScenario::InfrastructureFailure {
                    reason: "verifier boundary lost".to_owned(),
                },
            )
            .with_candidates(candidates(), "c-0318", 300),
            _ => panic!("unknown contract scenario: {name}"),
        }
    }

    fn buffer(harness: &ScreenHarness, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("external TestBackend terminal");
        terminal
            .draw(|frame| harness.render_frame(frame))
            .expect("external frame formation");
        let buffer = terminal.backend().buffer();
        let mut output = String::new();
        for y in 0..height {
            for x in 0..width {
                output.push_str(buffer[(x, y)].symbol());
            }
            output.push('\n');
        }
        output
    }

    fn require(output: &str, needle: &str, region: &str) -> Result<(), String> {
        if output.contains(needle) {
            Ok(())
        } else {
            Err(format!("missing {region}: {needle:?}"))
        }
    }

    fn validate_named_regions(output: &str) -> Result<(), String> {
        for (needle, region) in [
            ("Store", "REG-CONTEXT"),
            ("Assurance", "REG-CONTEXT"),
            ("Focus", "REG-FOCUS"),
            ("<1>", "REG-NAV"),
            ("<3>", "REG-NAV"),
            ("<5>", "REG-NAV"),
            ("<q>", "REG-ACTIONS"),
            (" ymp ", "REG-BREADCRUMB"),
        ] {
            require(output, needle, region)?;
        }
        Ok(())
    }

    fn validate_scenario(name: &str, harness: &ScreenHarness, output: &str) -> Result<(), String> {
        validate_named_regions(output)?;
        match name {
            "first launch" => {
                require(output, "[ ] empty", "REG-STATE")?;
                require(output, "first launch · no attempt started", "REG-REASON")?;
                if output.contains("<s> submit") || output.contains("<v> verify") {
                    return Err("first launch offered an unavailable candidate action".to_owned());
                }
            }
            "mixed runtime readiness" => {
                require(output, "runtimes(all)[3]", "REG-BODY")?;
                require(output, "ready", "REG-STATE")?;
                require(output, "not installed", "REG-STATE")?;
                require(output, "unauthenticated", "REG-STATE")?;
                require(output, "selection: fake", "REG-SELECTION")?;
            }
            "running" => {
                require(output, "[*] ready", "REG-STATE")?;
                require(output, "[*] running", "REG-STATUS")?;
                require(output, "run in progress", "REG-REASON")?;
                require(output, "<s> submit", "REG-ACTIONS")?;
                if output.contains("<v> verify") {
                    return Err("running without a candidate offered verification".to_owned());
                }
            }
            "cancellation" => {
                require(output, "[!] terminal", "REG-STATE")?;
                require(output, "[x] cancelled", "REG-STATUS")?;
                require(output, "operator cancellation", "REG-TERMINAL-REASON")?;
            }
            "acceptance" => {
                require(output, "[+] accepted", "REG-STATUS")?;
                require(
                    output,
                    "exact candidate accepted by verifier evidence",
                    "REG-TERMINAL-REASON",
                )?;
            }
            "candidate failure" => {
                if harness.view_state() != ViewStateKind::Degraded
                    || harness.root_status() != RootStatus::Running
                {
                    return Err("candidate failure mapped to a terminal root outcome".to_owned());
                }
                require(output, "[~] degraded", "REG-STATE")?;
                require(output, "[*] running", "REG-STATUS")?;
                require(
                    output,
                    "candidate failure · run remains active",
                    "REG-REASON",
                )?;
                require(output, "<s> submit", "REG-ACTIONS")?;
                require(output, "<v> verify", "REG-ACTIONS")?;
                if output.contains("infrastructure_error") {
                    return Err("candidate failure became infrastructure_error".to_owned());
                }
            }
            "budget exhaustion" => {
                require(output, "[-] exhausted", "REG-STATUS")?;
                require(output, "attempt budget exhausted", "REG-TERMINAL-REASON")?;
            }
            "infrastructure failure" => {
                require(output, "[!] infrastructure_error", "REG-STATUS")?;
                require(
                    output,
                    "verifier boundary lost · no candidate judged",
                    "REG-TERMINAL-REASON",
                )?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    #[test]
    fn external_package_forms_all_16_required_buffers() {
        let scenarios = [
            "first launch",
            "mixed runtime readiness",
            "running",
            "cancellation",
            "acceptance",
            "candidate failure",
            "budget exhaustion",
            "infrastructure failure",
        ];
        let mut formed = 0;
        for (width, height) in SIZES {
            for name in scenarios {
                let harness = ScreenHarness::new(scenario_projection(name));
                let output = buffer(&harness, width, height);
                validate_scenario(name, &harness, &output)
                    .unwrap_or_else(|error| panic!("{name} at {width}x{height}: {error}"));
                formed += 1;
            }
        }
        assert_eq!(formed, 16);
    }

    fn candidate_failure_at(width: u16, height: u16) {
        let harness = ScreenHarness::new(scenario_projection("candidate failure"));
        let output = buffer(&harness, width, height);
        validate_scenario("candidate failure", &harness, &output)
            .unwrap_or_else(|error| panic!("candidate failure at {width}x{height}: {error}"));
    }

    #[test]
    fn candidate_failure_80x24_rejects_wrong_mapping() {
        candidate_failure_at(80, 24);
    }

    #[test]
    fn candidate_failure_120x40_rejects_wrong_mapping() {
        candidate_failure_at(120, 40);
    }

    #[test]
    fn high_volume_candidate_selection_and_terminal_reason_remain_visible() {
        for (width, height) in SIZES {
            let mut harness = ScreenHarness::new(scenario_projection("infrastructure failure"));
            let output = buffer(&harness, width, height);
            assert!(output.contains("candidates(run-contract)[4096]"));
            assert!(output.contains("selection: c-0318 · stable_id=c-0318"));
            assert!(output.contains("verifier boundary lost · no candidate judged"));
            assert!(output.contains("primary=<Enter/d> detail"));
            assert!(!output.contains("c-0000"));
            assert_eq!(harness.selected_candidate_id(), Some("c-0318"));
            assert_eq!(harness.handle_key(KeyCode::PageDown), ScreenKeyAction::None);
            assert_eq!(harness.selected_candidate_id(), Some("c-0318"));
            let paged = buffer(&harness, width, height);
            assert!(paged.contains("selection: c-0318 · stable_id=c-0318"));
            assert!(paged.contains("verifier boundary lost · no candidate judged"));
            assert_eq!(harness.handle_key(KeyCode::PageUp), ScreenKeyAction::None);
            assert_eq!(harness.selected_candidate_id(), Some("c-0318"));
        }
    }

    #[test]
    fn terminal_screens_offer_only_current_actions() {
        for name in [
            "cancellation",
            "acceptance",
            "budget exhaustion",
            "infrastructure failure",
        ] {
            let mut harness = ScreenHarness::new(scenario_projection(name));
            if harness.screen() != Screen::Run {
                assert_eq!(
                    harness.handle_key(KeyCode::Char('3')),
                    ScreenKeyAction::None
                );
            }
            let output = buffer(&harness, 120, 40);
            assert!(!output.contains("<x> cancel"), "{name}");
            assert!(!output.contains("<s> submit"), "{name}");
            assert!(!output.contains("<v> verify"), "{name}");
            assert_eq!(
                harness.handle_key(KeyCode::Char('x')),
                ScreenKeyAction::None
            );
            assert!(
                !harness
                    .available_actions()
                    .contains(&ScreenKeyAction::CancelRun)
            );
        }
    }

    #[test]
    fn keyboard_contract_executes_every_required_binding() {
        let projection = ScreenProjection::new(Screen::Runtimes, RunScenario::FirstLaunch)
            .with_runtimes(mixed_runtimes(), "runtime-fake");
        let mut harness = ScreenHarness::new(projection);

        assert_eq!(harness.focus(), ScreenFocus::Body);
        assert_eq!(harness.handle_key(KeyCode::Tab), ScreenKeyAction::None);
        assert_eq!(harness.focus(), ScreenFocus::Actions);
        assert_eq!(harness.handle_key(KeyCode::Tab), ScreenKeyAction::None);
        assert_eq!(harness.focus(), ScreenFocus::Body);

        assert_eq!(harness.handle_key(KeyCode::Down), ScreenKeyAction::None);
        assert_eq!(harness.selected_runtime_id(), Some("runtime-codex"));
        assert_eq!(harness.handle_key(KeyCode::Up), ScreenKeyAction::None);
        assert_eq!(harness.selected_runtime_id(), Some("runtime-fake"));
        assert_eq!(
            harness.handle_key(KeyCode::Char('j')),
            ScreenKeyAction::None
        );
        assert_eq!(harness.selected_runtime_id(), Some("runtime-codex"));
        assert_eq!(
            harness.handle_key(KeyCode::Char('k')),
            ScreenKeyAction::None
        );
        assert_eq!(harness.selected_runtime_id(), Some("runtime-fake"));
        assert_eq!(harness.handle_key(KeyCode::PageDown), ScreenKeyAction::None);
        assert_eq!(harness.selected_runtime_id(), Some("runtime-claude"));
        assert_eq!(harness.handle_key(KeyCode::PageUp), ScreenKeyAction::None);
        assert_eq!(harness.selected_runtime_id(), Some("runtime-fake"));

        assert_eq!(
            harness.handle_key(KeyCode::Char('/')),
            ScreenKeyAction::None
        );
        assert_eq!(harness.focus(), ScreenFocus::Filter);
        assert_eq!(harness.handle_key(KeyCode::Esc), ScreenKeyAction::None);
        assert_eq!(harness.focus(), ScreenFocus::Body);
        assert_eq!(
            harness.handle_key(KeyCode::Char('d')),
            ScreenKeyAction::None
        );
        assert_eq!(harness.focus(), ScreenFocus::Detail);
        assert_eq!(harness.handle_key(KeyCode::Esc), ScreenKeyAction::None);
        assert_eq!(
            harness.handle_key(KeyCode::Char('?')),
            ScreenKeyAction::None
        );
        assert_eq!(harness.focus(), ScreenFocus::Help);
        assert_eq!(harness.handle_key(KeyCode::Esc), ScreenKeyAction::None);
        assert_eq!(
            harness.handle_key(KeyCode::Enter),
            ScreenKeyAction::UseRuntime
        );
        assert_eq!(
            harness.handle_key(KeyCode::Char('q')),
            ScreenKeyAction::Quit
        );
    }

    #[test]
    fn monochrome_text_and_symbols_distinguish_root_states() {
        let cases = [
            ("running", "[*] running"),
            ("acceptance", "[+] accepted"),
            ("budget exhaustion", "[-] exhausted"),
            ("cancellation", "[x] cancelled"),
            ("infrastructure failure", "[!] infrastructure_error"),
        ];
        for (name, marker) in cases {
            let harness = ScreenHarness::new(scenario_projection(name));
            let output = buffer(&harness, 80, 24);
            assert!(output.contains(marker), "{name}: missing {marker}");
        }
    }
}
