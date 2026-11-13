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
- The shape stiffness of the particle is $k(d) = k_{max} \cdot \mathrm{smoothstep}(L, L + h_{ramp}, d)$.
- As a result, the free part is an elastic rod with a clamp at one side. A pull on the open end gives almost equal strain along the free part.
- The stretch and bend constraints keep the rest lengths and the rest angles. As a result, a released free part returns to its rest shape.

## Solver

- The solver uses XPBD (extended position-based dynamics).
- The solver uses a fixed step $\Delta t$ and $s$ substeps for each step. Each substep has the length $\Delta t_s = \Delta t / s$.
- Each substep does these operations in this sequence:
  1. Calculate the velocity of each free particle: $v_i = (x_i - p_i) \cdot e^{-c \Delta t_s}$.
  2. Set $p_i = x_i$. Then set $x_i = x_i + v_i$.
  3. Move each fixed particle to its target.
  4. Apply each constraint one time.

### Constraints

A constraint has a function C and a compliance α. A low α gives a stiff constraint.

| Constraint | C | Effect |
| --- | --- | --- |
| Stretch | $\lvert x_{i+1} - x_i \rvert - h$ | Keeps adjacent particles near the rest distance. |
| Bend | $\theta_i - \theta^0_i$ | Keeps the turn angle at each particle near its rest angle. |
| Shape | $\lvert x_i - r_i \rvert$ | Pulls each particle to its rest position, with the stiffness k(d). |
| Pin | $\lvert x_i - t \rvert$ | Holds a particle at a target t, for example the cursor. |
| Seam | $\lvert x_a - x_b \rvert$ | Holds two welded ends at the same point. |
| Seam bend | $\theta_{seam}$ | Keeps the path straight through the seam. |

- $\theta_i$ is the signed angle from segment (i − 1, i) to segment (i, i + 1).
- Each substep applies each constraint one time. Thus λ starts at zero, and the update is:

$$\Delta\lambda = \frac{-C}{\sum_j w_j \lvert \nabla_j C \rvert^2 + \tilde\alpha}, \qquad \tilde\alpha = \frac{\alpha}{\Delta t_s^2}, \qquad x_j \leftarrow x_j + w_j \nabla_j C \, \Delta\lambda$$

- For the bend constraint, with $a = x_i - x_{i-1}$, $b = x_{i+1} - x_i$, and $\perp(v) = (-v_y, v_x)$:

$$\nabla_{i-1}\theta = \frac{\perp(a)}{\lvert a \rvert^2}, \qquad \nabla_{i+1}\theta = \frac{\perp(b)}{\lvert b \rvert^2}, \qquad \nabla_i\theta = -\nabla_{i-1}\theta - \nabla_{i+1}\theta$$

## Strain

- The strain of a segment is its length divided by h.
- The strain of a ribbon is the maximum segment strain.
- The drawn width of a segment is $w_0 / \sqrt{\text{strain}}$. As a result, a stretched ribbon becomes thin.
- Above the strain $s_{hum}$, the ribbon vibrates and hums.
- Above the strain $s_{grip}$, the held edge comes out of the grip.
- Above the strain $s_{tear}$, a weld tears.

## Weld

- A weld connects two open ends: (ribbon a, end $e_a$) and (ribbon b, end $e_b$).
- Two open ends are compatible when they have the same class and they are different ends.
- One ribbon can weld its two ends together to make a loop.
- In each substep, a magnet pulls the held end to the nearest compatible free end in the radius $R_{mag}$.
- When the distance between the two ends is less than $R_{weld}$, the weld forms.

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
