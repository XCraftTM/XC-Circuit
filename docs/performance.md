---
title: Performance
---

# Performance

::: info In short
XC™ Circuit draws a typical bot in **about 1 ms per frame** and runs at **800–1,100 frames a second** on a gaming PC when nothing holds it back. Even a workspace of **1,600 blocks** stays smooth on a 120 Hz screen on an everyday computer. Frames only drop with **thousands of blocks in one workspace** or **hundreds of blocks on screen at once on a 4K window**. Bots run **75,000–200,000 blocks a second** once exported.
:::

The bundled starter bots have 3 to 8 blocks per workspace, and even large bots rarely have more than 100 in one. Everything below is about how far beyond that you can go.

## How it's measured

A screen shows a new picture (a frame) 60 or 120 times a second, so the app has **16.7 ms** (60 Hz) or **8.3 ms** (120 Hz) per frame. If a frame takes longer, the screen shows the previous picture again and movement stutters: frames drop.

Two kinds of measurements:

- **Real frame rates:** the whole app in its window, drawing with the graphics card, with a made-up project that it pans and zooms by itself. Vsync is off, so it draws as fast as it can instead of waiting for the screen.
- **Frame times:** what XC Circuit does on the processor for each frame (building the interface and turning it into triangles), timed without a window, so many more cases can be compared. The graphics card then only draws the triangles, which took it about 0.2 ms a frame in the real measurements.

The tables show the **95th percentile**: 19 out of 20 frames are at least this fast. They ran on two computers:

| | CPU | Graphics | Notes |
| --- | --- | --- | --- |
| **Desktop** | AMD Ryzen 7 9800X3D (8 cores) | NVIDIA GeForce RTX 4070 SUPER | A fast gaming PC, Windows 11 |
| **Everyday** | AMD EPYC 7763, 4 cores of a shared server | none | GitHub's standard Windows machine: about 2× slower, like an ordinary laptop |

🟢 smooth at 120 Hz · 🟡 smooth at 60 Hz · 🔴 drops frames

## Real frame rates

The desktop, a 1440×900 window, Vulkan. Each workspace was shown still, panned across and zoomed from 100% to 25% and back.

| Blocks in the workspace | Still | Panning | Zooming | Slowest frames (95%) |
| --- | --- | --- | --- | --- |
| 25 | 1,045 FPS | 1,139 FPS | 374 FPS | 🟢 3.0 ms |
| 200 | 836 FPS | 801 FPS | 304 FPS | 🟢 4.9 ms |
| 800 | 493 FPS | 483 FPS | 234 FPS | 🟢 5.9 ms |
| 1,600 | 312 FPS | 308 FPS | 181 FPS | 🟢 7.4 ms |

**Without a graphics card** (drawing in software with Mesa's llvmpipe on Linux, 4 cores of an AMD EPYC 9V45 server), the same test still runs at about 42–67 FPS still or panning, and 34–51 FPS while zooming: usable, but noticeably less smooth.

Out of more than 10,000 frames on the desktop, only two took longer than 16.7 ms (single hiccups while zooming). Zooming costs the most, because every text is laid out again at its new size. Your screen still shows at most its own refresh rate (60, 144, 240 …); the rest is headroom.

## Blocks in one workspace

Frame times with a few blocks on screen (100% zoom) and the rest of the workspace off screen. Off-screen blocks cost a little: their layout is kept, and checked each frame for changes.

| Blocks in the workspace | Desktop | Everyday |
| --- | --- | --- |
| 25 | 🟢 0.9 ms | 🟢 1.7 ms |
| 200 | 🟢 1.0 ms | 🟢 2.0 ms |
| 400 | 🟢 1.5 ms | 🟢 2.7 ms |
| 800 | 🟢 1.7 ms | 🟢 4.1 ms |
| 1,600 | 🟢 3.3 ms | 🟢 8.2 ms |
| 3,200 | 🟢 6.4 ms | 🔴 18.5 ms |

An everyday computer stays smooth at 120 Hz up to about **1,600 blocks** in one workspace and drops frames at 3,200; the desktop handles 3,200 at 120 Hz. For very big bots, split them into several workspaces (the Call Workspace block runs one from another): other workspaces cost nothing.

## Blocks on screen at once

The window filled edge to edge with blocks, from 100% zoom down to 25% (the smallest). More pixels mean more blocks fit, so a 4K window (2560×1440 at 150%) shows the most.

| Window | Zoom | Blocks on screen | Desktop | Everyday |
| --- | --- | --- | --- | --- |
| 1400×900 | 100% | 12 | 🟢 0.6 ms | 🟢 1.1 ms |
| 1400×900 | 50% | 48 | 🟢 2.3 ms | 🟢 4.2 ms |
| 1400×900 | 35% | 88 | 🟢 3.1 ms | 🟢 6.0 ms |
| 1400×900 | 25% | 165 | 🟢 6.4 ms | 🟡 12.7 ms |
| 4K | 100% | 35 | 🟢 2.0 ms | 🟢 4.1 ms |
| 4K | 50% | 126 | 🟢 7.3 ms | 🟡 14.6 ms |
| 4K | 35% | 260 | 🟡 9.6 ms | 🔴 22.8 ms |
| 4K | 25% | 459 | 🔴 19.5 ms | 🔴 44.2 ms |

On a normal-sized window frames never drop, even fully zoomed out. On a **4K window**, about **130–250 blocks on screen** is where an everyday computer starts to drop frames (450 for the desktop). Drawing that many blocks is the one thing that costs real time: each one is a small form with its own fields.

## Working on the canvas

A workspace of 400 blocks at 50% zoom: what moving around and editing cost compared with doing nothing.

| What you do | 1400×900, desktop | 1400×900, everyday | 4K, desktop | 4K, everyday |
| --- | --- | --- | --- | --- |
| Nothing (idle) | 🟢 2.8 ms | 🟢 6.1 ms | 🟡 8.8 ms | 🔴 18.1 ms |
| Moving the pointer | 🟢 2.8 ms | 🟢 6.4 ms | 🟡 8.5 ms | 🔴 18.0 ms |
| Panning | 🟢 2.8 ms | 🟢 5.5 ms | 🟡 8.8 ms | 🔴 18.1 ms |
| Zooming | 🟢 5.0 ms | 🟡 9.6 ms | 🟡 10.2 ms | 🔴 22.2 ms |
| Dragging a block | 🟢 3.0 ms | 🟢 5.6 ms | 🟢 7.5 ms | 🔴 17.2 ms |

Panning and dragging cost no more than standing still. A 400-block workspace with 120 blocks on a 4K screen is the one case that isn't smooth on an everyday computer (just under 60 frames a second).

When nothing changes, XC Circuit doesn't redraw (at most once a second, while a plugin's script runs), so an idle window uses almost no processor time. While blocks glow after running, the canvas redraws about 30 times a second.

