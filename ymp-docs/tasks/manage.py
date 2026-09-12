#!/usr/bin/env python3
"""Maintain the task register and its Markdown views; never starts an agent."""
import argparse
import collections
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REGISTER = ROOT / 'tasks.json'
STATES = {
    'planned': ('[ ]', 'Planned; ready when dependencies are complete'),
    'in_progress': ('[~]', 'In progress'),
    'owner_question': ('[?]', 'Waiting for an explicit owner answer'),
    'done': ('[x]', 'Completed'),
    'rejected': ('[!]', 'Rejected'),
    'paused': ('[=]', 'Paused or waiting for dependencies'),
    'new': ('[+]', 'New; not yet scheduled'),
}
STATUSES = tuple(STATES)
ALIASES = {'proposed': 'planned', 'ready': 'planned', 'blocked': 'paused', 'parked': 'paused'}
ALIASES.update({marker: state for state, (marker, _) in STATES.items()})


def last_update(task):
    history = task.get('history', [])
    return history[-1] if history else {'at': '', 'note': ''}


def timestamp(value):
    if not value:
        return 'Not recorded'
    return datetime.fromisoformat(value.replace('Z', '+00:00')).astimezone(timezone.utc).strftime('%Y-%m-%d %H:%M')


def unmet(task, by_id):
    return [key for key in task['depends_on'] if by_id[key]['status'] != 'done']


def display_state(task, by_id):
    if task['status'] == 'planned' and unmet(task, by_id):
        return 'paused'
    return task['status']


def marker(task, by_id):
    return STATES[display_state(task, by_id)][0]


def reason(task, by_id):
    if task['status'] == 'planned':
        missing = unmet(task, by_id)
        return 'Needs ' + ', '.join(missing) if missing else 'Ready to start'
    return last_update(task)['note'] or STATES[task['status']][1]


def cell(value):
    return str(value).replace('|', '\\|').replace('\n', ' ')


def task_link(task, page=''):
    return f"[{task['id']}]({page}#{task['id'].lower()})"


def validate(data):
    if data.get('schema_version') != 2:
        raise ValueError('Unsupported task-register schema version')
    tasks = data['tasks']
    ids = [t['id'] for t in tasks]
    if len(ids) != len(set(ids)):
        raise ValueError('Duplicate task ID')
    by_id = {t['id']: t for t in tasks}
    for task in tasks:
        if task['status'] not in STATES:
            raise ValueError('Invalid status: ' + task['id'])
        if not task['title'] or not task['acceptance']:
            raise ValueError('Missing task definition: ' + task['id'])
        for dep in task['depends_on']:
            if dep not in by_id:
                raise ValueError('Unknown dependency: ' + dep)
        if task['status'] == 'done' and not task['evidence']:
            raise ValueError('Completion requires evidence: ' + task['id'])
        if task['status'] in ('owner_question', 'paused'):
            update = last_update(task)
            if not update['note'].strip() or update.get('status') != task['status']:
                raise ValueError('An owner question or pause needs a current reason: ' + task['id'])
    visited, visiting = set(), set()
    def visit(key):
        if key in visiting:
            raise ValueError('Dependency cycle at ' + key)
        if key in visited:
            return
        visiting.add(key)
        for dep in by_id[key]['depends_on']:
            visit(dep)
        visiting.remove(key)
        visited.add(key)
    for key in ids:
        visit(key)
    for task in tasks:
        if task['status'] in ('in_progress', 'done') and unmet(task, by_id):
            raise ValueError('Started or completed task has unfinished dependencies: ' + task['id'])
    order = data.get('delivery_order', [])
    if len(order) != len(set(order)):
        raise ValueError('Duplicate delivery task')
    position = {key: i for i, key in enumerate(order)}
    for key in order:
        if key not in by_id:
            raise ValueError('Unknown delivery task: ' + key)
        for dep in by_id[key]['depends_on']:
            if dep in position and position[dep] >= position[key]:
                raise ValueError('Delivery order precedes dependency: ' + key)
    milestones = data.get('milestones', [])
    if len({m['id'] for m in milestones}) != len(milestones):
        raise ValueError('Duplicate milestone ID')
    assigned = set()
    for milestone in milestones:
        if not milestone['outcome'] or not milestone['task_ids']:
            raise ValueError('Missing milestone definition: ' + milestone['id'])
        for key in milestone['task_ids']:
            if key not in by_id or key in assigned:
                raise ValueError('Unknown or repeated milestone task: ' + key)
            assigned.add(key)
    if milestones and any(key not in assigned for key in order):
        raise ValueError('Delivery task has no milestone')
    coverage = data.get('intent_coverage', [])
    if len({c['clause'] for c in coverage}) != len(coverage):
        raise ValueError('Duplicate intent coverage clause')
    for clause in coverage:
        if not clause['task_ids'] or any(key not in by_id for key in clause['task_ids']):
            raise ValueError('Missing or unknown coverage task: ' + clause['clause'])
    if data.get('intent_sha256'):
        source = (ROOT.parents[1] / data['intent']).read_bytes()
        if hashlib.sha256(source).hexdigest() != data['intent_sha256']:
            raise ValueError('Approved intent changed; reconcile the plan before updating its digest')
        text = source.decode()
        goals = text.split('## Цели\n', 1)[1].split('\n## ', 1)[0]
        principles = text.split('## Принципы\n', 1)[1].split('\n## ', 1)[0]
        def count(section):
            return sum(line.startswith('- ') for line in section.splitlines())
        expected = {f'G{i}' for i in range(1, count(goals) + 1)}
        expected |= {f'P{i:02}' for i in range(1, count(principles) + 1)}
        if {c['clause'] for c in coverage} != expected:
            raise ValueError('Intent coverage does not match approved goals and principles')
    return by_id


