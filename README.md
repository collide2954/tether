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
- Refer to [docs/design.md](docs/design.md) for the game design.
- Refer to [docs/model.md](docs/model.md) for the mathematical model.

## License

Copyright (c) 2026 Brett Gilio. All rights reserved. Tether is proprietary software. Refer to [LICENSE](LICENSE).
