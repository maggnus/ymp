# Distinct owner authorization for new work from current files

The owner-approved `recovery-owner-decision.md` supersedes the earlier temporary
production refusal only for an explicit, scoped authorization. Parent originals
remain unchanged. No real session/data or native inference is authorized here.

The implementation keeps four independent facts:

1. A fresh immutable-plan verdict can satisfy the outstanding review obligation,
   while leaving the old failures and unknown effects intact.
2. Continue can consume that recorded verdict; it issues no manual retry permit.
3. ReleaseHold restores the condition preceding an explicit stage Wait/Pause,
   without admitting a call, resolving effects or authorizing a retry. Session
   holds use the existing owner-team Continue command.
4. Continue with current files acknowledges exact known-ended historical failures
   and authorizes ordinary new work. It is a separate durable owner record with
   a context/revision-bound receipt. It neither changes effect_resolution nor
   makes Retry legal for an old uncertain invocation.

The reviewable context binds the session, saved stage/proposal/result, stage and
team revisions, current task-board version, exact acknowledged failures and a
bounded metadata digest of the workspace's existing file listing. File contents
are hashed using the shared file-evidence adapter; source copies are not stored.
The command revalidates that context under session/project ownership before an
atomic commit. Native calls and production are separate ordinary operations.

The authorization removes only the acknowledged failures from the new-work
admission refusal. Historical uncertainty stays visible. Later uncertain failures
remain uncovered and must stop further new work until separately addressed.
Fresh assignments and native contexts prevent resurrection of old native authority.
Membership, independence, access serialization, budgets and usage are unchanged.

Required controls retain the no-authorization refusal, known/unknown termination,
owner hold release, stale/foreign commands and changed workspace context,
idempotent reopen, normal resource denial, actual task execution without duplicate
planning, and refusal after a later unacknowledged uncertain failure.
