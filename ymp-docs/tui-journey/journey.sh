#!/bin/sh
# Drive the ymp executable through the W1 terminal journey inside tmux and write
# what the terminal showed to a transcript.
#
#   journey.sh YMP ROOT TRANSCRIPT
#
# YMP is the built executable, ROOT a directory that does not exist yet and
# TRANSCRIPT the file to write. Every part uses the scripted team, so no model
# is called. Workspaces and stores are created under ROOT and nowhere else.
set -eu

[ $# -eq 3 ] || { echo "usage: journey.sh YMP ROOT TRANSCRIPT" >&2; exit 2; }
YMP=$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")
[ -x "$YMP" ] || { echo "journey: $1 is not an executable" >&2; exit 2; }
[ ! -e "$2" ] || { echo "journey: $2 exists; name a new directory" >&2; exit 2; }
mkdir -p "$2"
ROOT=$(cd "$2" && pwd -P)
OUT=$3
PANE=
: > "$OUT"

GOAL='Sort the numbers of input.json into sorted.json'
EXPECT='/expect sorted.json [1,2,2,3]\n'

# Paths under ROOT and the executable are written as <root> and ymp.
plain() { sed -e "s|$ROOT|<root>|g" -e "s|$YMP|ymp|g"; }
note() { printf '\n==== %s ====\n' "$1" | plain >> "$OUT"; }
# Lines the terminal wrapped are joined, so a long path stays one path. Spaces
# that only fill a line to the width of the terminal are dropped.
shot() {
    note "$1"
    tmux capture-pane -t "$PANE" -p -J | plain | awk '
        { sub(/[ \t]+$/, ""); line[NR] = $0 } /[^ ]/ { last = NR }
        END { for (i = 1; i <= last; i++) print line[i] }' >> "$OUT"
}
say() {
    for line in "$@"; do
        tmux send-keys -t "$PANE" -l -- "$line"
        tmux send-keys -t "$PANE" Enter
        sleep 0.3
    done
}
key() { tmux send-keys -t "$PANE" "$@"; sleep 0.3; }
# await TEXT [SECONDS]: wait until the terminal shows TEXT.
await() {
    tries=$(( ${2:-240} * 2 ))
    while ! tmux capture-pane -t "$PANE" -p | grep -qF -- "$1"; do
        tries=$((tries - 1))
        [ "$tries" -gt 0 ] || { shot "journey stopped: '$1' did not appear"; exit 1; }
        sleep 0.5
    done
}
# prepare NAME PROGRAM: a workspace with one input file and a scripted program.
prepare() {
    mkdir -p "$ROOT/$1/work"
    printf '[3,1,2,2]\n' > "$ROOT/$1/work/input.json"
    printf '%s\n' "$2" > "$ROOT/$1/program.json"
}
# enter NAME COLUMNS ROWS: the interface in a terminal of that size.
enter() {
    PANE="ymp-journey-$1"
    tmux kill-session -t "$PANE" 2>/dev/null || true
    tmux new-session -d -s "$PANE" -x "$2" -y "$3" \
        "'$YMP' --store '$ROOT/$1/store' --workspace '$ROOT/$1/work' --scripted '$ROOT/$1/program.json'; echo EXIT=\$?; stty -a | head -2; sleep 600"
    await 'State the goal'
}
leave() {
    say '/quit'
    await 'EXIT='
    shot "$1"
    tmux kill-session -t "$PANE"
}
recorded() { "$YMP" sessions --store "$ROOT/$1/store" | awk 'NR == 1 { print $1 }'; }
command_line() {
    note "\$ ymp $*"
    code=0
    text=$("$YMP" "$@" 2>&1) || code=$?
    printf '%s\n(exit %s)\n' "$text" "$code" | plain >> "$OUT"
}
program() {
    printf '{"writes":[{"path":"sorted.json","text":"%s"}],"questions":%s,"producer_coverage":"Complete","pace_ms":%s}' \
        "$1" "$2" "$3"
}
QUESTION='[{"question":"Should duplicates be kept in the sorted output?","p_misinterpretation":0.5,"rework_cost":10.0,"assumption":"Duplicates are kept","reason":"The goal does not say whether duplicates stay"}]'

# 1. The goal is stated, the session runs to a delivered report, the recorded
#    information is inspected and the durable report is read again.
prepare success "$(program '[1,2,2,3]\n' '[]' 200)"
enter success 110 34
say "$GOAL"
shot '1.1 text alone starts nothing'
say "$EXPECT" '/preserve input.json' '/start'
await 'report delivered'
shot '1.2 the session ran to a delivered report'
say '/criteria'; shot '1.3 criteria table'
key Enter; shot '1.4 detail of the selected criterion'
key Escape; key Escape
say '/resources'; shot '1.5 resources'
key Escape
say '/activity'; shot '1.6 activity table'
key Escape
say '/start'
await 'Denied'
shot '1.7 a second start is denied and records nothing'
leave '1.8 after /quit the terminal is restored'
command_line sessions --store "$ROOT/success/store"
command_line report --store "$ROOT/success/store" "$(recorded success)"
enter success 110 34
say '/sessions'; shot '1.9 recorded sessions of the store'
key Enter
await 'Opened read-only'
shot '1.10 the recorded session and its report, opened read-only'
leave '1.11 leaving a session that was only read'

# 2. The produced bytes do not meet the expectation.
prepare unmet "$(program '[1,2,3]\n' '[]' 200)"
enter unmet 110 34
say "$GOAL" "$EXPECT" '/preserve input.json' '/start'
await 'report delivered'
shot '2.1 the report names the unmet criterion'
say '/criteria'; shot '2.2 criteria with an unmet criterion'
key Escape
leave '2.3 exit'

# 3. Work is interrupted; the runtime reports from recorded facts.
prepare interrupt "$(program '[1,2,2,3]\n' '[]' 4000)"
enter interrupt 110 34
say "$GOAL" "$EXPECT" '/preserve input.json' '/start'
await 'Producer · scripted · working'
key C-c
await 'report delivered'
shot '3.1 Ctrl+C during work: the stop is recorded and a report follows'
say '/interrupt'
await 'not_working'
shot '3.2 nothing runs, so the interrupt is denied and records nothing'
say '/recover continue'
await 'Recovery requested'
sleep 2
shot '3.3 the runtime decides whether a stopped session may continue'
leave '3.4 exit'

# 4. The interface is left while a call is in flight. The call has no recorded
#    end, so its usage is unknown; recovery delivers a report and resumes nothing.
prepare left "$(program '[1,2,2,3]\n' '[]' 4000)"
enter left 110 34
say "$GOAL" "$EXPECT" '/preserve input.json' '/start'
await 'Producer · scripted · working'
leave '4.1 /quit while a call is in flight'
command_line sessions --store "$ROOT/left/store"
command_line report --store "$ROOT/left/store" "$(recorded left)"
enter left 110 34
say "/open $(recorded left)"
await 'Opened read-only'
shot '4.2 reopened: a call without a recorded end and unknown usage'
say 'Please continue the work'
shot '4.3 text in an opened session continues nothing'
say '/recover report'
await 'report delivered'
shot '4.4 the deterministic report, with no further call'
say '/resources'; shot '4.5 resources with the retained hold'
key Escape
leave '4.6 exit'

# 5. A narrow terminal, a clarifying question and reading older activity while
#    new activity arrives.
prepare narrow "$(program '[1,2,2,3]\n' "$QUESTION" 1500)"
enter narrow 70 24
say "$GOAL" "$EXPECT" '/preserve input.json' '/start'
await 'waiting for your answer'
shot '5.1 narrow terminal: the question waits for the user'
say 'Keep the duplicates'
await 'scripted · working'
key PageUp; key PageUp
shot '5.2 reading older activity while work runs'
sleep 5
shot '5.3 the same place five seconds later'
key End
await 'report delivered'
shot '5.4 back at the end: report delivered'
key C-b; shot '5.5 sidebar overlay'
key Escape
key C-p
tmux send-keys -t "$PANE" -l -- 'rep'; sleep 0.3
shot '5.6 command palette filtered by "rep"'
key Escape
leave '5.7 exit'

# 6. A store inside the workspace is refused before anything is created.
prepare inside "$(program '[1,2,2,3]\n' '[]' 200)"
PANE=ymp-journey-inside
tmux kill-session -t "$PANE" 2>/dev/null || true
tmux new-session -d -s "$PANE" -x 110 -y 12 \
    "'$YMP' --store '$ROOT/inside/work/store' --workspace '$ROOT/inside/work' --scripted '$ROOT/inside/program.json'; echo EXIT=\$?; stty -a | head -2; sleep 600"
await 'EXIT='
shot '6.1 a store inside the workspace is refused'
tmux kill-session -t "$PANE"
note '$ ls <root>/inside/work'
ls "$ROOT/inside/work" >> "$OUT"

# 7. Refusals before anything is recorded.
prepare draft "$(program '[1,2,2,3]\n' '[]' 200)"
enter draft 110 34
say "$GOAL" '/expect ../outside.json text'
await 'workspace_path'
say "$EXPECT" '/new'
await 'Draft cleared'
shot '7.1 a path outside the workspace is refused; /new drops the draft'
say "$GOAL" "$EXPECT"
mv "$ROOT/draft/work" "$ROOT/draft/work-moved"
say '/start'
await 'workspace_io'
shot '7.2 a start that records nothing leaves the draft and no session'
leave '7.3 exit'
command_line sessions --store "$ROOT/draft/store"

echo "journey: transcript written to $OUT"
