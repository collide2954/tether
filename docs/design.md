# Tether design

This document gives the design rules for Tether. All work on the game must agree with this document.

## Concept

Tether is a visual puzzle game. The player changes the shape of space. The player does not move the traveler.

## Visual language

- The style is Bauhaus and Suprematist geometric abstraction.
- Ribbons, planes, and lines float on warm paper. Large areas stay empty.
- The game shows no text, no numbers, no grid, and no HUD.
- Animation, color, shape, and sound teach each rule.
- Color is rare. Each color has one meaning.

### Palette

| Name | sRGB value | Use |
| --- | --- | --- |
| Ground | `#EFE9DD` | Paper, with faint grain |
| Ink | `#18171C` | Ribbons and ink edges |
| Red | `#D1301F` | Red edges, square caps |
| Yellow | `#F0B21A` | Yellow edges, triangle caps |
| Blue | `#1F4FA3` | Blue edges, circle caps |
| Pulse | White core, warm halo | The traveler |

## Rules

### Traveler

- The traveler is a pulse of light.
- The pulse leaves the source. It moves along the ribbon at a constant speed.
- A bright tether connects the pulse to the source. The tether shows the path of the pulse.
- When the pulse arrives at the goal, the level is complete.
- When the pulse falls off an open edge or stops at a cap, the tether pulls the pulse back to the source.
- After a short pause, the pulse leaves again. All welds stay.

### Stretch

- The player can pull the open edge of a ribbon. The free part of the ribbon stretches similar to elastic paper.
- A large stretch causes strain. The ribbon becomes thin and it vibrates.
- If the strain becomes too large, the edge comes out of the grip of the player.
- When the player releases an edge without a weld, the edge returns to its rest shape.

### Weld

- Each open edge has a class. The color and the shape of the edge cap show the class.

  | Class | Cap |
  | --- | --- |
  | Ink | Flat |
  | Yellow | Triangle |
  | Red | Square |
  | Blue | Circle |

- An edge can weld only to an edge of the same class.
- When a held edge comes near a compatible edge, the two edges pulse together and a harmonic tone sounds.
- A short-range magnet pulls the held edge in. On contact, the two edges weld with a pulse. Then the grip releases.
- The pulse can cross a weld from one ribbon to the other ribbon.
- To open a weld, pull the seam hard. The weld tears and the two edges return to their rest shapes.

### Mechanics for later levels

- One-way seam: The pulse can cross this seam one time. Then the seam tears open.
- Dynamic geometry: Some ribbons turn, oscillate, or drift. The player must weld at the correct time.
- Self-intersection: The pulse must not cross its own tether. The player must fold space to make a detour.

## Flow

- The game starts at the first level that is not complete.
- A complete level folds into the next level. There is no menu.
- Escape shows a map of the complete levels as points of light. Click a point to play that level again.

## Window

- The game starts in borderless fullscreen on the current monitor.
- F11 or Alt+Enter changes between fullscreen and a window.
- VSync is on.

## Sound

- A synthesizer in the code makes all sound. The game uses no audio files.
- Near a compatible edge, a harmonic tone sounds. A weld makes a chime. Strain makes a hum. An arrival makes a chord.

## Decisions

| Topic | Decision |
| --- | --- |
| Window and GPU | winit and wgpu only. We write the renderer, the shaders, the physics, and the game code. |
| Helper crates | We can use small helper crates, for example glam, bytemuck, pollster, serde, and ron. |
| Palette | Paper: a warm light ground with ink and three primary colors. |
| Deformation | Each ribbon is a particle chain. An XPBD solver moves the particles. |
| Weld | The weld forms on contact, with a short-range magnet. |
| Failure | The tether pulls the pulse back to the source. Welds stay. |
| Compatibility | Color and cap shape, from the Kandinsky set. |
| Sound | Procedural synthesis. cpal opens the audio device. |
| Levels | RON files, read with serde. The binary contains all levels. |
| Flow | A complete level folds into the next level. Escape shows the map. |
| Window | Borderless fullscreen at start. |
| Copyright | Copyright (c) 2026 Brett Gilio. All rights reserved. |