## Everyday tasks

| Task | Desktop | Everyday |
| --- | --- | --- |
| Load every plugin (205 blocks, compiled in QuickJS) | 81 ms | 125 ms |
| Open a project (any size; it loads its plugins too) | 84 ms | 128 ms |
| Open a workspace of 25 blocks for the first time | 1.8 ms | 3.8 ms |
| Open a workspace of 200 blocks for the first time | 12 ms | 27 ms |
| Open a workspace of 800 blocks for the first time | 78 ms | 168 ms |
| Open a workspace of 1,600 blocks for the first time | 266 ms | 530 ms |
| Open the block picker | 14 ms | 28 ms |
| Each letter typed into the picker's search | 3.3 ms | 7.2 ms |
| Paste 100 blocks | 0.2 ms | 0.3 ms |
| Undo that | under 0.1 ms | under 0.1 ms |
| Save a project of 1,600 blocks | 1.0 ms | 2.3 ms |
| Start a bot, until its first block ran | 54 ms | 108 ms |
| Export a bot of 800 blocks (without installing its packages) | 0.8 ms | 3.6 ms |

A workspace takes longer the first time it's opened because each block's shape is worked out (its `define()` runs) and its texts are laid out once; after that, switching to it is instant. Exporting with "install packages" also runs npm, which takes as long as the download.

## Running bots

The runtime that runs your bot, timed with 10,000 rounds of each case. **Run in the editor** reports every block to the editor as it starts and ends (that's how blocks glow and show their times); an **exported bot** skips that.

| Case | Run in the editor, desktop | Exported, desktop | Run in the editor, everyday | Exported, everyday |
| --- | --- | --- | --- | --- |
| Blocks (Set Variable with Math) | 94,000 a second | 202,000 a second | 19,000 a second | 74,000 a second |
| Stored data (Increase Stored Value) | 147,000 a second | 294,000 a second | 27,000 a second | 83,000 a second |
| Signals (Emit Signal → On Signal) | 66,000 a second | 156,000 a second | 15,000 a second | 52,000 a second |

A block takes 5–15 microseconds in an exported bot. Real bots spend almost all their time waiting for Discord or Fluxer (a reply takes 50–300 ms), so the blocks themselves are never what makes a bot slow. During these tests a bot used 50–150 MB of memory, most of it Node.js itself.

## If it gets slow

- **Split very big workspaces.** Thousands of blocks in one workspace cost a little each frame, other workspaces cost nothing. Use Call Workspace to run one from another.
- **Zoom in a bit on big screens.** Hundreds of blocks on a 4K screen are the most expensive thing to draw.
- **Check the graphics driver.** If no graphics card driver can be used, XC Circuit draws in software (it says so in the Console). On Linux that still gave 34–67 FPS above; Windows' software renderer was much slower on GitHub's screenless servers (about 2 FPS), though that says more about those servers than about a normal PC. Installing the graphics driver fixes it.

## Run them yourself

The benchmarks are in the repository. With [Rust](https://rustup.rs) and Node.js installed:

```bash
cargo run --release -p circuit-editor -- --fps fps.json
```

```bash
cargo test --release -p circuit-editor bench_ -- --ignored --nocapture --test-threads=1
```

```bash
node runtime/bench/runtime.js
```

The first opens XC Circuit for about a minute: it uses a made-up project and its own settings in a temporary folder, moves by itself (don't click into it) and closes when done, writing `fps.json`. The others print tables like the ones above and write `target/bench/*.json`. The **Benchmarks** workflow on GitHub runs all three on GitHub's Windows and Linux machines.

Measured with XC Circuit 0.1.0 in November 2026.
