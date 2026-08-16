//! A run's board section, as a fixture that records and reopens one.
//!
//! A run's store carries a board section wired when the run is created: the section directory
//! exists, its opening terms are recorded, and the facts committed to the board are persisted
//! by the crate that owns the section. Checks that need a run whose board has said things — a
//! delivery to replay, an export to compare, a corrupted section to refuse — reach that state
//! through here rather than by rebuilding the wiring by hand.
//!
//! The fixture uses the board exactly as the plane offers it: a command is executed against the
//! board the section holds, and the facts it commits are recorded by the section's own writer.
//! No payload bytes enter this file, and none leave the board; what a publication states is the
//! identity and the length of one, exactly as the plane records it.

use std::path::Path;

use ymp_application::Application;
use ymp_board::{Audience, BOARD_SECTION, BoardCommand, MessageKind, Payload, Publish, Relation};
use ymp_domain::Budget;

/// The identity of the payload every fixture publication states. The bytes are hashed by the
/// board itself and never carried into any record, so what this names is a digest, not content.
pub const FIXTURE_PAYLOAD: &[u8] = b"fixture board payload";

/// A run with a board section holding `messages` committed publications by the root participant.
///
/// The publications go to the project-discovery audience, which every participant may publish
/// to, so the fixture needs no scope opened and no grant held. Each publication commits the
/// facts the plane commits for one, and the section records them all.
pub fn run_with_board(root: &Path, messages: usize) -> Application {
    let mut application =
        Application::create(root, "board-fixture-run", Budget::new(2, 1)).expect("create run");
    for index in 1..=messages {
        let publish = Publish {
            message_id: format!("fixture-message-{index}"),
            author: "participant-root".to_owned(),
            audience: Audience::ProjectDiscovery,
            kind: MessageKind::Observation,
            payload: Payload::of(FIXTURE_PAYLOAD),
            salience_ms: 5_000,
            references: Vec::new(),
            relation: Relation::Standalone,
            claimed_decision_basis: Vec::new(),
        };
        application
            .record_board(&BoardCommand::Publish(publish))
            .expect("a fixture publication is recorded");
    }
    application
}

/// The board section directory of a run's store.
pub fn section_of(root: &Path) -> std::path::PathBuf {
    root.join(BOARD_SECTION)
}
