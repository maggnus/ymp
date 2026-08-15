//! Ending the engine processes a provider measurement started, where waiting for them ran out.
//!
//! A measurement runs on a worker of its own, and that worker is inside the engines it started:
//! the driver starts each one and waits for it to answer, so nothing returns until it does.
//! Waiting for the worker is therefore waiting for those processes, and a wait that is bounded —
//! as a shutdown's must be — settles nothing about an engine that answers later than the bound.
//! Past it the interface is about to go away while a process it started is still running, which is
//! the condition W1-APP-02k rules out for a managed run. This module ends those processes instead,
//! and reads back from the operating system that none of them remains.
//!
//! # Which processes belong to the measurement
//!
//! Every process a managed run starts is put in a private process group of its own. Every process
//! the measurement path starts is not: a release and a model catalogue are read with a plain
//! invocation, so those processes stay in the process group this interface itself belongs to.
//! Membership of that group is what separates the two, and it is read rather than remembered, so a
//! run this interface is still supervising is left untouched by an ending that is not its own.
//!
//! That group also holds processes this interface never started — the shell or the harness that
//! leads it, and whatever else they placed there. What is ended is therefore the intersection: a
//! process this interface is an ancestor of, in this interface's own process group. Both halves are
//! read from the process table, through the same admitted utilities a managed run observes and ends
//! its own processes with.
//!
//! # What this cannot reach
//!
//! A descendant that leaves the process group and is orphaned in the same moment carries nothing
//! either reading can attribute to this interface. That is the boundary W1-APP-02k recorded for the
//! managed path, where it is closed by a marker the launch installs; the measurement path enters no
//! launch of its own and installs none, so the boundary stands here. Nothing on the measurement
//! path daemonises: the engines are asked for a version and for the models they serve, and each one
//! answers on the descriptor it was given.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ymp_runtime_api::{
    AdmittedProgram, ProgramRole, admit_lifecycle_programs, verify_admitted_programs,
};

/// How long the engines are given to leave on a request to end before they are ended outright.
pub const REQUEST_LIMIT: Duration = Duration::from_millis(500);
/// How long their absence is waited for once they have been ended outright.
pub const ENFORCEMENT_LIMIT: Duration = Duration::from_millis(500);
/// How often the process table is read while absence is waited for.
const OBSERVATION_INTERVAL: Duration = Duration::from_millis(20);

/// What ending the engines of a measurement established.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Termination {
    /// No process this interface started outside a private process group was running.
    NothingRunning,
    /// These processes were ended, and the process table was read back with none of them left.
    Ended(Vec<u32>),
    /// Nothing was established, for this reason. An ending that could not be observed is reported
    /// as one that did not happen, because the absence of an answer is not the absence of a
    /// process.
    Unestablished(String),
}

/// One process, as the operating system describes it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Process {
    pid: u32,
    parent: u32,
    group: u32,
    /// The state the operating system reports. A process that has ended and is only waiting to be
    /// collected by its parent still holds a row in the table, and it is not something running.
    state: String,
}

impl Process {
    fn ended(&self) -> bool {
        self.state.starts_with('Z')
    }
}

/// The utilities a termination reads and signals through. They are admitted on every termination
/// and verified again before each signal, so a utility that changes underneath a running interface
/// stops being used rather than being executed on this interface's behalf.
struct Utilities {
    admitted: Vec<AdmittedProgram>,
    table: PathBuf,
    signal: PathBuf,
}

impl Utilities {
    fn admit() -> Result<Self, String> {
        let admitted = admit_lifecycle_programs().map_err(|error| {
            format!(
                "the utilities that observe and end processes were not admitted, so nothing about \
                 the engines could be established: {error}"
            )
        })?;
        let path = |role: ProgramRole| {
            admitted
                .iter()
                .find(|program| program.role == role)
                .map(|program| program.path.clone())
                .ok_or_else(|| format!("no {role} was admitted"))
        };
        Ok(Self {
            table: path(ProgramRole::ProcessTable)?,
            signal: path(ProgramRole::Signal)?,
            admitted,
        })
    }

    /// Every process the operating system reports, except the reader this call started to ask.
    fn processes(&self) -> Result<Vec<Process>, String> {
        let reader = Command::new(&self.table)
            .args(["-A", "-o", "pid=,ppid=,pgid=,state="])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| format!("the process table could not be read: {error}"))?;
        let asking = reader.id();
        let output = reader
            .wait_with_output()
            .map_err(|error| format!("the process table reader did not finish: {error}"))?;
        if !output.status.success() {
            return Err("the process table reader exited unsuccessfully".to_owned());
        }
        let reported = String::from_utf8_lossy(&output.stdout);
        let processes = parse(&reported, asking);
        if processes.is_empty() {
            return Err("the process table reader reported no process at all".to_owned());
        }
        Ok(processes)
    }

    fn signal(&self, signal: &str, processes: &[u32]) -> Result<(), String> {
        if processes.is_empty() {
            return Ok(());
        }
        verify_admitted_programs(&self.admitted).map_err(|error| {
            format!("the utility that ends processes changed after it was admitted: {error}")
        })?;
        let mut command = Command::new(&self.signal);
        command.arg(signal);
        for pid in processes {
            command.arg(pid.to_string());
        }
        // A process that ended between the reading and the signal is reported as an error by the
        // utility. What decides is the reading that follows, not this exit.
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(drop)
            .map_err(|error| format!("the engines could not be signalled with {signal}: {error}"))
    }
}

