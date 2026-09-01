# Fuse inventory and route branches into one release plan

The project receives inventory lines (`service=revision`) and route lines
(`route->service`). Implement the public functions without changing their signatures.

## Requirements

1. Inventory normalization trims fields, rejects malformed or empty fields, accepts an exact
   duplicate, and rejects conflicting revisions for the same service.
2. Route normalization applies the same field rules, accepts an exact duplicate, and rejects a
   route that names two different services.
3. Release-plan synthesis rejects a route whose service is absent and rejects an inventory service
   with no route. A valid plan is sorted and contains `route@service#revision` entries.

Only files under `src/` may change. `Cargo.toml`, `Cargo.lock`, this contract, visible checks, and
all build entry points are fixed. Protected checks instantiate only these public requirements.
