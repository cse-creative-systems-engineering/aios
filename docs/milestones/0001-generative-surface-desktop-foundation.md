# M8 Generative Surface Desktop Foundation

**Status:** Working checkpoint
**Date:** 2026-08-17
**Checkpoint commit:** `8afcded`
**Current branch:** `main`

## What This Checkpoint Proves

- Aios gathers live specialist evidence through the existing backend path.
- A separate groundless generation call produces a surface from the verified evidence.
- The generated surface is displayed without the old fixed panel clipping it.
- The canvas is transparent outside the generated widget.
- The widget can be moved around the usable desktop area.
- Clicks outside the widget pass through to the desktop.
- The resident sidebar remains docked at the left edge below the desktop top bar.

## Manual Regression

Launch the current debug binary with:

```bash
AIOS_UNCONSTRAINED_SURFACE=1 ./src-tauri/target/debug/aios-tauri
```

Then verify:

1. Ask for a CPU usage widget.
2. Confirm the complete widget is visible.
3. Drag the widget to more than one position.
4. Click the desktop outside the widget.
5. Confirm the sidebar remains at the left edge and below the top bar.
6. Confirm the sidebar still accepts another prompt while the widget is open.

## Deliberate Follow-Up Work

This checkpoint establishes the desktop surface foundation. It does not claim
that the surface lifecycle is complete. Follow-up work includes multiple live
surfaces, editing an existing surface from a later prompt, composing one
surface from evidence gathered by multiple specialists, persistence and close
state, and a redesigned premium sidebar.

Development changes must preserve the fail-fast rule. Errors should be
reported at their boundary, and undocumented fallbacks must not hide broken
surface delivery or stale evidence.
