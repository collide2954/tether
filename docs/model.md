# Tether model

This document gives the mathematical model of the game world.

## Space

- The world is a 2D plane. The unit is the world unit (wu).
- The x axis points right. The y axis points up.
- Each level has a frame of 16 wu by 9 wu. The center of the frame is at the origin.
- The camera fits the frame in the window and keeps the aspect ratio. The area outside the frame is empty space.

## Ribbon

- A ribbon is a chain of n particles. At rest, the distance between two adjacent particles is h.
- The game samples the control points of the level at equal arc length to make the particles.
- Each particle i has these values:
  - The position $x_i$.
  - The previous position $p_i$.
  - The inverse mass $w_i$. A fixed particle has $w_i = 0$.
  - The rest position $r_i$.
- A motion (rotation, oscillation, or drift) can move all rest positions of a ribbon together.
- A ribbon has two ends. The head is particle 0. The tail is particle n − 1.
- Each end has one of these types:

  | Type | Pulse | Player |
  | --- | --- | --- |
  | Source | Starts here | Cannot move it |
  | Goal | Completes the level here | Cannot move it |
  | Cap | Stops here | Cannot move it |
  | Open | Falls off here | Can pull it and weld it |

## Free part

- Near an open end, the ribbon is free. Away from open ends, the ribbon stays on its rest shape.
- Each open end has a free length L.
- Let d be the rest arc length from a particle to the nearest open end.
- The stiffness of the particle is $s(d) = \mathrm{smoothstep}(L, L + h_{ramp}, d)$, with $h_{ramp} = 1.5$ wu.
- The shape compliance of the particle is $\alpha_{shape} \cdot 10^{6 (1 - s)}$. A particle with $s = 0$ has no shape constraint.
- The compliance changes through six decades. As a result, the clamp becomes weaker gradually, and the ribbon curves smoothly at the clamp.
- The free part is an elastic rod with a clamp at one side. A pull on the open end gives almost equal strain along the free part.
- The stretch and bend constraints keep the rest lengths and the rest curvature. As a result, a released free part returns to its rest shape.

## Solver

- The solver uses XPBD (extended position-based dynamics).
- The solver uses a fixed step $\Delta t$ and $s$ substeps for each step. Each substep has the length $\Delta t_s = \Delta t / s$.
- Each substep does these operations in this sequence:
  1. Calculate the velocity of each free particle: $v_i = x_i - p_i$.
  2. Apply internal damping: $v_i \leftarrow v_i + 0.3 \left( \tfrac{1}{2}(v_{i-1} + v_{i+1}) - v_i \right)$. This removes zig-zag oscillation and keeps smooth motion.
  3. Apply global damping: $v_i \leftarrow v_i \cdot e^{-c \Delta t_s}$.
  4. Set $p_i = x_i$. Then set $x_i = x_i + v_i$.
  5. Move each fixed particle to its target.
  6. Apply each constraint one time.

### Constraints

A constraint has a function C and a compliance α. A low α gives a stiff constraint.

| Constraint | C | Effect |
| --- | --- | --- |
| Stretch | $\lvert x_{i+1} - x_i \rvert - \ell_i$ | Keeps adjacent particles near the rest distance $\ell_i$. |
| Bend | $x_i - \tfrac{1}{2}(x_{i-k} + x_{i+k}) - R\,o_i^k$ | Keeps each particle at its rest offset from the midpoint of its neighbors. |
| Shape | $\lvert x_i - r_i \rvert$ | Pulls each particle to its rest position. The clamp profile sets the compliance. |
| Pin | $\lvert x_i - t \rvert$ | Holds a particle at a target t, for example the cursor. |
| Seam | $\lvert x_a - x_b \rvert$ | Holds two welded ends at the same point. |
| Seam bend | $x_s - \tfrac{1}{2}(x_{a'} + x_{b'})$ | Keeps the path straight through the seam. |

- Each substep applies each constraint one time. Thus λ starts at zero, and the update is:

$$\Delta\lambda = \frac{-C}{\sum_j w_j \lvert \nabla_j C \rvert^2 + \tilde\alpha}, \qquad \tilde\alpha = \frac{\alpha}{\Delta t_s^2}, \qquad x_j \leftarrow x_j + w_j \nabla_j C \, \Delta\lambda$$

### Bend at many scales

- The bend constraint is a vector constraint. Its gradient is $I$ for particle i and $-\tfrac{1}{2} I$ for each neighbor.
- $o_i^k = r_i - \tfrac{1}{2}(r_{i-k} + r_{i+k})$ is the rest offset at the stride k.
- R is the rotation from the rest chord $r_{i+k} - r_{i-k}$ to the current chord $x_{i+k} - x_{i-k}$. R does not scale the offset. A scaled offset can grow without limit at a corner.
- The solver applies the strides k = 12, 4, and 1, in this sequence. The compliance at the stride k is $\alpha_{bend} \cdot k^4$.
- The long strides move smooth shapes quickly. The short stride keeps the curve smooth between particles.

## Strain

- The strain of a segment is its length divided by h.
- The strain of a ribbon is the maximum segment strain.
- The drawn width of a segment is $w_0 / \sqrt{\text{strain}}$. As a result, a stretched ribbon becomes thin.
- The free strain of a ribbon is the current length of its soft segments divided by their rest length. A soft segment has a particle with $s < 1$.
- The free strain changes smoothly. A fast pull of the cursor does not cause a large change.
- Above the strain $s_{hum}$, the ribbon vibrates and hums.
- Above the free strain $s_{grip}$, the held edge comes out of the grip.
- Above the free strain $s_{tear}$, a weld tears. If the player holds that seam, the grip releases.

## Weld

- A weld connects two open ends: (ribbon a, end $e_a$) and (ribbon b, end $e_b$).
- Two open ends are compatible when they have the same class, they are different ends, and they have no weld.
- One ribbon can weld its two ends together to make a loop.
- The magnet finds the nearest compatible end in the radius $R_{mag}$. Let $p = \mathrm{smoothstep}(R_{mag}, R_{weld}, d)$, where d is the distance between the ends.
- In each substep, the grip target moves to the compatible end by the fraction p. The compatible end also moves a small distance to the held end.
- The pull becomes stronger when the ends come nearer. As a result, the held end snaps in.
- When the distance is less than $R_{weld}$, the weld forms at the midpoint and the grip releases.

## Traveler

- The state of the traveler is (ribbon, u, d).
  - u is a material coordinate in [0, n − 1]. The integer part of u is the segment index. The fractional part is the position in the segment.
  - d is the direction, +1 or −1.
- Because u is a material coordinate, the traveler moves with the ribbon when the ribbon changes shape.
- In each step, the traveler moves the distance $v \Delta t$ along the current polyline of the ribbon.
- At an end of a ribbon, these rules apply:

  | End | Result |
  | --- | --- |
  | Weld to (b, $e_b$) | The traveler continues on ribbon b, away from end $e_b$. |
  | Goal | The level is complete. |
  | Open, with no weld | The traveler falls off. Then the tether pulls it back. |
  | Cap | The traveler stops. Then the tether pulls it back. |

## Tether

- The tether is the list of ribbon intervals that the traveler crossed, in sequence.
- The game draws the tether on the ribbons. As a result, the tether moves with the ribbons.
- To pull the traveler back, the game moves the traveler backward along the tether at high speed.
