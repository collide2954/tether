# Tether sound

This document gives the sound design of Tether. A synthesizer in the code makes all sound. The game uses no audio files.

## Tuning

- All pitches come from one just-intoned pentatonic scale. The ratios are 1, 9/8, 5/4, 3/2, and 5/3.
- Each level has a root. The root of level n is $110 \cdot (3/2)^n$ Hz, folded into the octave from 110 Hz to 220 Hz.
- As a result, each new level moves one step on the circle of fifths.

## Voices

Each edge class has its own voice. The voices follow the color and sound pairs of Kandinsky.

| Class | Voice | Pitch |
| --- | --- | --- |
| Blue | Organ: soft sine partials with a slow attack | 2 × root |
| Ink | Wood: a short, dry knock with inharmonic FM | 3 × root |
| Red | Horn: warm FM brass | 5 × root |
| Yellow | Trumpet: bright FM brass with a sharp attack | 9 × root |

## Cues

| Moment | Sound |
| --- | --- |
| The player grips an edge | The voice of the class, soft |
| A compatible edge is near (2 wu or less) | Two close tones. The detune is 0.03 × distance. The beat becomes slower until the tones are in unison. |
| A clashing edge is near (0.9 wu or less) | One soft rub of a minor second. The rub becomes ready again at 1.3 wu. |
| The ribbon stretches | A crackle. Its density and level increase with the free strain. |
| A weld forms | A sub-bass boom at root / 2, a bell shimmer at 12 × root, and the voice of the class |
| A weld tears | A paper rip and a knock |
| The grip slips | A plucked string at 2 × root |
| The pulse departs | A wood tick |
| The pulse crosses a seam | A sparkle and the voice of the class |
| The pulse falls off an edge | A glide down one octave |
| The tether pulls the pulse back | A glide up one octave |
| The pulse stops at a dead end | A knock |
| The pulse arrives | A horn arpeggio (1, 5/4, 3/2, 2), a shimmer, and a soft boom |
| A new level starts | A whoosh. The drone moves to the new root. |

## Drone

- The drone plays the partials 1, 2, 3, and 4 × root. It breathes with a slow amplitude cycle of 7 s.
- Strain adds tension. Tension adds the partials 9/4 and 45/16 × root, opens the filter, and makes the breath faster (3 s).
- When the pulse arrives, the tension goes back to zero.

## Mix

- The synthesizer has 32 voices for short sounds. When all voices are in use, the oldest voice stops.
- A stereo reverb (8 comb filters and 4 allpass filters for each channel) gives the room. Each sound has its own reverb send.
- An equal-power pan follows the x position of the sound.
- A soft clip on the master keeps each sample between −1 and 1.
- M fades the master out or back in, in approximately 0.15 s.

## Real-time rules

- The audio thread does not lock and does not allocate memory in normal operation.
- The game sends commands through a bounded channel. If the channel is full, the game discards the command.
- The game continues without sound if no audio device is available.

## Previews

Use these commands to write the sound to a WAV file. The game does not open a window.

```bash
cargo run -- --catalog catalog.wav
```

```bash
cargo run -- --listen level.wav --level 0 --at 9 --drag -1.2,-0.6,1.2,0.6
```

- `--catalog` writes a 29 s tour of all sounds.
- `--listen` plays the real game with the score and the synthesizer, and writes what the player would hear.
