//! Mechanical study in millimeters; component envelopes, not fabrication files.
use std::{fmt::Write, fs, path::Path};
const PCB_W: f64 = 144.;
const PCB_D: f64 = 18.;
const CASE_W: f64 = 150.;
const CASE_D: f64 = 24.;
const LED_PITCH: f64 = 8.;
const LED_X: f64 = 10.;
const LED_Y: f64 = 5.;
const LENS: f64 = 2.9;
const FLANGE_MAX: f64 = 3.45;
const MOUNTS: [(f64, f64); 2] = [(4., 9.), (140., 5.)];
// SSOP driver and rotated JST SH header, including 0.5mm courtyards.
const COMPONENTS: [(f64, f64, f64, f64); 2] = [(72., 13., 9.65, 7.), (134., 12., 5.2, 9.)];
fn svg(w: f64, h: f64, body: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}"><style>text{{font-family:ui-monospace,monospace}} .dim{{font-size:2.7px;fill:#394d64}} .line{{stroke:#52728b;stroke-width:.25;fill:none}}</style>{body}</svg>"##
    )
}
fn front() -> String {
    let mut s = String::from(
        r##"<defs><linearGradient id="bezel" x2="0" y2="1"><stop stop-color="#49483f"/><stop offset=".16" stop-color="#292b26"/><stop offset="1" stop-color="#121511"/></linearGradient><radialGradient id="screw"><stop stop-color="#c2bda9"/><stop offset=".72" stop-color="#6e6b5d"/><stop offset="1" stop-color="#242721"/></radialGradient><radialGradient id="lens"><stop offset="0" stop-color="#ffb89b"/><stop offset=".28" stop-color="#de6248"/><stop offset=".8" stop-color="#9a3026"/><stop offset="1" stop-color="#4d201b"/></radialGradient></defs><rect x=".3" y=".3" width="149.4" height="23.4" rx="2" fill="url(#bezel)" stroke="#665f4b" stroke-width=".6"/><rect x="1.3" y="1.3" width="147.4" height="21.4" rx="1.3" fill="none" stroke="#070b08" stroke-width=".5"/>"##,
    );
    for (x, y) in MOUNTS {
        let (x, y) = (x + 3., y + 3.);
        write!(s,r##"<circle cx="{x}" cy="{y}" r="2.25" fill="url(#screw)"/><path d="M{} {y}h2.5" stroke="#282921" stroke-width=".55"/>"##,x-1.25).unwrap();
    }
    for i in 0..16 {
        let x = 3. + LED_X + i as f64 * LED_PITCH;
        let y = LED_Y + 3.;
        let fill = "#421f1b";
        write!(s,r##"<circle cx="{x}" cy="{y}" r="1.85" fill="#080b08" stroke="#635c49" stroke-width=".25"/><circle cx="{x}" cy="{y}" r="{}" fill="{fill}"/><ellipse cx="{}" cy="{}" rx=".5" ry=".27" fill="#efb4a0" opacity=".35"/><text x="{x}" y="15.5" text-anchor="middle" fill="#b8ae8f" font-size="2.2">{}</text>"##,LENS/2.,x-0.35,y-0.55,i+1).unwrap();
    }
    s.push_str(r##"<path d="M148 12h2v6h-2" fill="#080b08"/>"##);
    svg(CASE_W, CASE_D, &s)
}
fn pcb_body() -> String {
    let mut s = format!(
        r##"<rect width="{PCB_W}" height="{PCB_D}" rx="1" fill="#254e3e" stroke="#91a28b" stroke-width=".3"/>"##
    );
    for (x, y) in MOUNTS {
        write!(s,r##"<circle cx="{x}" cy="{y}" r="2.5" fill="none" stroke="#d1b16c" stroke-width=".25"/><circle cx="{x}" cy="{y}" r="1.35" fill="#111c17" stroke="#c7b985" stroke-width=".35"/>"##).unwrap();
    }
    for i in 0..16 {
        let x = LED_X + i as f64 * LED_PITCH;
        write!(s,r##"<circle cx="{x}" cy="{LED_Y}" r="{}" fill="none" stroke="#b5cbb2" stroke-width=".2"/><text x="{x}" y="11" text-anchor="middle" fill="#e1dfb9" font-size="2">{}</text>"##,FLANGE_MAX/2.,i+1).unwrap();
        for y in [LED_Y - 1.27, LED_Y + 1.27] {
            write!(s,r##"<circle cx="{x}" cy="{y}" r=".8" fill="#c6b57e"/><circle cx="{x}" cy="{y}" r=".45" fill="#132e23"/>"##).unwrap();
        }
    }
    s.push_str(r##"<rect x="67.175" y="9.5" width="9.65" height="7" rx=".3" fill="#132a22" stroke="#c8c8a5" stroke-width=".2" stroke-dasharray=".6 .5"/><rect x="67.675" y="10" width="8.65" height="6" fill="#202624"/><text x="72" y="12.7" text-anchor="middle" font-size="1.55" fill="#dcddc4">TLC59282</text><text x="72" y="15" text-anchor="middle" font-size="1.2" fill="#c8c8a5">UNDERSIDE</text><rect x="61" y="12" width="2" height="1.25" fill="#c8b584"/><rect x="81" y="12" width="1.6" height=".8" fill="#c8b584"/><text x="62" y="16.5" text-anchor="middle" font-size="1.3" fill="#dcddc4">C1</text><text x="82" y="16" text-anchor="middle" font-size="1.3" fill="#dcddc4">Rset</text><rect x="131.4" y="7.5" width="5.2" height="9" fill="none" stroke="#d7b37c" stroke-width=".25" stroke-dasharray=".5 .4"/><rect x="131.9" y="8" width="4.2" height="8" rx=".4" fill="#d9d6ba"/><text x="125" y="15.5" font-size="1.5" fill="#dcddc4">J1 →</text>"##);
    for (x, y, w, h) in COMPONENTS {
        write!(s,r##"<rect x="{}" y="{}" width="{w}" height="{h}" fill="none" stroke="#e1c688" stroke-width=".2" stroke-dasharray=".6 .5"/>"##,x-w/2.,y-h/2.).unwrap();
    }
    s
}
fn drawing() -> String {
    let mut s = String::from(
        r##"<rect width="180" height="114" fill="#f1eee4"/><text x="12" y="8" font-size="4" fill="#253e3c">AnsaLoop · sequencer module</text><text x="12" y="13" font-size="2.6" fill="#53615c">Mechanical proposal · mm · not a routed PCB or confirmed Panasonic fit</text><rect x="15" y="25" width="150" height="24" rx="2" fill="none" stroke="#4a5851" stroke-width=".4"/><path class="line" d="M15 24v-6m150 6v-6M15 20h150"/><text class="dim" x="90" y="19" text-anchor="middle">150 enclosure</text><path class="line" d="M166 25h7m-7 24h7m-3-24v24"/><text class="dim" x="174" y="37" transform="rotate(90 174 37)" text-anchor="middle">24</text><g transform="translate(18 28)">"##,
    );
    s.push_str(&pcb_body());
    s.push_str(r##"</g><path d="M165 40h8v9h-8" fill="none" stroke="#b67435" stroke-width=".4" stroke-dasharray="1 .7"/><text x="166" y="54" text-anchor="end" font-size="2.3" fill="#9b622d">8 mm cable / service reserve beyond housing</text><text x="15" y="59" font-size="2.8" fill="#253e3c">PCB: 144 × 18 × 1.6 · 16 LEDs · 8 mm center spacing</text><text x="15" y="64" font-size="2.8" fill="#253e3c">LED lens: Ø2.9 · flange envelope: Ø3.45 · holes: Ø0.9 / 2.54 pitch</text><text x="15" y="69" font-size="2.8" fill="#253e3c">Mounts: 2 × Ø2.7 · driver / Rset / bypass on reverse · 6-wire JST SH</text><g transform="translate(18 76)"><rect x="0" y="0" width="24" height="14" fill="none" stroke="#52728b" stroke-dasharray="1 .5" stroke-width=".2"/><rect x=".5" y="12.5" width="23" height="1.5" fill="#565b4e"/><path d="M.5 12.5V2h22.5v10.5" fill="none" stroke="#565b4e" stroke-width="1.5"/><rect x="3" y="7.9" width="18" height="1.6" fill="#42674f"/><rect x="12.5" y="9.5" width="6" height="1.75" fill="#252c29"/><path d="M6.55 6.9v-4.45a1.45 1.45 0 0 1 2.9 0v4.45z" fill="#b84c37"/><path d="M7.4 6.9v1m1.2-1v1" stroke="#b5a783" stroke-width=".35"/><path class="line" d="M26 0h3m-3 14h3m-1.5-14v14"/><text class="dim" x="31" y="7.5">14 mm</text></g><text x="66" y="81" font-size="2.7" fill="#253e3c">Raised above lid; 1 mm mounting pad makes 15 mm installed.</text><text x="66" y="86" font-size="2.7" fill="#253e3c">1.5 floor + 3 reverse clearance + 1.6 PCB</text><text x="66" y="91" font-size="2.7" fill="#253e3c">+ 1 lead standoff + 5.9 max LED body = 13.0</text><text x="15" y="102" font-size="2.7" fill="#7b4c32">Photo scale: estimated from 100.4 mm cassette length. Not a case measurement.</text><text x="15" y="107" font-size="2.7" fill="#7b4c32">Check lid structure, hinge sweep, cable loop and access before fabrication.</text>"##);
    svg(180., 114., &s)
}
pub fn generate(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(root.join("assets/sequencer-module.svg"), front())?;
    fs::write(
        root.join("assets/sequencer-pcb.svg"),
        svg(PCB_W, PCB_D, &pcb_body()),
    )?;
    fs::write(root.join("assets/sequencer-layout.svg"), drawing())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn led_pads_mounts_and_component_courtyards_have_real_board_clearance() {
        for i in 0..16 {
            let x = LED_X + i as f64 * LED_PITCH;
            assert!(x - FLANGE_MAX / 2. > 0. && x + FLANGE_MAX / 2. < PCB_W);
            for y in [LED_Y - 1.27, LED_Y + 1.27] {
                assert!(y - 0.8 > 0. && y + 0.8 < PCB_D);
                for (cx, cy, w, h) in COMPONENTS {
                    let dx = (x - cx).abs() - w / 2.;
                    let dy = (y - cy).abs() - h / 2.;
                    assert!(dx > 0.8 || dy > 0.8, "pad / underside courtyard collision");
                }
            }
            for (hx, hy) in MOUNTS {
                assert!((x - hx).hypot(LED_Y - hy) > 2.5 + FLANGE_MAX / 2.);
            }
        }
        for (cx, cy, w, h) in COMPONENTS {
            assert!(
                cx - w / 2. >= 0.
                    && cx + w / 2. <= PCB_W
                    && cy - h / 2. >= 0.
                    && cy + h / 2. <= PCB_D
            );
            for (hx, hy) in MOUNTS {
                let dx = ((hx - cx).abs() - w / 2.).max(0.);
                let dy = ((hy - cy).abs() - h / 2.).max(0.);
                assert!(dx.hypot(dy) > 2.5, "mount / component courtyard collision");
            }
        }
    }
}
