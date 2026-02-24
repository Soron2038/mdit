# AGENTS.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project

`mdit` is a small, in-space-rendering Markdown editor (MIT licensed) for macOS. Built in Rust using native AppKit bindings.

**Core concept:** Typora-style inline rendering — Markdown syntax disappears when the cursor leaves a span. No split view, no preview pane. The document is the UI.

## Key Documents

- **PRD:** `docs/plans/2026-02-24-mdit-prd.md` — Full product requirements: features, non-goals, architecture decisions, success criteria
- **Implementation Plan:** `docs/plans/2026-02-24-mdit-implementation.md` — 19-task step-by-step plan with code, TDD approach, and exact commands
