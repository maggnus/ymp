# Exporter correction: independent parent review

**ACCEPT, 9/10** for code `534ce54a80a7498795f58ff162d9eb40ae8dcb13`,
with evidence commit `bf7c3b4`. The parent did not author this correction.
This is bounded exporter acceptance, not acceptance of all seventeen driver cases.

The build snapshot and runtime source verification, backend attribution, shared
durable native observation sequence, provenance audit, lifecycle anomaly handling,
and effort/location/partial projections were inspected. They consume actual
records and preserve unexpected relevant occurrences rather than selecting the
fixture's desired order. Unknown identities remain null; each projected event
has its durable source, including the strict-guarantee query.

All four dedicated controls were independently run and passed. They execute the
actual case adapters before mutating retained record copies, and invoke the
independent validator for extra events or changed order. Missing or altered
provenance rejects explicitly. Source/checker/reference drift is detected, and
the running executable is hashed. Output is
`/tmp/ymp126-exporter-parent-review.log`.

The author additionally ran the full workspace checks and rebuilt actual CLI
cases, including a source-drift control that rejects before a case starts. Those
results remain author evidence in `ymp-126-exporter-corrections.md`; the parent
did not duplicate the full suite or claim a new native-provider experiment.

The complete combined driver still needs all-case verification and independent
review, including the remaining adapters and the separately introduced
unknown-usage policy. Scripted results establish runtime/projection behavior,
not model quality or efficiency.
