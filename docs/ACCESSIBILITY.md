# Accessibility

The interface is keyboard-only. Arrow keys work alongside `j`/`k`; Home/End
and cursor pages reduce repetitive input; overlays have Escape paths and
`ctrl-c` is global.

Color is never the sole status signal. Availability, mutation mode, Workflow
state, and warnings retain text labels. Disable color with `--no-color`,
`[ui] color = false`, or `NO_COLOR`.

The layout has a plain fallback below 58×16 and is tested at 80×24. Long input
scrolls to retain the cursor; Unicode movement uses character boundaries and
display widths.

Terminal screen-reader behavior varies by emulator. Generated `--help` and the
manpage are linear text alternatives; private JSON export is a structured
alternative to dense details. Operators requiring a screen reader should also
consider Temporal Web UI for continuous navigation.
