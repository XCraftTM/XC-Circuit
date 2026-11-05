---
title: Performance
---

# Performance

::: info In short
XC™ Circuit draws a typical bot in **1–2 ms per frame**, far inside what a 120 Hz screen needs. It stays smooth at 60 Hz up to **800 blocks in one workspace** on an everyday computer (1,600 on a fast one); frames only drop beyond that, or with **hundreds of blocks on screen at once on a 4K window**. Bots run **70,000–200,000 blocks a second** once exported.
:::

The bundled starter bots have 3 to 8 blocks per workspace, and even large bots rarely have more than 100 in one. Everything below is about how far beyond that you can go.

## How it's measured

A screen shows a new picture (a frame) 60 or 120 times a second, so the app has **16.7 ms** (60 Hz) or **8.3 ms** (120 Hz) per frame. If a frame takes longer, the screen shows the previous picture again and movement stutters: frames drop.

The benchmarks time what XC Circuit does on the processor for each frame: building the interface and turning it into triangles. The graphics card then only draws those triangles, which takes it far less time. Each case runs 60 to 90 frames; the tables show the **95th percentile**: 19 out of 20 frames are at least this fast.

They ran on two computers:

| | CPU | Notes |
| --- | --- | --- |
| **Desktop** | AMD Ryzen 7 9800X3D (8 cores) | A fast gaming desktop, Windows 11 |
| **Everyday** | AMD EPYC 7763, 4 cores of a shared server | GitHub's standard Windows machine: about 2–3× slower, like an ordinary laptop |

Green means smooth at 120 Hz, yellow smooth at 60 Hz, red drops frames.

## Blocks in one workspace

A window of 1400×900 at 100% zoom, so only about a dozen blocks are on screen; the rest of the workspace is off screen but still costs a little (its blocks are laid out to know where wires go).

| Blocks in the workspace | Desktop | Everyday |
| --- | --- | --- |
| 25 | 🟢 0.9 ms | 🟢 1.8 ms |
| 100 | 🟢 1.4 ms | 🟢 2.6 ms |
| 200 | 🟢 1.9 ms | 🟢 4.3 ms |
| 400 | 🟢 3.1 ms | 🟢 7.3 ms |
| 800 | 🟢 5.7 ms | 🟡 12.8 ms |
| 1,600 | 🟡 10.9 ms | 🔴 32.8 ms |
| 3,200 | 🔴 24.1 ms | 🔴 68.3 ms |

An everyday computer stays smooth at 60 Hz up to **800 blocks** in one workspace and drops frames at 1,600; a fast one stays smooth up to 1,600 and drops at 3,200. Split big bots into several workspaces (the Call Workspace block runs one from another): blocks in other workspaces cost nothing.

## Blocks on screen at once

The window filled edge to edge with blocks, from 100% zoom down to 25% (the smallest). More pixels mean more blocks fit, so a 4K window (2560×1440 at 150%) shows the most.

| Window | Zoom | Blocks on screen | Desktop | Everyday |
| --- | --- | --- | --- | --- |
| 1400×900 | 100% | 12 | 🟢 0.6 ms | 🟢 1.2 ms |
| 1400×900 | 50% | 48 | 🟢 2.7 ms | 🟢 4.8 ms |
| 1400×900 | 35% | 88 | 🟢 3.4 ms | 🟢 7.3 ms |
| 1400×900 | 25% | 165 | 🟢 6.5 ms | 🟡 14.2 ms |
| 4K | 100% | 35 | 🟢 1.9 ms | 🟢 4.8 ms |
| 4K | 50% | 126 | 🟢 7.6 ms | 🟡 16.4 ms |
| 4K | 35% | 260 | 🟡 11.1 ms | 🔴 23.1 ms |
| 4K | 25% | 459 | 🔴 22.0 ms | 🔴 48.8 ms |

On a normal-sized window frames never drop, even fully zoomed out. On a **4K window**, about **250 blocks on screen** is the limit on an everyday computer (450 on a fast one); at that point the GPU gets about 400,000 triangles a frame, which is still easy for it.

