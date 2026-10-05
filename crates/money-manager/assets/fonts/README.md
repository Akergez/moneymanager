# Bundled fonts

The application ships its own UI font instead of relying on what the system
has installed. The text renderer in this GPUI snapshot loads a variable font
(Adwaita Sans, Noto Sans on current desktops) as one regular face, so nothing
drawn bold or semibold actually is; and on Android none of the families its
fallback list names exist at all.

`Inter-*.ttf` are the static faces from the Inter 4.1 release
(<https://github.com/rsms/inter>), unmodified. SIL Open Font License 1.1 — see
`Inter-LICENSE.txt`. `src/fonts.rs` registers them.