def write_atomic(path, text):
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(text)
    temporary.replace(path)


def render_plan(data):
    by_id = {t['id']: t for t in data['tasks']}
    delivery = [by_id[key] for key in data.get('delivery_order', [])]
    counts = collections.Counter(display_state(t, by_id) for t in delivery)
    completed = counts['done']
    total = sum(t['status'] != 'rejected' for t in delivery)
    lines = ['# Delivery plan and progress', '',
             'Updated: ' + timestamp(data['updated_at']) + ' UTC.', '',
             '[Approved intent](../../intent.md) · [Task details and evidence](README.md) · [Canonical register](tasks.json)', '',
             'Generated by [manage.py](manage.py) from tasks.json. Status changes update this plan and the detailed register together. All timestamps are UTC.', '',
             f"**Delivery: {completed}/{total} tasks complete.** {counts['in_progress']} in progress; {counts['planned']} ready; {counts['owner_question']} need an owner answer; {counts['paused']} paused or waiting for dependencies.", '',
             'Delivery counts exclude completed research and planning work. They are task counts, not an estimate of effort or time.', '']
    if release := data.get('release'):
        final_task = by_id[release['final_verification_task']]
        lines += [f"Target release: **{release['target_version']}**. Final acceptance: {task_link(final_task, 'README.md')}, with independent review at **{release['final_verification_reasoning']}** reasoning. The version is not released until that task is complete.", '']
    lines += ['## Status key', '', '| Marker | Meaning |', '| --- | --- |']
    lines += [f'| `{symbol}` | {meaning} |' for symbol, meaning in STATES.values()]
    lines += ['', 'A planned task automatically shows `[=]` while prerequisites remain unfinished and returns to `[ ]` when they are complete. `[?]` is reserved for an explicit unresolved owner question recorded in the latest note.', '',
              '## Current work and owner questions', '']
    attention = [t for t in data['tasks'] if t['status'] in ('in_progress', 'owner_question', 'new')]
    for task in attention:
        lines += [f"- `{marker(task, by_id)}` {task_link(task, 'README.md')} — {task['title']} ({timestamp(last_update(task)['at'])}). {last_update(task)['note']}"]
    if not attention:
        lines.append('No active work, owner questions or unscheduled additions.')
    lines += ['', '## Ready next', '']
    ready = [t for t in delivery if display_state(t, by_id) == 'planned']
    lines += [f"- `[ ]` {task_link(t, 'README.md')} — {t['title']}" for t in ready] or ['No delivery task is ready. See the dependency reasons below.']
    for milestone in data.get('milestones', []):
        group = [by_id[key] for key in milestone['task_ids']]
        done = sum(t['status'] == 'done' for t in group)
        if not any(t in delivery for t in group):
            continue
        lines += ['', f"## {milestone['id']} — {milestone['title']}", '',
                  f"{done}/{len(group)} complete. {milestone['outcome']}", '',
                  '| State | Task | Last update | Dependency or note |', '| --- | --- | --- | --- |']
        for t in group:
            lines.append(f"| `{marker(t, by_id)}` | {task_link(t, 'README.md')} — {cell(t['title'])} | {timestamp(last_update(t)['at'])} | {cell(reason(t, by_id))} |")
    lines += ['', '## Working approach', '']
    lines += ['- ' + note for note in data.get('plan_notes', [])]
    lines += ['', '## Planning and deferred work', '']
    for milestone in data.get('milestones', []):
        group = [by_id[key] for key in milestone['task_ids']]
        if any(t in delivery for t in group):
            continue
        done = sum(t['status'] == 'done' for t in group)
        lines += [f"{milestone['title']}: {done}/{len(group)} complete. " + ', '.join(task_link(t, 'README.md') for t in group) + '.']
    deferred = [t for t in data['tasks'] if t['status'] == 'paused' and t not in delivery]
    lines += ['', 'Deferred options and quota-dependent studies:']
    lines += [f"- `[=]` {task_link(t, 'README.md')} — {t['title']}. {last_update(t)['note']}" for t in deferred] or ['None.']
    lines += ['', '## Recent updates', '', '| Time (UTC) | Task | State | Update |', '| --- | --- | --- | --- |']
    recent = sorted(data['tasks'], key=lambda t: last_update(t)['at'], reverse=True)[:5]
    for t in recent:
        lines.append(f"| {timestamp(last_update(t)['at'])} | {task_link(t, 'README.md')} | `{marker(t, by_id)}` | {cell(last_update(t)['note'])} |")
    lines += ['', '## Maintenance', '',
              'Update a task with a concise progress note, pause reason or the exact owner question:', '',
              '```sh', 'python3 ymp-docs/tasks/manage.py status YMP-116 in_progress --note "Drafting the transition table"',
              'python3 ymp-docs/tasks/manage.py render', 'python3 ymp-docs/tasks/manage.py check', '```', '',
              'Canonical status names: ' + ', '.join(f'`{s}`' for s in STATES) + '. Quoted markers such as `"[~]"` are also accepted. `new` means added but not yet scheduled. Use the same status with a new note to record progress; history is retained.', '',
              'Approved intent SHA-256: `' + data.get('intent_sha256', 'not recorded') + '`. Implementation baseline: `' + data.get('implementation_baseline', data['baseline']) + '`.', '']
    return '\n'.join(lines)


