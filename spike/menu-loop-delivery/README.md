# menu-loop-delivery-spike

Throwaway spike. It checks which deliveries still reach Dome's UI thread on Windows while Dome's own tray menu is open. The tray menu runs `TrackPopupMenu`, a modal loop on the UI thread, until the menu closes. Appendix A1 of `plans/self-drawn-borders/design.md` in the Dome repository lists this check as unverified.

## What it does

The probe opens a popup menu from inside the window procedure of a hidden window, the way Dome's tray opens its menu, and closes the menu itself after 2 s. Throughout the run, a second thread posts a thread message, posts a message to a message-only window, and requests a `WM_PAINT` for each of two small windows, every 50 ms. One window asks with `RDW_INTERNALPAINT`, which requests a paint without marking anything as needing one. The other asks with `RDW_INVALIDATE`, which marks the whole window as needing a paint. A thread timer with a `TIMERPROC` and a timer on the message-only window tick at the same rate. The probe counts each delivery before the menu opens, while it is open, and after it closes.

| Delivery | Stands for | The design expects |
|---|---|---|
| Thread timer with a `TIMERPROC` | The design's deadline | Arrives inside the menu loop |
| Message to a message-only window | The design's wake | Arrives inside the menu loop |
| `WM_PAINT` from `RDW_INTERNALPAINT` | The frame callback | Arrives inside the menu loop |
| `WM_PAINT` from `RDW_INVALIDATE` | A candidate fix for the frame callback | Nothing, it is a comparison |
| Thread message | Today's wake | Lost |
| Timer on the message-only window | A fallback for the deadline | Nothing, it is a comparison |

## Running it

Run it in a Windows desktop session, from this folder.

```
cargo run --release
```

The run takes about 4 s. A popup menu opens at the mouse pointer, and two 16 px squares show at the top-left corner of the primary monitor. Leave the mouse and keyboard alone until the result prints, because a click outside the menu closes it early.

## Reading the result

The probe prints a count table and one verdict line per delivery. Its exit code means the following.

| Code | Meaning |
|---|---|
| 0 | Every delivery behaved as the design expects |
| 1 | At least one delivery did not |
| 2 | Inconclusive, because the menu did not stay open or a delivery never arrived even before it opened |
| 3 | The menu did not close, so the probe gave up |

A hosted CI runner may have no interactive desktop, so an exit code of 2 there says nothing about the design.
