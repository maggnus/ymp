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
//! Membership of that group is what separates the two, so a run this interface is still supervising
//! is left untouched by an ending that is not its own.
//!
//! That group also holds processes this interface never started — the shell or the harness that
//! leads it, and whatever else they placed there. A process is taken as this measurement's,
//! therefore, where it stands under this interface in the process table and in this interface's own
//! process group. Both halves are read through the same admitted utilities a managed run observes
//! and ends its own processes with.
//!
//! # Why membership is recorded and not worked out again
//!
//! Both halves rest on links the operating system keeps only while both ends of them are alive.
//! When the process that started another one ends, what is left is handed to the first process on
//! the machine, and from that moment the table says nothing about where it came from. A shutdown
//! that decided membership afresh on every reading would therefore lose precisely the process that
//! outlives the one that started it — and, finding nothing left, would report an ending it had not
//! established. This is the reasoning `configure_process_group` in `ymp-runtime-api` records for
//! the managed path, and it applies here for the same reason.
//!
//! Membership is therefore written down while it can still be read: [`observe`] records every
//! process of the measurement from the moment the measurement starts, and a process stays recorded
//! until the operating system stops reporting it. Neither being handed to another parent nor
//! leaving the process group takes a process off that record, and a signal is sent by identifier,
//! which no such change affects. The identifier is held with the start time the operating system
//! reports beside it, so a record is never read as a later program the same identifier was given.
//!
//! # What this cannot reach
//!
//! A process that no reading ever saw while it still stood under this interface. That is one that
//! is started and detached inside a single interval between readings, or started by a process that
//! had already left the record. It carries nothing either reading can attribute, and the marker
//! that closes the same boundary for a managed run is installed by a launch the measurement path
//! does not enter. Nothing on the measurement path daemonises: the engines are asked for a release
//! and for the models they serve, and each one answers on the descriptor it was given.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
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
/// How often the process table is read while a measurement is running. The record only has to be
/// written before the link it is read from disappears, which happens when a process ends, so this
/// is far longer than the interval a shutdown reads at.
const RECORDING_INTERVAL: Duration = Duration::from_millis(100);

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
    /// The start time the operating system reports. It is what tells a recorded process apart from
    /// a later program the same identifier was given, so a record never ends a process this
    /// measurement did not start.
    started: String,
}

impl Process {
    fn ended(&self) -> bool {
        self.state.starts_with('Z')
    }

    /// Whether this row is the process a record holds, rather than a later one the operating system
    /// gave the same identifier.
    fn is(&self, pid: u32, started: &str) -> bool {
        self.pid == pid && self.started == started && !self.ended()
    }
}

/// The processes of the measurement that is running, recorded while the process table still says
/// they are this interface's.
#[derive(Default)]
struct Record {
    /// Each recorded process, by identifier, with the start time it was recorded under.
    members: BTreeMap<u32, String>,
    /// Whether a measurement is running and its processes are being recorded.
    recording: bool,
    /// Which recording this is. A measurement that starts while the reader of the previous one is
    /// still leaving does not gain a second reader: each reader stops when the count moves past the
    /// one it was started for.
    generation: u64,
}

fn record() -> &'static Mutex<Record> {
    static RECORD: OnceLock<Mutex<Record>> = OnceLock::new();
    RECORD.get_or_init(|| Mutex::new(Record::default()))
}

/// Start recording the processes of a measurement that is beginning.
///
/// The record is what a shutdown ends, and it is written here rather than at the shutdown because
/// the links it is read from are gone by then for exactly the process that most needs ending. A
/// reading that fails is passed over: the shutdown reads the table itself and reports what it could
/// not establish, and a measurement is not refused because the machine could not be observed at one
/// moment during it.
pub fn observe() {
    let generation = {
        let Ok(mut record) = record().lock() else {
            return;
        };
        record.members.clear();
        record.recording = true;
        record.generation = record.generation.wrapping_add(1);
        record.generation
    };
    thread::Builder::new()
        .name("ymp-measurement-processes".to_owned())
        .spawn(move || {
            let root = std::process::id();
            let Ok(utilities) = Utilities::admit() else {
                return;
            };
            loop {
                match record().lock() {
                    Ok(record) if record.recording && record.generation == generation => {}
                    _ => return,
                }
                if let Ok(processes) = utilities.processes()
                    && let Ok(mut record) = record().lock()
                {
                    absorb(&processes, root, &mut record.members);
                }
                thread::sleep(RECORDING_INTERVAL);
            }
        })
        .ok();
}