## Working on the canvas

A workspace of 400 blocks at 50% zoom: what moving around and editing cost compared with doing nothing.

| What you do | 1400×900, desktop | 1400×900, everyday | 4K, desktop | 4K, everyday |
| --- | --- | --- | --- | --- |
| Nothing (idle) | 🟢 4.4 ms | 🟡 9.5 ms | 🟡 11.1 ms | 🔴 23.4 ms |
| Moving the pointer | 🟢 4.6 ms | 🟡 9.5 ms | 🟡 10.4 ms | 🔴 24.3 ms |
| Panning | 🟢 4.4 ms | 🟡 10.3 ms | 🟡 9.7 ms | 🔴 23.5 ms |
| Zooming | 🟢 6.0 ms | 🟡 12.8 ms | 🟡 12.2 ms | 🔴 27.8 ms |
| Dragging a block | 🟢 4.4 ms | 🟡 9.1 ms | 🟡 9.7 ms | 🔴 23.1 ms |

Panning and dragging cost no more than standing still; zooming a little more, because text is laid out again at the new size. A 400-block workspace on a 4K window is the one combination that isn't smooth on an everyday computer (about 40 frames a second).

When nothing changes, XC Circuit doesn't redraw (at most once a second, when a native plugin is turned on), so an idle window uses almost no processor time. While blocks glow after running, the canvas redraws about 30 times a second.

## Starting

| Step | Desktop | Everyday |
| --- | --- | --- |
| Load every plugin (205 blocks, compiled in QuickJS) | 85 ms | 130 ms |
| Open DemoBot | 83 ms | 128 ms |
| First frame of a 400-block workspace | 54 ms | 111 ms |
| Every frame after that | 4.4 ms | 9.2 ms |
| Prepare a bot to run (400 blocks) | 1.1 ms | 1.8 ms |
| Save the project | 2.0 ms | 5.9 ms |

The first frame is slower because every text is laid out and every icon loaded once; after that they're reused.

## Running bots

The runtime that runs your bot, timed with 10,000 rounds of each case. **Run in the editor** reports every block to the editor as it starts and ends (that's how blocks glow and show their times); an **exported bot** skips that.

| Case | Run in the editor, desktop | Exported, desktop | Run in the editor, everyday | Exported, everyday |
| --- | --- | --- | --- | --- |
| Blocks (Set Variable with Math) | 94,000 a second | 202,000 a second | 23,000 a second | 69,000 a second |
| Stored data (Increase Stored Value) | 147,000 a second | 294,000 a second | 23,000 a second | 81,000 a second |
| Signals (Emit Signal → On Signal) | 66,000 a second | 156,000 a second | 15,000 a second | 51,000 a second |

A block takes 5–15 microseconds in an exported bot. Real bots spend almost all their time waiting for Discord or Fluxer (a reply takes 50–300 ms), so the blocks themselves are never what makes a bot slow. During these tests a bot used 50–150 MB of memory, most of it Node.js itself.

## If it gets slow

- **Split big workspaces.** Off-screen blocks still cost a little each frame, other workspaces cost nothing. Use Call Workspace to run one from another.
- **Zoom in a bit on big screens.** Hundreds of blocks on a 4K screen are the most expensive thing to draw.
- **Check the graphics driver.** If no graphics card driver can be used, XC Circuit draws in software (it says so in the Console), which is much slower than anything above. Installing the graphics driver fixes it.

## Run them yourself

The benchmarks are in the repository. With [Rust](https://rustup.rs) and Node.js installed:

```bash
cargo test --release -p circuit-editor bench_ -- --ignored --nocapture --test-threads=1
```

```bash
node runtime/bench/runtime.js
```

Both print tables like the ones above and write `target/bench/*.json`. The **Benchmarks** workflow on GitHub runs them on GitHub's Windows and Linux machines. Measured with XC Circuit 0.1.0 in November 2026. On the same kind of machine, Linux was a little faster than Windows: frames by 10–30%, bots run in the editor about twice as fast.