def render(data, save=True):
    by_id = {t['id']: t for t in data['tasks']}
    lines = ['# Task details and evidence', '', 'Updated: ' + timestamp(data['updated_at']) + ' UTC.', '',
             'Use the [delivery plan](plan.md) to follow progress. This generated register contains full acceptance criteria, dependencies and evidence. [tasks.json](tasks.json) is the single source; [manage.py](manage.py) updates both pages.', '',
             'A planned task is not implemented functionality. Completed research and planning are separate from product delivery. Real-provider checks and experiments require their recorded quota authorization.', '',
             '## Work by type', '', '| Type | Completed | Total |', '| --- | ---: | ---: |']
    for kind in sorted({t['type'] for t in data['tasks']}):
        group = [t for t in data['tasks'] if t['type'] == kind]
        lines.append(f"| {kind} | {sum(t['status'] == 'done' for t in group)} | {len(group)} |")
    lines += ['', '## Index', '', '| State | ID | Priority | Task | Last update (UTC) |', '| --- | --- | --- | --- | --- |']
    for t in data['tasks']:
        lines.append(f"| `{marker(t, by_id)}` | {task_link(t)} | {t['priority']} | {cell(t['title'])} | {timestamp(last_update(t)['at'])} |")
    for t in data['tasks']:
        lines += ['', f"## {t['id']}", '', t['title'], '',
                  f"**State:** `{marker(t, by_id)}` ({t['status']}) · **Type:** {t['type']} · **Priority:** {t['priority']}", '',
                  '**Last update (UTC):** ' + timestamp(last_update(t)['at']), '',
                  '**Current reason:** ' + reason(t, by_id), '',
                  '**Owner:** ' + t['owner'], '', '**Authorization:** ' + t['authorization'], '',
                  '**Depends on:** ' + (', '.join(task_link(by_id[key]) for key in t['depends_on']) or 'None'), '',
                  t['reason'], '']
        if t['status'] == 'planned' and last_update(t)['note']:
            lines += ['**Latest progress note:** ' + last_update(t)['note'], '']
        lines += ['**Acceptance criteria:**', '']
        lines += ['- ' + a for a in t['acceptance']]
        lines += ['', '**Evidence:**', '']
        lines += ['- ' + e for e in t['evidence']] or ['- Pending.']
        if t.get('budget'):
            lines += ['', '**Quota:** ' + t['budget']]
    if data.get('intent_coverage'):
        lines += ['', '## Intent coverage', '',
                  'G and P identifiers follow the goals and principles in the approved intent. This maps planned work, not implemented behavior.', '',
                  '| Clause | Requirement | Tasks |', '| --- | --- | --- |']
        for clause in data['intent_coverage']:
            links = ', '.join(task_link(by_id[key]) for key in clause['task_ids'])
            lines.append(f"| {clause['clause']} | {clause['requirement']} | {links} |")
    lines.append('')
    result = '\n'.join(lines)
    if save:
        write_atomic(ROOT / 'README.md', result)
        write_atomic(ROOT / data.get('work_plan', 'plan.md'), render_plan(data))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    for name in ('list', 'check', 'render'):
        sub.add_parser(name)
    status = sub.add_parser('status')
    status.add_argument('id')
    status.add_argument('status', choices=tuple(STATES) + tuple(ALIASES))
    status.add_argument('--note', required=True)
    args = parser.parse_args()
    data = json.loads(REGISTER.read_text())
    by_id = validate(data)
    if args.command == 'status':
        if args.id not in by_id:
            parser.error('Unknown task: ' + args.id)
        if not args.note.strip():
            parser.error('A progress note, pause reason or owner question is required')
        task = by_id[args.id]
        if args.status == 'ready' and unmet(task, by_id):
            parser.error('This task still has unfinished dependencies')
        task['status'] = ALIASES.get(args.status, args.status)
        at = datetime.now(timezone.utc).isoformat()
        task.setdefault('history', []).append({'at': at, 'status': task['status'], 'note': args.note.strip()})
        data['updated_at'] = at
        validate(data)
        write_atomic(REGISTER, json.dumps(data, indent=2, ensure_ascii=False) + '\n')
        render(data)
    elif args.command == 'render':
        render(data)
    elif args.command == 'check':
        expected = {'README.md': render(data, save=False), data.get('work_plan', 'plan.md'): render_plan(data)}
        for name, content in expected.items():
            if (ROOT / name).read_text() != content:
                raise ValueError(name + ' is stale. Run manage.py render.')
        print(f'Valid register: {len(by_id)} tasks; dependencies, intent coverage and both Markdown views agree.')
    else:
        for t in data['tasks']:
            print(f"{marker(t, by_id)} {t['id']} {t['priority']} {t['title']} — {reason(t, by_id)}")


if __name__ == '__main__':
    main()
