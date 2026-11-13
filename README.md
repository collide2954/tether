# Tether

Tether is an abstract puzzle game. You fold space to make a path for a pulse of light.

## How to play

- A pulse of light moves along a ribbon at a constant speed.
- If you do nothing, the pulse falls off an open edge. Its tether then pulls it back to the source.
- Pull an open edge to stretch the ribbon.
- Move the edge near an edge with the same color and shape. The two edges weld together.
- The pulse moves across the weld to the other ribbon.
- Make a path to the goal for the pulse.

The game has no text and no tutorial. Shape, color, motion, and sound show each rule.

## Controls

| Input | Result |
| --- | --- |
| Drag an open edge | Stretch the ribbon |
| Drag a seam hard | Tear the weld open |
| F11, Alt+Enter, or Control+Command+F | Change between fullscreen and a window |
| M | Mute the sound or turn it on again |

You can also use a touch screen.

## Platforms

- Windows 10 or later (DirectX 12)
- macOS 11 or later (Metal)
- Linux with X11 or Wayland (Vulkan)

## Build

1. Install the stable Rust toolchain from <https://rustup.rs>.
2. On Linux, install the ALSA development files. On Debian or Ubuntu, use this command:

   ```bash
   sudo apt-get install libasound2-dev
   ```

3. Build and start the game:

   ```bash
   cargo run --release
   ```

## Development

- Use test-driven development. First, write a test that fails. Then, write the code that makes the test pass.
- Before each commit, use these commands:

  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

- To make Git check the format and the lints before each commit, use this command one time:

  ```bash
  git config core.hooksPath .githooks
  ```

- Write all documentation, code comments, and commit messages in ASD-STE100 Simplified Technical English.
- To see a frame without a window, use the capture options. The game writes a PNG file and stops.

  ```bash
  cargo run -- --capture frame.png --level 2 --at 5.5 --drag -1.2,-0.6,1.2,0.6
  ```

  | Option | Value |
  | --- | --- |
  | `--windowed` | Start in a window, not in fullscreen. |
  | `--capture` | The PNG file to write. |
  | `--level` | The level index. The first level is 0. |
  | `--at` | The game time of the frame, in seconds. |
  | `--size` | The frame size in pixels, for example `1920x1080`. |
  | `--drag` | A drag from x0,y0 to x1,y1 in world units, during the first second. |
  | `--listen` | A WAV file. The game plays the level with the drag and writes the sound. The default time is 6 s. |
  | `--catalog` | A WAV file. The game writes a tour of all sounds. |

- Refer to [docs/sound.md](docs/sound.md) for the sound design.
- Refer to [docs/design.md](docs/design.md) for the game design.
- Refer to [docs/model.md](docs/model.md) for the mathematical model.

## License

Copyright (c) 2026 Brett Gilio. All rights reserved. Tether is proprietary software. Refer to [LICENSE](LICENSE).