/// Reads the utility's answer into the rows it is made of. A line that does not carry exactly the
/// four values that were asked for is not read at all, so a reader that answers something else is
/// not silently taken for a shorter process table.
///
/// Its own reader is left out: it is a process this interface started, in this interface's group,
/// and it is already leaving.
fn parse(reported: &str, asking: u32) -> Vec<Process> {
    let mut processes = Vec::new();
    for line in reported.lines() {
        let mut fields = line.split_whitespace();
        let (Some(pid), Some(parent), Some(group), Some(state), None) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            continue;
        };
        let (Ok(pid), Ok(parent), Ok(group)) = (pid.parse(), parent.parse(), group.parse()) else {
            continue;
        };
        if pid <= 1 || pid == asking {
            continue;
        }
        processes.push(Process {
            pid,
            parent,
            group,
            state: state.to_owned(),
        });
    }
    processes
}

/// The processes `root` is an ancestor of that are still in `root`'s own process group, an ancestor
/// before anything descended from it.
///
/// The table states one link each, so ancestry is closed by passing over it until it attributes
/// nothing further, and how many passes a process needed is how far it stands from `root`. The
/// group is applied to the result and not to the walk: a run that was given a private group is
/// excluded by it, and the descendants of such a run are excluded with it, because a private group
/// is inherited by everything the run starts.
///
/// The order matters when the signal is delivered. A process that is ending is free to start
/// another one first — the driver reads the models a build serves in batches, and a shell in the
/// same position starts its next child when the previous one dies — and a process that has already
/// been ended starts nothing. Reaching the ancestor first therefore stops the succession rather
/// than racing it, while everything already standing under it is signalled from the identifiers
/// this same reading holds, which no later reparenting takes away.
fn attributed(processes: &[Process], root: u32, group: u32) -> Vec<Process> {
    let mut depths = BTreeMap::from([(root, 0_u32)]);
    loop {
        let known = depths.len();
        for process in processes {
            if let Some(depth) = depths.get(&process.parent).copied() {
                depths.entry(process.pid).or_insert(depth + 1);
            }
        }
        if depths.len() == known {
            break;
        }
    }
    let mut attributed: Vec<Process> = processes
        .iter()
        .filter(|process| {
            process.pid != root
                && process.group == group
                && depths.contains_key(&process.pid)
                && !process.ended()
        })
        .cloned()
        .collect();
    attributed.sort_by_key(|process| (depths[&process.pid], process.pid));
    attributed
}

/// Ends every engine process this interface started on the measurement path, and establishes from
/// the process table that none of them is left.
///
/// The request to end comes first, so an engine that keeps records of its own closes them; what is
/// still attributed when [`REQUEST_LIMIT`] runs out is ended outright, and [`ENFORCEMENT_LIMIT`]
/// bounds the reading that establishes the result.
pub fn end_measurement_processes() -> Termination {
    match ended() {
        Ok(termination) => termination,
        Err(reason) => Termination::Unestablished(reason),
    }
}

