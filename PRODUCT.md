# Product

## Register

product

## Users

Rust developers building terminal applications with reusable `ratatui`, `crossterm`, and tuicore components. They need predictable, composable controls that remain clear across keyboard focus changes and supported terminal themes.

## Product Purpose

Tuicore provides library-first TUI components and shared runtime primitives so downstream applications can stay declarative without reimplementing focus, scrolling, animation, theming, and event plumbing.

## Brand Personality

Quiet, precise, dependable. Individual themes may carry stronger personality, but interaction states remain coherent and trustworthy.

## Anti-references

Avoid inconsistent component vocabularies, selection colors that resemble success or warning states, low-contrast focus changes, and decorative styling that obscures terminal workflows.

## Design Principles

- Make state changes unmistakable without making them loud.
- Keep interaction semantics consistent across components.
- Preserve each theme's character while protecting readability.
- Hide runtime machinery behind small, composable APIs.
- Prefer familiar terminal affordances over ornamental novelty.

## Accessibility & Inclusion

Target WCAG AA-like text contrast where terminal color capabilities allow. Distinguish focused and blurred selection with more than color alone, and avoid relying on status-color associations for interaction state.
