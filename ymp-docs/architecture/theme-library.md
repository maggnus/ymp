# Theme palette integration

Decision: use `ratatui-themes` as a palette source behind ymp's own semantic `Theme` interface.
The owner also requires the latest stable Ratatui release. This decision is implemented through
YMP-136 (terminal dependency migration) and YMP-137 (theme catalog and installed verification).

## Verified versions

On 2026-09-13 the official latest releases are
[Ratatui 0.30.2](https://github.com/ratatui/ratatui/releases/tag/ratatui-v0.30.2) and
[ratatui-themes 0.3.0](https://github.com/ricardodantas/ratatui-themes/releases/tag/v0.3.0).
The latter includes Midnight Commander. Its local reference checkout is
`/Users/maggnus/Code/ratatui-themes` at `33961a7758005d784b57d6a8488fe965b33182d2`;
that path is a design reference, not an application dependency.

The previous ymp dependency is Ratatui 0.29. Updating it is a deliberate terminal-stack
migration, with compatible Crossterm dependencies and regression checks for input, restoration
and table behavior. The theme library does not justify retaining an obsolete Ratatui version.

## Why the library fits

It supplies named palettes, canonical IDs, light/dark classification and semantic colors such as
accent, foreground, background, selection, error, warning, success and information. Reusing these
values avoids maintaining a duplicate table of sixteen palettes. Its MIT license and small API
make it suitable for this limited role.

It does not define the whole ymp appearance. ymp also needs surface, raised background, rules,
faint text, focus borders and readable text on an accent background. One adapter maps library
palettes to those roles; views continue to use the existing `Theme` interface. This also keeps a
future replacement of the palette source local to that adapter.

Keep the existing chooser, persistence and keyboard behavior. The library's optional picker
widget is unnecessary, and unused optional features should remain disabled. This is a compiled
palette dependency, not a plugin runtime, network service or configuration framework.

## Catalog and compatibility

Keep Ember and Slate unchanged, with Ember as the default. Replace selectable Paper, Contrast
and Terminal with the sixteen themes requested by the owner: Dracula, One Dark Pro, Nord,
Catppuccin Mocha, Catppuccin Latte, Gruvbox Dark, Gruvbox Light, Tokyo Night, Solarized Dark,
Solarized Light, Monokai Pro, Rosé Pine, Kanagawa, Everforest, Cyberpunk and Midnight Commander.

The final catalog has eighteen themes. IDs remain stable strings in existing preferences.
Removed or unknown saved IDs must load safely; the existing fallback to Ember is acceptable.
Palette colors come from the library's actual data, which is more authoritative than preview
chips in its README. Additional role mappings must preserve readable text and selection.

The main cost is the Ratatui migration and the regression checks it requires. The palette
adapter itself should stay small. Verification covers preserved Ember/Slate values, catalog
membership, dark/light examples, reaching the last theme in short windows, preview cancellation,
saving/reloading a choice, and the existing table/input/exit scenarios. No provider inference
is needed for these checks.
