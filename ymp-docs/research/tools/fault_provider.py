#!/usr/bin/env python3
"""Deterministic local Codex-protocol fixture. No network or model access."""
import json
import os
import re
import sqlite3
import sys
import time
from pathlib import Path

control = Path(sys.argv[1])
settings = json.loads((control / 'settings.json').read_text())
state = sqlite3.connect(control / 'fixture.sqlite', timeout=30)
state.execute('CREATE TABLE IF NOT EXISTS calls(seq INTEGER PRIMARY KEY,phase TEXT,agent TEXT,fault TEXT,retried INTEGER)')
state.commit()
agent, thread, cwd = 'unknown', '', Path.cwd()


def send(value):
    print(json.dumps(value), flush=True)


def effect(prompt, bad=False):
    number = int(re.search(r'Write artifact-(\d+)\.txt', prompt).group(1))
    (cwd / f'artifact-{number}.txt').write_text('wrong\n' if bad else 'verified\n')
    with (cwd / 'effects.log').open('a') as out:
        out.write(str(number) + '\n')


def output(phase, prompt):
    if phase == 'plan':
        tasks = []
        for i in range(1, settings.get('tasks', 1) + 1):
            tasks.append(dict(title=f'Artifact {i}', description=f'Write artifact-{i}.txt containing exactly verified.', competence='implementation', difficulty=settings.get('difficulty', 'simple'), dependencies=[], checks=[f"test \"$(cat artifact-{i}.txt)\" = verified"]))
        return json.dumps(dict(summary='Deterministic fixture plan', tasks=tasks))
    if phase == 'bid':
        return json.dumps(dict(willing=True, approach='Produce and verify the fixture artifact.'))
    if phase in ('review_plan', 'review', 'final_review', 'review_memory'):
        return json.dumps(dict(approved=True, reason='Fixture approval; shell and external checks decide correctness.'))
    if phase == 'execute':
        effect(prompt, settings.get('fault') == 'content_bad')
        return 'The fixture artifact was written.'
    if phase == 'learn':
        return json.dumps(dict(useful=True, title='Fixture procedure', content='Inspect the artifact using an independent exact-content assertion.'))
    if phase == 'conversation':
        return json.dumps(dict(action='answer', answer='The file is in the current working directory.'))
    return 'Fixture task completed.'


for line in sys.stdin:
    message = json.loads(line)
    method, params = message.get('method'), message.get('params', {})
    if method == 'initialize':
        send(dict(id=message['id'], result={}))
    elif method in ('thread/start', 'thread/resume'):
        match = re.search(r'fixture_agent=(\w+)', params.get('developerInstructions', ''))
        agent = match.group(1) if match else 'unknown'
        cwd = Path(params['cwd'])
        thread = agent + ('-read' if params['sandbox'] == 'read-only' else '-write')
        send(dict(id=message['id'], result=dict(thread=dict(id=thread))))
    elif method == 'turn/start':
        prompt = params['input'][0]['text']
        phase = re.search(r'current assignment \(([^)]+)\)', prompt).group(1)
        prompt = prompt.rsplit('\n\nYour current assignment (', 1)[-1]
        fault = settings.get('fault', 'none')
        target = settings.get('fault_phase') == phase
        if fault in ('plan_one_exit', 'bid_one_exit'):
            target = phase == fault.split('_')[0] and agent == 'a0'
        state.execute('BEGIN IMMEDIATE')
        previous = state.execute('SELECT COUNT(*) FROM calls WHERE fault != ?', ('none',)).fetchone()[0]
        inject = target and (previous == 0 or settings.get('persistent_fault', False))
        policy = settings.get('retry_policy', 'none')
        eligible = inject and (policy == 'blind' or (policy == 'bounded_read' and phase != 'execute' and fault == 'transient_refusal'))
        retries = (2 if settings.get('persistent_fault', False) else 1) if eligible else 0
        recovered = retries > 0 and not settings.get('persistent_fault', False)
        cursor = state.execute('INSERT INTO calls(phase,agent,fault,retried) VALUES (?,?,?,?)', (phase, agent, fault if inject else 'none', retries))
        sequence = cursor.lastrowid
        state.commit()
        turn = f't{sequence}'
        send(dict(id=message['id'], result=dict(turn=dict(id=turn))))
        if inject and fault == 'timeout':
            time.sleep(10)
        if inject and fault == 'execute_exit_after':
            effect(prompt)
        for attempt in range(retries):
            time.sleep(0.01 * 2**attempt)
        if inject and fault == 'native_retry_notice':
            send(dict(method='error', params=dict(threadId=thread,turnId=turn,willRetry=True,error=dict(codexErrorInfo='serverOverloaded',message='Synthetic native retry in progress'))))
        elif inject and not recovered and fault == 'transient_refusal':
            send(dict(method='error', params=dict(threadId=thread, error=dict(code='serverOverloaded', message='Synthetic transient refusal'))))
            break
        elif inject and not recovered and fault not in ('malformed_review','native_retry_notice'):
            sys.exit(17)
        text = 'not a structured review' if inject and fault == 'malformed_review' else output(phase, prompt)
        counters = dict(inputTokens=100, outputTokens=20, cachedInputTokens=0, cacheWriteInputTokens=0, reasoningOutputTokens=0)
        send(dict(method='thread/tokenUsage/updated', params=dict(threadId=thread, turnId=turn, tokenUsage=dict(total=counters, last=counters))))
        send(dict(method='item/completed', params=dict(threadId=thread, turnId=turn, item=dict(type='agentMessage', text=text))))
        send(dict(method='turn/completed', params=dict(threadId=thread, turn=dict(id=turn, status='completed'))))
        break
