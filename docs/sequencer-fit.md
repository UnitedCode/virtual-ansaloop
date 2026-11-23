# Sequencer LED module — mechanical proposal

This replaces the decorative thin strip. Dimensions below are design choices
based on actual component drawings. This is a space study, not a routed PCB,
electrical verification, or proof of fit in the Panasonic.

## Size and construction

| Item | Proposed space |
| --- | --- |
| Enclosure | 150 × 24 × 14 mm maximum, including LED lenses |
| Temporary mounting pad | 1 mm; total installed height 15 mm above the lid |
| Two-layer PCB | 144 × 18 × 1.6 mm, with 3 mm perimeter allowance inside enclosure |
| Indicators | 16 red 3 mm through-hole LEDs, 8 mm center spacing; 120 mm first-to-last span |
| LED locations on PCB | x = 10 + 8n mm, y = 5 mm, n = 0…15 |
| Internal module fasteners | M2.5, Ø2.7 mm holes at (4,9) and (140,5); Ø5 mm component/copper keepouts |
| Driver | TLC59282DBQ SSOP on reverse; 9.65 × 7 mm courtyard centered at (72,13) |
| Connector | Side-entry six-pin JST SH, rotated toward right end; 5.2 × 9 mm courtyard at (134,12) |
| Other components | Current-setting resistor and local bypass capacitor on reverse |
| Wiring | 6 conductors: power, ground, data, clock, latch, blank; reserve 8 mm beyond housing and a proposed 10 mm-radius hinge service loop |

The driver/connector courtyards, LED pads and mounting keepouts are checked for
overlap by the Rust layout test. The drawings do not contain PCB traces or a
completed netlist. Housing, attachment and cable strain relief still need design
and mechanical validation. Internal fasteners assemble the module; drilling
the Panasonic lid is not assumed.

Depth allowance: 1.5 mm floor + 3 mm reverse-component clearance + 1.6 mm PCB +
1 mm LED lead standoff + 5.9 mm maximum LED body = 13 mm. A 14 mm envelope leaves
1 mm allowance. The 1.75 mm driver fits within the reverse clearance with 1.25 mm
to the floor. A separate 1 mm mounting pad brings the installed envelope to 15 mm.
This raised design assumes no available cavity underneath the lid.

## Component sources

- [Kingbright WP710A10ID drawing](https://www.kingbrightusa.com/images/catalog/spec/WP710A10ID.pdf): nominal Ø2.9 mm lens, Ø3.2 mm flange, body height 5.4 ±0.5 mm; two Ø0.9 mm holes on 2.54 mm pitch. The drawing uses a Ø3.45 mm flange envelope and Ø3.3 mm face opening.
- [TI TLC59282](https://www.ti.com/product/TLC59282) and [datasheet](https://www.ti.com/lit/ds/symlink/tlc59282.pdf): 16 constant-current channels, 3.3 V-compatible logic, SSOP-24 package 8.65 × 6 mm overall. The selected package has a 1.75 mm maximum height; the proposal adds courtyard allowance.
- [JST SH drawing](https://www.jst-mfg.com/product/pdf/eng/eSH.pdf): six-position SM06B-SRSS-TB, 1 mm pitch, approximately 8 × 4.2 mm side-entry package and 2.9 mm height. The proposal reserves additional perimeter and cable space.

Driver current, actual brightness through the lid, power supply, EMI/noise and
firmware pin allocation remain to be designed. The browser overlays firmware
pattern and playhead state on these lens positions; it does not add physical
step buttons. The actual TLC59282 GPIO/serial driver is not implemented yet.

## Panasonic scale and remaining uncertainty

Exact-model parts, photographs and manufacturer packaging were searched. No
verified dimensioned KX-T1421 case/lid drawing or under-lid clearance was found.
The [open-lid photograph](https://www.ebay.com/itm/336319829810) shows both tape
positions, hinges and the existing lower ledge, but supplies no measurements.

The mockup uses cassette-based scale: [standard compact-cassette dimensions in
this Philips patent](https://patents.google.com/patent/EP0557726B1/en) give
100.4 × 63.8 mm. The existing cassette artwork occupies 39% of the 1230-pixel
reference canvas before rotation. Treating its long side as 100.4 mm yields
a canvas width of 257.44 mm. The 150 mm module therefore occupies 58.267% of
the canvas width. This is an estimate using reconstructed artwork, not a
measurement of the real machine; a single photo cannot establish hinge sweep
or interior clearance.

The module deliberately occupies a substantial part of the lower lid. Confirm
the exact exterior dimensions, lid stiffness, attachment area, opening sweep,
cassette access and harness route before making parts. Do not shrink its LEDs,
PCB or depth merely to make the photograph appear to fit.

`cargo run --release -p xtask -- layout` regenerates the front view, PCB space
view and dimensioned assembly SVG from `xtask/src/layout.rs`. The SVG geometry
uses millimeters and remains independent of viewport size.
