# Bumeter brand (finalized 2026-09-28)

- **Name:** **Bumeter** = *Bum* (the author's nickname) + *meter* (a measuring gauge). Lowercase in the wordmark: `bumeter`. Available on crates.io and npm at the time of choosing.
- **Logo:** mascot **Bum**, a round flame with a smiling face (tokens are the "fuel" burning). See `bum/index.html`; the SVG file for each expression is in `bum/`.
- **Colors:** primary orange `#FF7A1A`; red-orange `#FF4B2B` for the blazing state; face `#3A1C0C`; cheeks `#FFB27A`. Flat, no gradients.
- **Font:** Be Vietnam Pro (800–900 for the wordmark).
- **App icon background:** white (primary), orange (white Bum, orange face), dark `#1F2227`.

## Expressions by state (tray, notifications, empty states)

| File | Expression | When |
|---|---|---|
| `bum-happy.svg` | Happy (smiling eyes, smiling mouth) | Today's usage is below average |
| `bum-calm.svg` | Normal (default logo) | Everything is fine |
| `bum-worry.svg` | Worried (raised eyebrows, sweat drop) | 5-hour quota ≥ first warning threshold (default 70%) |
| `bum-blaze.svg` | Blazing (red flames flaring, tense face) | An anomalous turn just occurred, or quota ≥ second threshold (default 90%) |
| `bum-sleep.svg` | Sleeping (closed eyes, "z z") | No session is running / quota data is stale |

## Notes

- The current version is drawn with SVG code to settle the concept and layout. For smoother lines in the release version, you can give `bum/index.html` to a designer or redo it in a vector tool, keeping the proportions and colors.
- At the 16px tray size, the pink cheek detail can be dropped.
