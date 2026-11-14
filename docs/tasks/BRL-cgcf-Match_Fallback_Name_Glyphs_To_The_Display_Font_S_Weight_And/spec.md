# Context
After BRL-5e2s, characters outside Lilita One use the OS fallback for their script. The 2026-11-14 macOS captures show those glyphs as regular-weight system faces next to the chunky display face. Greek falls back to Athelas, a serif. CJK and kana use Heiti SC and Hiragino Sans W3. They are readable but thinner than Lilita One on grass and floor tiles. Evidence is in the BRL-5e2s spec.

# Options
- Set explicit script fallbacks to heavier sans families through `FontCx` (`collection.set_fallbacks`) per platform. For example Helvetica Neue Bold or SF for Grek/Cyrl/Latn, and Hiragino Sans W6 on macOS. Android would use Noto Sans Bold.
- Bundle a small OFL heavy subset (for example Noto Sans Black, Latin Extended + Greek + Cyrillic, about 0.3–0.6 MB). Measure it against BRL-5k0g first.
- Do not set `FontWeight::BOLD` on the name `TextFont`: Lilita One has one weight, so parley/fontique would synthesize a faux-bold Lilita.

# Acceptance criteria
- Overhead and Dashboard captures of accented Latin, Greek, Cyrillic and CJK names show fallback glyphs at a visibly similar weight, with no serif fallback for Greek.
- The size impact is recorded.
