# Tether design

This document gives the design rules for Tether. All work on the game must agree with this document.

## Concept

Tether is a visual puzzle game. The player changes the shape of space. The player does not move the traveler.

## Visual language

A reference video sets the look and the feel. The current renderer still uses the first flat paper look. The next work changes the renderer to the look in this section.

- The style is Bauhaus and Suprematist geometric abstraction, in 2.5D.
- The ground is dark charcoal. A faint, irregular lattice covers the ground. The lattice bends where the player pulls and welds. It shows the shape of space.
- The puzzle stays on one flat plane. A tilted perspective camera shows the plane in 3D.
- Ribbons are flat bands of color. Bauhaus forms with height float near the plane: planks, folded strips, and blocks. They cast soft shadows.
- Thin lines with small dot nodes float in the space.
- The game shows no text, no numbers, and no HUD.
- Animation, color, shape, and sound teach each rule.
- Color is rare. Each color has one meaning.

### Light and motion

- The pulse is a white orb with bloom and a short comet trail. Small sparks follow it.
- A held edge shows two corner handles and a ring at the cursor.
- A pulled ribbon lifts off the plane and twists to the other edge.
- A weld makes a cyan flash, a ring of cyan particles, and cyan bands at the seam.

### Palette

| Name | sRGB value | Use |
| --- | --- | --- |
| Ground | `#1C1E22` | Charcoal ground |
| Lattice | `#2C3036` | Faint lattice lines |
| Cream | `#E6DCC4` | Neutral edges (the class Ink) and planks |
| Red | `#C8563C` | Terracotta: red edges, square caps |
| Blue | `#34406A` | Slate navy: blue edges, circle caps |
| Yellow | `#D9A441` | Ochre: yellow edges, triangle caps |
| Pulse | `#FFFFFF` | The traveler, with bloom |
| Weld | `#3FE6F0` | Cyan weld light |

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
- A large stretch causes strain. The ribbon becomes thin, it crackles, and it vibrates.
- If the strain becomes too large, the edge comes out of the grip of the player.
- When the player releases an edge without a weld, the edge returns to its rest shape.

### Weld

- Each open edge has a class. The color and the shape of the edge cap show the class.

  | Class | Cap |
  | --- | --- |
  | Ink (cream) | Flat |
  | Yellow | Triangle |
  | Red | Square |
  | Blue | Circle |

- An edge can weld only to an edge of the same class.
- When a held edge comes near a compatible edge, two tones move to unison as the edges come nearer.
- When a held edge comes near a clashing edge, a soft dissonant rub sounds.
- A short-range magnet pulls the held edge in. On contact, the two edges weld with a flash. Then the grip releases.
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

## Window and camera

- The game starts in borderless fullscreen on the current monitor.
- F11 or Alt+Enter changes between fullscreen and a window.
- VSync is on.
- The camera stays close to the action and follows the pulse.
- The player can zoom in and out with the mouse wheel or with a pinch of two fingers.

## Sound

- A synthesizer in the code makes all sound. The game uses no audio files.
- Each class has a voice from the Kandinsky set. A soft drone breathes under each level.
- Strain makes a crackle. A weld makes a deep boom and a bell shimmer.
- M mutes the sound or turns it on again.
- Refer to [sound.md](sound.md) for the full sound design.

## Decisions

| Topic | Decision |
| --- | --- |
| Window and GPU | winit and wgpu only. We write the renderer, the shaders, the physics, and the game code. |
| Helper crates | We can use small helper crates, for example glam, bytemuck, pollster, serde, ron, and cpal. |
| Look | A reference video sets the look and the feel. |
| Depth | 2.5D. The puzzle stays on one plane. A perspective camera shows it in 3D. |
| Camera | Close follow of the pulse. The player can zoom in and out. |
| Palette | The video palette: charcoal, cream, terracotta, slate navy, ochre, a white pulse, and cyan weld light. It replaces the first Paper palette. |
| Deformation | Each ribbon is a particle chain. An XPBD solver moves the particles. |
| Weld | The weld forms on contact, with a short-range magnet. |
| Failure | The tether pulls the pulse back to the source. Welds stay. |
| Compatibility | Color and cap shape, from the Kandinsky set. |
| Sound | Procedural synthesis: Kandinsky voices, a breathing drone, tones that move to unison, and M to mute. cpal opens the audio device. |
| Order | First the sound, then the renderer for the video look. |
| Levels | RON files, read with serde. The binary contains all levels. |
| Flow | A complete level folds into the next level. Escape shows the map. |
| Window | Borderless fullscreen at start. |
| Copyright | Copyright (c) 2026 Brett Gilio. All rights reserved. |
