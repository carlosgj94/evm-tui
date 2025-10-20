# Input & Navigation Spec

## Key Map
- `h` `j` `k` `l`: move within the focused pane, mirroring Vim semantics.
- `[` `]`: cycle backward/forward through tabs within the pane.
- `1`..`9`: focus numbered panes (Top=1, Sidebar=2, Main View=3, Bottom Bar reserved for future).
- `Tab` / `Shift-Tab`: optional alternative focus cycling for accessibility.
- `q`: exit application (confirm if background jobs are running).
- Key remapping is deferred; bindings are fixed in MVP to match documentation.

## Focus Model
- Global app state tracks active pane, active tab per pane, and selection indices.
- Pane headers render their focus number and a highlight when active.
- Modal dialogs temporarily capture input but remember the previous focus stack for return.

## Accessibility & Feedback
- Provide auditory-equivalent cues via status messages for screen reader compatibility (future work).
- When a key has no effect, flash a subtle border color to indicate blocked action.
- Record keybindings in bottom bar hints and update in real time.
- Keyboard-first navigation is the target experience; mouse support may be explored later but is not required for MVP.
