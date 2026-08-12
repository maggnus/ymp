# Repair independent TOML datetime bounds

Source revision: 99d50bb686adcdb569c0a1d37db1ac175c57e5ea.

## Goal

Repair two independent datetime validity bounds without changing parsing syntax, public API, or
other TOML crates.

## Requirements

1. TOML-LEAP-SECOND: seconds value 60 must be accepted as the leap-second boundary and values
   greater than 60 must be rejected.
2. TOML-CALENDAR-DAY: day validity must follow Gregorian month lengths and leap-year rules,
   including century and 400-year exceptions.

## Visible check

Run cargo test --locked --offline -p toml_datetime --lib. The protected oracle supplies held-out
boundary dates and times from the same public rules.

## Scope

Changes may affect only toml_datetime validation and its tests. Dependencies, manifests, public
API, other workspace crates, and formatting or serialization semantics are out of scope.
