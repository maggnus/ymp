# Registry and readiness observations

W1-0003 provides the Registry consumer and the metadata/probe contracts needed by
later executors. Canonical task status is in `tasks/records/W1-0003.json`.

## Identity and unknown metadata

`ymp-domain/src/identity.rs` keeps Agent, Provider, ModelOffering and
ExecutionProfile distinct. An Agent has a stable ID, instructions, enabled flag
and defaults; it has no permanent role or assignment authority. Each model owns
its native effort set. Optional version, model, family and effort data remains
unknown when not observed. An empty known effort set and an unknown set are
separate values. An offering without a concrete model stays visible in Pool but
cannot resolve to an execution profile.

`ProfileSettings` represents native model/effort values without a shared effort
scale. `InvocationSettings` contains independent requested, sent and reported
values: constructing a request leaves sent/reported unknown, and setting sent
values does not fill reported fields. W1-0017 adds their real invocation lifecycle.

Discovery records preserve their method, time and Native/ScriptedFixture source.
Source and provider kind must agree in both directions: Scripted cannot claim a
native observation, and a native provider cannot claim fixture readiness. The
NativeDiscovery trait is a metadata-only adapter contract. Concrete Codex
transport/discovery belongs to W1-0018; Claude and Glm have their own later tasks.
A represented provider kind does not establish an available executor.

## Static observations and replaceable policy

`StaticDependencyProbe::capture` inspects an explicit absolute dependency path.
It distinguishes missing, unreadable, wrong-kind and non-executable entries. On
macOS/Linux an executable prerequisite must be a file with an executable mode
bit. Capture neither executes the program nor reads authentication. This is a
static prerequisite check, not a guarantee that a future process will start.
The execution host must handle later drift and launch failures separately.

The runtime obtains dependency facts before policy evaluation. ReadinessProbe
receives an immutable profile-specific ReadinessView and returns a Proposal.
`readiness_response` binds the profile and input digest before invoking the probe.
The kernel matches responses by profile and checks digests, uniqueness and complete
coverage; reordered responses cannot be assigned to another profile.

StaticDependencyProbe version 1 has an empty parameter object. ParameterSchemas
binds that schema to its exact port/implementation/version. The contract test also
registers a deterministic maintenance policy with its own parameter schema. Kernel
hard exclusions remain effective even if a probe proposes Ready: disabled agent,
missing provider or adapter, unknown backend capabilities, conflicting pins and
unavailable dependencies cannot be overridden. Native readiness additionally
requires an executable observation. A stricter policy can exclude an otherwise
eligible profile.

## Recorded Pool and selection

Application exposes registry_input and record_pool around the Registry consumer;
strategy invocation stays outside the kernel. PoolRecorded retains the complete
input facts and constraints, the selected policy and parameters, profile-bound
responses, their kernel outcomes and the resulting Pool. Replay validates those
relationships without querying files or providers. The event has a definition and
rationale in model section 9 because these Registry decisions require the same
D-2/D-5/R-17 attribution as other decisions.

An agent is eligible if any discovered profile satisfies its constraints and
readiness rules. Invalid defaults alone do not hide other usable profiles.
Registry.profile resolves an explicit model/effort first, then agent defaults,
then observed native defaults. It denies unsupported settings or excluded
profiles. It never selects the first model arbitrarily or copies a request into
reported metadata. Family/version may stay unknown in the profile; a later
consumer requiring them must explicitly refuse or record its permitted fallback.

Pool/profile reads use the recorded snapshot. Changing task constraints makes it
stale and requires a new observation; it does not rewrite history. The source time
remains visible. There is no automatic refresh during painting or an invented
expiration period. ReadinessProbe selection must match the session's initial
selection; record_pool cannot silently switch a policy.

Registry.capabilities takes an explicit WorkspaceCapabilities observation bound
to the exact ExecutionProfile. Unknown workspace enforcement or a different
profile is denied. Effective capabilities are the intersection of backend
capabilities, workspace enforcement and user constraints. All nine model
capabilities are represented. These observation inputs are not OS isolation:
W1-0005 supplies the concrete WorkspaceGuard enforcement, and W1-0006 validates
admission against it. A Pool observation itself issues no Grant or Assignment.

## Evidence and limits

`ymp-runtime/tests/registry.rs` exercises Registry through Application, including:

- per-model native settings, bad defaults with valid overrides, unknown metadata
  and exact replay of source facts and decisions;
- different probe implementations and schemas, with kernel exclusions intact;
- backend/workspace/user capability intersection and unknown enforcement;
- stale snapshots after owner refinements and the resulting pin enforcement;
- atomic refusal of duplicate inputs and forged Pool outcomes;
- real temporary-filesystem dependency inspection with an executable marker that
  remains absent, plus missing, wrong-kind and unreadable paths;
- both directions of native/fixture separation, reordered responses and malformed
  response bindings, and independent requested/sent/reported values.

The integration uses explicit Scripted identity/offerings fixtures. It does not
establish native inference, concrete executor availability, billing or workspace
isolation. The frozen W1-0001 event fixtures remain readable after this extension.
