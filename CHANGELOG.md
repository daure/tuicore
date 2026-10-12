# Changelog

## Unreleased

- Spinners schedule their next 80 ms glyph frame through `TickResult.next_tick` with `active = false`.
  Custom parents must merge the complete tick result to preserve animation scheduling.