fn ended() -> Result<Termination, String> {
    let utilities = Utilities::admit()?;
    let root = std::process::id();
    let mut signalled = BTreeSet::new();

    let left = clear(&utilities, root, "-TERM", REQUEST_LIMIT, &mut signalled)?;
    if signalled.is_empty() {
        return Ok(Termination::NothingRunning);
    }
    let left = match left.is_empty() {
        true => left,
        false => clear(&utilities, root, "-KILL", ENFORCEMENT_LIMIT, &mut signalled)?,
    };
    if left.is_empty() {
        return Ok(Termination::Ended(signalled.into_iter().collect()));
    }
    Ok(Termination::Unestablished(format!(
        "{} engine process(es) this interface started were still running after being ended \
         outright: {}",
        left.len(),
        left.iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

/// Signals what is attributed to this interface now, and keeps reading the process table until
/// nothing is attributed to it any more or until `limit` runs out. What is returned is what the
/// last reading still found.
///
/// Every reading signals what it finds rather than only what the first one did, because the worker
/// that is being ended is a loop: the driver measures the models a build serves in batches, and one
/// batch starting while the previous one is being ended would otherwise be left running by a
/// shutdown that had already read its snapshot. `signalled` accumulates everything that was reached,
/// so the report names the engines that were ended and not only the last of them.
fn clear(
    utilities: &Utilities,
    root: u32,
    signal: &str,
    limit: Duration,
    signalled: &mut BTreeSet<u32>,
) -> Result<Vec<u32>, String> {
    let deadline = Instant::now() + limit;
    loop {
        let processes = utilities.processes()?;
        let group = processes
            .iter()
            .find(|process| process.pid == root)
            .ok_or_else(|| {
                format!(
                    "the process table reader did not report this interface itself ({root}), so \
                     which processes belong to it was not established"
                )
            })?
            .group;
        let running: Vec<u32> = attributed(&processes, root, group)
            .iter()
            .map(|process| process.pid)
            .collect();
        if running.is_empty() {
            return Ok(running);
        }
        signalled.extend(running.iter().copied());
        utilities.signal(signal, &running)?;
        if Instant::now() >= deadline {
            return Ok(running);
        }
        std::thread::sleep(OBSERVATION_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, parent: u32, group: u32, state: &str) -> Process {
        Process {
            pid,
            parent,
            group,
            state: state.to_owned(),
        }
    }

    /// The processes of a managed run are not the measurement's to end.
    ///
    /// A managed run is started in a private process group, and every process under it inherits
    /// that group. Ending a measurement while such a run is supervised must reach the engines the
    /// measurement started and nothing else: the run has its own ending, which closes its records
    /// and its slice in the kernel, and a measurement that killed it from the side would take that
    /// away and report a run that never ended as it did.
    ///
    /// The check that must fail: attribute by ancestry alone, as reading the process table without
    /// the group would. The managed process and its child are then returned beside the engine.
    #[test]
    fn a_managed_run_in_its_own_process_group_is_not_attributed_to_the_measurement() {
        let table = [
            process(100, 1, 40, "S"),    // this interface
            process(101, 100, 40, "S"),  // an engine it started while measuring
            process(102, 101, 40, "S"),  // and what that engine started
            process(200, 100, 200, "S"), // a managed run, in a private group
            process(201, 200, 200, "S"), // and what it started, which inherited that group
        ];
        assert_eq!(
            attributed(&table, 100, 40)
                .iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            vec![101, 102],
            "attribution did not separate the engines of the measurement from a managed run"
        );
    }

    /// Nothing outside this interface's own descendants is ended, however the group is shared.
    ///
    /// The interface is commonly not the leader of its process group: a shell or a test harness
    /// started it and put it there, and whatever else they started is in it too. Those are
    /// processes this interface never started, and ending them would be this product reaching
    /// outside itself.
    #[test]
    fn a_process_this_interface_did_not_start_is_not_attributed_to_it() {
        let table = [
            process(50, 1, 40, "S"),    // the shell that leads the group
            process(60, 50, 40, "S"),   // something else it started
            process(100, 50, 40, "S"),  // this interface
            process(101, 100, 40, "S"), // an engine it started
        ];
        assert_eq!(
            attributed(&table, 100, 40)
                .iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            vec![101],
            "attribution reached a process this interface did not start"
        );
    }

    /// A process that has already ended is not something still running, so it is neither signalled
    /// nor waited for. Its row stays in the table until its parent collects it, and reading that
    /// row as a live process would hold a bounded shutdown open for a process that is already gone
    /// — which is how a worker that cannot collect its own child would report a survivor it does
    /// not have.
    #[test]
    fn a_process_waiting_to_be_collected_is_not_running() {
        let table = [process(100, 1, 40, "S"), process(101, 100, 40, "Z+")];
        assert!(
            attributed(&table, 100, 40).is_empty(),
            "a process that had already ended was taken for a running engine"
        );
    }

    /// An ancestor is signalled before what stands under it.
    ///
    /// A process that is being ended may start another one on its way out, and one that has already
    /// been ended starts nothing. Reading the deeper process first would leave the ancestor free to
    /// replace it in the moment between the two signals, and the replacement — orphaned as its
    /// parent dies — carries nothing the next reading could attribute to this interface.
    ///
    /// The check that must fail: signal in the order the process table happens to report, which is
    /// by identifier. The engine started later than what it started stands second there.
    #[test]
    fn an_ancestor_is_signalled_before_what_stands_under_it() {
        let table = [
            process(300, 200, 40, "S"), // what the engine started
            process(100, 1, 40, "S"),   // this interface
            process(200, 100, 40, "S"), // the engine itself
        ];
        assert_eq!(
            attributed(&table, 100, 40)
                .iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            vec![200, 300],
            "a process was signalled before the one that is free to replace it"
        );
    }

    /// The reader this interface starts to ask the question is not part of the answer, and a line
    /// that does not carry the four values that were asked for is not read as a process at all.
    #[test]
    fn the_reader_of_the_process_table_is_left_out_of_it() {
        let reported = "  100     1    40 S\n  777   100    40 R\n  101 100 40 S extra\n";
        assert_eq!(
            parse(reported, 777)
                .iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            vec![100],
            "the process table was not read as the four values it was asked for"
        );
    }
}