/// Stop recording and drop the record: the measurement it describes is over.
pub fn forget() {
    if let Ok(mut record) = record().lock() {
        record.recording = false;
        record.members.clear();
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
        verify_admitted_programs(&self.admitted).map_err(|error| {
            format!(
                "the utility that reads the process table changed after it was admitted: {error}"
            )
        })?;
        let reader = Command::new(&self.table)
            .args(["-A", "-o", "pid=,ppid=,pgid=,state=,lstart="])
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

/// Reads the utility's answer into the rows it is made of. A line that does not carry the values
/// that were asked for is not read at all, so a reader that answers something else is not silently
/// taken for a shorter process table.
///
/// Its own reader is left out: it is a process this interface started, in this interface's group,
/// and it is already leaving.
fn parse(reported: &str, asking: u32) -> Vec<Process> {
    let mut processes = Vec::new();
    for line in reported.lines() {
        let mut fields = line.split_whitespace();
        let (Some(pid), Some(parent), Some(group), Some(state)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let (Ok(pid), Ok(parent), Ok(group)) = (pid.parse(), parent.parse(), group.parse()) else {
            continue;
        };
        // The start time the utility reports carries spaces, so it is what remains of the line.
        let started = fields.collect::<Vec<_>>().join(" ");
        if started.is_empty() || pid <= 1 || pid == asking {
            continue;
        }
        processes.push(Process {
            pid,
            parent,
            group,
            state: state.to_owned(),
            started,
        });
    }
    processes
}

/// Records every process that now stands under this interface in its own process group, and drops
/// the records the operating system no longer reports.
///
/// Dropping is what keeps an identifier from outliving the process it was recorded for: once the
/// operating system stops reporting that process, the identifier is free to be given to another
/// program, and a record kept past that point would end a program this measurement never started.
fn absorb(processes: &[Process], root: u32, members: &mut BTreeMap<u32, String>) {
    members.retain(|pid, started| {
        processes
            .iter()
            .any(|process| process.is(*pid, started.as_str()))
    });
    let Some(group) = processes
        .iter()
        .find(|process| process.pid == root)
        .map(|process| process.group)
    else {
        return;
    };
    for process in attributed(processes, root, group, members) {
        members.insert(process.pid, process.started);
    }
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
/// What is standing under an already recorded process is this measurement's too, so the walk starts
/// from the record as well as from `root`: a process the record still holds keeps carrying what it
/// starts, whatever the operating system now says about the record's own parent.
fn attributed(
    processes: &[Process],
    root: u32,
    group: u32,
    members: &BTreeMap<u32, String>,
) -> Vec<Process> {
    let mut descended: BTreeMap<u32, ()> = BTreeMap::from([(root, ())]);
    for (pid, started) in members {
        if processes.iter().any(|process| process.is(*pid, started)) {
            descended.insert(*pid, ());
        }
    }
    loop {
        let known = descended.len();
        for process in processes {
            if descended.contains_key(&process.parent) {
                descended.insert(process.pid, ());
            }
        }
        if descended.len() == known {
            break;
        }
    }
    processes
        .iter()
        .filter(|process| {
            process.pid != root
                && process.group == group
                && descended.contains_key(&process.pid)
                && !process.ended()
        })
        .cloned()
        .collect()
}

/// The recorded processes the operating system still reports, an ancestor before anything standing
/// under it.
///
/// The order matters when the signal is delivered. A process that is ending is free to start
/// another one first — the driver reads the models a build serves in batches, and a shell in the
/// same position starts its next child when the previous one dies — and a process that has already
/// been ended starts nothing. Reaching the ancestor first therefore stops the succession rather
/// than racing it, while everything already standing under it is signalled from identifiers this
/// same reading holds. A recorded process whose parent is not recorded stands first: nothing among
/// these can end it, so nothing among these has to precede it.
fn targets(processes: &[Process], members: &BTreeMap<u32, String>) -> Vec<u32> {
    let held: Vec<&Process> = processes
        .iter()
        .filter(|process| {
            members
                .get(&process.pid)
                .is_some_and(|started| process.is(process.pid, started))
        })
        .collect();
    let mut depths: BTreeMap<u32, u32> = held
        .iter()
        .filter(|process| !members.contains_key(&process.parent))
        .map(|process| (process.pid, 0))
        .collect();
    loop {
        let known = depths.len();
        for process in &held {
            if let Some(depth) = depths.get(&process.parent).copied() {
                depths.entry(process.pid).or_insert(depth + 1);
            }
        }
        if depths.len() == known {
            break;
        }
    }
    let mut ordered: Vec<u32> = held.iter().map(|process| process.pid).collect();
    // A process in a cycle of records the walk never reached is still ended; it is placed last
    // rather than left out, because being unreachable is not being gone.
    ordered.sort_by_key(|pid| (depths.get(pid).copied().unwrap_or(u32::MAX), *pid));
    ordered
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
    let mut signalled = BTreeMap::new();

    let left = clear(&utilities, root, "-TERM", REQUEST_LIMIT, &mut signalled)?;
    if signalled.is_empty() {
        return Ok(Termination::NothingRunning);
    }
    let left = match left.is_empty() {
        true => left,
        false => clear(&utilities, root, "-KILL", ENFORCEMENT_LIMIT, &mut signalled)?,
    };
    if left.is_empty() {
        return Ok(Termination::Ended(signalled.into_keys().collect()));
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

/// Signals every recorded process the operating system still reports, and keeps reading the table
/// until the record holds nothing that is still running or until `limit` runs out. What is returned
/// is what the last reading still found.
///
/// Each reading records again before it signals, for two reasons that are one rule. A process that
/// started since the previous reading is recorded and ended with the rest, because the worker being
/// ended is a loop: the driver reads the models a build serves in batches, and a batch beginning
/// while the previous one is ended would otherwise be released by a shutdown working from a
/// snapshot. And a process whose parent has since ended stays recorded, because it was recorded
/// while the table still said whose it was — which is the case a reading on its own can no longer
/// establish. `signalled` accumulates everything that was reached, so the report names the engines
/// that were ended and not only the last of them.
fn clear(
    utilities: &Utilities,
    root: u32,
    signal: &str,
    limit: Duration,
    signalled: &mut BTreeMap<u32, ()>,
) -> Result<Vec<u32>, String> {
    let deadline = Instant::now() + limit;
    loop {
        let processes = utilities.processes()?;
        if !processes.iter().any(|process| process.pid == root) {
            return Err(format!(
                "the process table reader did not report this interface itself ({root}), so which \
                 processes belong to it was not established"
            ));
        }
        let running = {
            let mut record = record().lock().map_err(|_| {
                "the record of this measurement's processes failed, so nothing about them was \
                 established"
                    .to_owned()
            })?;
            absorb(&processes, root, &mut record.members);
            targets(&processes, &record.members)
        };
        if running.is_empty() {
            return Ok(running);
        }
        signalled.extend(running.iter().map(|pid| (*pid, ())));
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

    const STARTED: &str = "Mon Jan  1 00:00:00 2035";

    fn process(pid: u32, parent: u32, group: u32, state: &str) -> Process {
        Process {
            pid,
            parent,
            group,
            state: state.to_owned(),
            started: STARTED.to_owned(),
        }
    }

    /// The record as it stands after reading this table, from an empty record.
    fn recorded(table: &[Process], root: u32) -> BTreeMap<u32, String> {
        let mut members = BTreeMap::new();
        absorb(table, root, &mut members);
        members
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
            recorded(&table, 100).into_keys().collect::<Vec<_>>(),
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
            recorded(&table, 100).into_keys().collect::<Vec<_>>(),
            vec![101],
            "attribution reached a process this interface did not start"
        );
    }

    /// A process the record already holds stays this measurement's after the one that started it
    /// has ended.
    ///
    /// Once its parent is gone the process is handed to the first process on the machine, and the
    /// table no longer says where it came from. What says so is that a reading already said so,
    /// while both ends of that link were alive.
    ///
    /// The check that must fail: work membership out afresh from each reading, which is what a
    /// shutdown reading the table alone would do. The process is then this measurement's before its
    /// parent ends and nobody's afterwards, so a quit ends what is easy to reach and reports an
    /// ending for what it lost.
    #[test]
    fn a_recorded_process_stays_recorded_once_the_one_that_started_it_has_ended() {
        let engine_running = [
            process(100, 1, 40, "S"),   // this interface
            process(101, 100, 40, "S"), // the engine
            process(102, 101, 40, "S"), // what the engine started
        ];
        let mut members = recorded(&engine_running, 100);
        assert_eq!(
            members.keys().copied().collect::<Vec<_>>(),
            vec![101, 102],
            "the engine and what it started were not recorded while both were reachable"
        );

        // The engine ends on the request to end; what it started declines, is handed to the first
        // process on the machine, and takes a process group of its own — so neither half of what a
        // reading attributes by still holds. The record is what holds instead.
        let engine_gone = [process(100, 1, 40, "S"), process(102, 1, 102, "S")];
        absorb(&engine_gone, 100, &mut members);
        assert_eq!(
            targets(&engine_gone, &members),
            vec![102],
            "the process that outlived the engine stopped being this measurement's to end"
        );
    }

    /// A record is dropped once the operating system stops reporting the process it was written
    /// for, so an identifier given to another program afterwards is not ended in its place.
    #[test]
    fn a_record_does_not_outlive_the_process_it_was_written_for() {
        let running = [process(100, 1, 40, "S"), process(101, 100, 40, "S")];
        let mut members = recorded(&running, 100);

        let mut reused = process(101, 1, 40, "S");
        reused.started = "Mon Jan  1 00:00:01 2035".to_owned();
        let after = [process(100, 1, 40, "S"), reused];
        absorb(&after, 100, &mut members);
        assert!(
            targets(&after, &members).is_empty(),
            "a program that was only given the same identifier was left to be ended"
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
            recorded(&table, 100).is_empty(),
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
            targets(&table, &recorded(&table, 100)),
            vec![200, 300],
            "a process was signalled before the one that is free to replace it"
        );
    }

    /// The reader this interface starts to ask the question is not part of the answer, and a line
    /// that carries no start time beside the rest is not read as a process at all.
    #[test]
    fn the_reader_of_the_process_table_is_left_out_of_it() {
        let reported = "  100     1    40 S    Mon Jan  1 00:00:00 2035\n  \
                        777   100    40 R    Mon Jan  1 00:00:00 2035\n  101 100 40 S\n";
        assert_eq!(
            parse(reported, 777)
                .iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            vec![100],
            "the process table was not read as the values it was asked for"
        );
    }
}
