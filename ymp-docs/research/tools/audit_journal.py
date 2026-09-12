#!/usr/bin/env python3
"""Read a consistent SQLite snapshot and export aggregate, content-free evidence."""
import argparse
import collections
import datetime as dt
import json
import random
import sqlite3
import statistics
from pathlib import Path


def category(error):
    value = error.lower()
    for label, terms in [
        ('cancellation', ['cancelled', 'canceled']),
        ('timeout', ['timed out', 'timeout']),
        ('capacity_or_rate_limit', ['overloaded', 'rate limit', '429']),
        ('authentication', ['unauthorized', 'authentication', '401']),
        ('protocol', ['invalid json', 'protocol', 'missing thread', 'missing session']),
        ('process', ['process exited', 'cannot start']),
    ]:
        if any(term in value for term in terms):
            return label
    return 'unclassified'


def summarize(source):
    db = sqlite3.connect(':memory:')
    with sqlite3.connect(source.resolve().as_uri() + '?mode=ro', uri=True) as original:
        original.backup(db)
    db.row_factory = sqlite3.Row
    sessions = {r['id']: json.loads(r['data']) for r in db.execute('SELECT id,data FROM sessions ORDER BY rowid')}
    labels = {id: f'S{n + 1:02}' for n, id in enumerate(sessions)}
    events = []
    for row in db.execute('SELECT * FROM events ORDER BY seq'):
        events.append({**dict(row), 'data': json.loads(row['data'])})
    starts, ends = {}, {}
    for event in events:
        if event['kind'] not in ('turn_started', 'turn_completed', 'turn_failed'):
            continue
        key = event['session_id'], event['data']['turn']
        (starts if event['kind'] == 'turn_started' else ends)[key] = event
    usage = {}
    if db.execute("SELECT 1 FROM sqlite_master WHERE name='token_usage'").fetchone():
        usage = {(r['session_id'], r['turn']): json.loads(r['snapshot']) if r['snapshot'] else None for r in db.execute('SELECT * FROM token_usage')}
    phases = collections.defaultdict(lambda: dict(started=0, completed=0, failed=0, open=0, known_tokens=0, reported=0, partial=0, durations_seconds=[]))
    for key, start in starts.items():
        item = phases[start['data'].get('purpose', 'unknown')]
        item['started'] += 1
        end = ends.get(key)
        item['completed' if end and end['kind'] == 'turn_completed' else 'failed' if end else 'open'] += 1
        if end:
            item['durations_seconds'].append((dt.datetime.fromisoformat(end['created_at']) - dt.datetime.fromisoformat(start['created_at'])).total_seconds())
        snapshot = usage.get(key)
        if snapshot:
            counts = snapshot['counts']
            if counts.get('input') is not None or counts.get('output') is not None:
                item['reported'] += 1
                item['known_tokens'] += (counts.get('input') or 0) + (counts.get('output') or 0)
            item['partial'] += bool(snapshot['partial'] or not snapshot['finalized'])
    for item in phases.values():
        durations = item.pop('durations_seconds')
        item['summed_invocation_seconds'] = round(sum(durations), 3)
        item['median_invocation_seconds'] = round(statistics.median(durations), 3) if durations else None
    summaries = []
    for id, session in sessions.items():
        selected = [e for e in events if e['session_id'] == id]
        begun = [e for e in selected if e['kind'] == 'turn_started']
        first_execution = next((e['seq'] for e in begun if e['data'].get('purpose') == 'execute'), None)
        model_settings = [a.get('model') for a in session['team']]
        summaries.append(dict(label=labels[id], status=session['status'], team_size=len(session['team']),
                              native_models_explicit=sum(m is not None for m in model_settings), turns_used=session['turns_used'], started=len(begun),
                              phases=dict(collections.Counter(e['data'].get('purpose', 'unknown') for e in begun)),
                              calls_before_first_execute=sum(e['seq'] < first_execution for e in begun) if first_execution else None,
                              check_events=sum(e['kind'] == 'check' for e in selected),
                              failed_checks=sum(e['kind'] == 'check' and not e['data'].get('success', False) for e in selected),
                              right_censored=session['status'] == 'running'))
    choices = [e['data'] for e in events if e['kind'] == 'assignment_choice']
    replay = []
    rng = random.Random(56)
    for index, choice in enumerate(choices):
        parameters = [(s['successes'] + 1, s['failures'] + 1) for s in choice['scores']]
        selected = next(i for i,s in enumerate(choice['scores']) if s['agent'] == choice['selected'])
        wins = 0
        draws = 10000
        for _ in range(draws):
            values = [rng.betavariate(a,b) for a,b in parameters]
            wins += max(range(len(values)),key=values.__getitem__) == selected
        replay.append(dict(choice=index+1,candidates=len(parameters),beta_parameters=parameters,selected_index=selected,
                           selected_propensity_mc=wins/draws,draws=draws,competence=choice['competence'],difficulty=choice['difficulty']))
    observations = [dict(r) for r in db.execute('SELECT competence,difficulty,success,COUNT(*) AS count FROM observations GROUP BY competence,difficulty,success')]
    memory = [json.loads(r['data']) for r in db.execute('SELECT data FROM memory')]
    return dict(snapshot_at=dt.datetime.now(dt.timezone.utc).isoformat(), schema=db.execute('PRAGMA user_version').fetchone()[0],
                max_event_seq=max((e['seq'] for e in events), default=0), source='Existing local application journal; consistent read-only SQLite backup',
                event_counts=dict(collections.Counter(e['kind'] for e in events)), sessions=summaries, phases=dict(phases),
                failures=dict(collections.Counter(category(e['data'].get('error', '')) for e in events if e['kind'] == 'turn_failed')),
                assignment=dict(count=len(choices), cold_prior_choices=sum(all(s['successes'] == 0 and s['failures'] == 0 for s in e['scores']) for e in choices),
                                selected_with_posterior=sum(any(s['agent'] == e['selected'] and (s['successes'] or s['failures']) for s in e['scores']) for e in choices),
                                decisions_with_logged_propensity=sum('propensity' in e for e in choices),
                                decisions_with_task_id=sum('task_id' in e for e in choices),
                                reconstructed_propensities=replay,
                                propensity_caveat='Reconstructed from logged Beta parameters under the current selection rule; sampled scores are not action probabilities. This does not identify counterfactual rewards or costs.',
                                decisions_with_candidate_costs=sum(any('cost' in s for s in e['scores']) for e in choices)),
                observations=observations,
                memory=dict(count=len(memory), statuses=dict(collections.Counter(m['status'] for m in memory)), supersedes_links=sum(bool(m.get('supersedes')) for m in memory)),
                caveats=['Historical usage may be partial; known_tokens is a reported lower bound.',
                         'Invocation durations are not additive wall-clock latency when calls overlap.',
                         'Completion statuses are product outcomes, not independent benchmark labels.',
                         'An open run is right-censored; no missing result is treated as a failure.',
                         'No prompts, message bodies, check output, file contents, credentials, or native session IDs are exported.'])


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--database', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    evidence = summarize(args.database)
    args.output.write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps({'snapshot_at': evidence['snapshot_at'], 'sessions': len(evidence['sessions']), 'phases': evidence['phases'], 'assignment': evidence['assignment'], 'failures': evidence['failures']}))
