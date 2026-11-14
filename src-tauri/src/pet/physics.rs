//! Fisica e "cervello" del pet: logica pura (niente finestre né sistema operativo), così si
//! prova nei test. Tutte le misure sono in pixel fisici dello schermo; le costanti sono in
//! pixel logici e vengono moltiplicate per `k` (DPI × dimensione del pet).

/// Area di lavoro di un monitor (schermo meno la barra delle applicazioni).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub l: f64,
    pub t: f64,
    pub r: f64,
    pub b: f64,
}

impl Area {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.l && x <= self.r && y >= self.t && y <= self.b
    }
}

/// Bordo su cui si può stare in piedi: il bordo superiore di una finestra (solo la parte
/// visibile) o il fondo di un'area di lavoro (la barra delle applicazioni).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Platform {
    /// Stesso id per tutti i pezzi visibili della stessa finestra.
    pub id: u64,
    pub x0: f64,
    pub x1: f64,
    pub y: f64,
    /// Bordo sinistro della finestra intera: serve per seguirla quando si sposta.
    pub anchor: f64,
}

/// Id delle piattaforme "pavimento" (fondo dei monitor): `FLOOR + indice del monitor`.
pub const FLOOR: u64 = 1 << 62;

#[derive(Clone, Debug, Default)]
pub struct World {
    pub areas: Vec<Area>,
    pub platforms: Vec<Platform>,
}

impl World {
    /// Mondo con i soli monitor: il fondo di ogni area di lavoro è un pavimento.
    pub fn from_areas(areas: Vec<Area>) -> Self {
        let platforms = areas
            .iter()
            .enumerate()
            .map(|(i, a)| Platform { id: FLOOR + i as u64, x0: a.l, x1: a.r, y: a.b, anchor: a.l })
            .collect();
        Self { areas, platforms }
    }

    fn inside(&self, x: f64, y: f64) -> bool {
        self.areas.iter().any(|a| a.contains(x, y))
    }

    fn area_at(&self, x: f64, y: f64) -> Option<&Area> {
        self.areas.iter().find(|a| a.contains(x, y))
    }

    /// Il corpo largo `2·half` con i piedi in (x, y) sta dentro gli schermi?
    fn fits(&self, x: f64, y: f64, half: f64) -> bool {
        let y = y - 1.0;
        self.inside(x - half, y) && self.inside(x + half, y)
    }

    fn supports(&self, id: u64, x: f64) -> Option<&Platform> {
        self.platforms.iter().find(|p| p.id == id && x >= p.x0 && x <= p.x1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anim {
    Idle,
    Run,
    Jump,
    Fall,
    Held,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Idle { left: f64 },
    Walk { target: f64 },
    Air,
    Held,
}

/// Cose successe durante un passo, per la finestra del pet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Atterraggio dopo una caduta lunga: faccina preoccupata.
    HardLanding,
}

// costanti in pixel logici (× k)
const GRAVITY: f64 = 2200.0;
const TERMINAL: f64 = 1900.0;
const WALK: f64 = 75.0;
const JUMP_UP: f64 = 280.0;
const JUMP_DX: f64 = 420.0;
const MAX_VX: f64 = 560.0;
const HOP: f64 = 46.0;
const HARD_FALL: f64 = 420.0;
const THROW_MAX: f64 = 2600.0;

pub struct Pet {
    /// Centro dei piedi.
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    /// Mezza larghezza del corpo (pareti).
    pub half_w: f64,
    /// Altezza del corpo (soffitto).
    pub height: f64,
    /// Pixel fisici per pixel logico (DPI × dimensione).
    pub k: f64,
    pub facing_right: bool,
    /// Piattaforma sotto i piedi: (id, anchor e y all'ultimo passo).
    on: Option<(u64, f64, f64)>,
    mode: Mode,
    fall_from: f64,
    rng: u64,
}

impl Pet {
    pub fn new(x: f64, y: f64, k: f64, half_w: f64, height: f64, seed: u64) -> Self {
        Self { x, y, vx: 0.0, vy: 0.0, half_w, height, k, facing_right: false, on: None, mode: Mode::Air, fall_from: y, rng: seed | 1 }
    }

    pub fn anim(&self) -> Anim {
        match self.mode {
            Mode::Idle { .. } => Anim::Idle,
            Mode::Walk { .. } => Anim::Run,
            Mode::Held => Anim::Held,
            Mode::Air if self.vy < 0.0 => Anim::Jump,
            Mode::Air => Anim::Fall,
        }
    }

    pub fn on_ground(&self) -> bool {
        self.on.is_some()
    }

    /// Id della piattaforma sotto i piedi.
    #[cfg(test)]
    pub fn standing_on(&self) -> Option<u64> {
        self.on.map(|o| o.0)
    }

    fn rand(&mut self) -> f64 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.rand()
    }

    // ---------- comandi dall'esterno ----------

    /// Preso col mouse: la posizione la decide chi chiama.
    pub fn grab(&mut self) {
        self.mode = Mode::Held;
        self.on = None;
        self.vx = 0.0;
        self.vy = 0.0;
    }

    pub fn move_held(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
    }

    /// Lasciato andare: cade (o vola) con la velocità del mouse.
    pub fn release(&mut self, vx: f64, vy: f64) {
        let max = THROW_MAX * self.k;
        self.vx = vx.clamp(-max, max);
        self.vy = vy.clamp(-max, max);
        self.mode = Mode::Air;
        self.fall_from = self.y;
    }

    /// Lasciato senza trascinare (un click): resta dov'era.
    pub fn drop_in_place(&mut self) {
        self.release(0.0, 0.0);
    }

    /// Saltello sul posto (anche appena lasciato dalla mano, dopo un click).
    pub fn hop_in_air(&mut self) {
        self.vy = -(2.0 * GRAVITY * HOP).sqrt() * self.k;
        self.vx = 0.0;
        self.on = None;
        self.mode = Mode::Air;
        self.fall_from = self.y;
    }

    fn hop(&mut self) {
        if self.on.is_some() {
            self.hop_in_air();
        }
    }

    // ---------- passo di simulazione ----------

    pub fn step(&mut self, dt: f64, world: &World) -> Vec<Event> {
        let dt = dt.clamp(0.0, 0.05);
        let mut ev = Vec::new();
        match self.mode {
            Mode::Held => {}
            Mode::Air => self.step_air(dt, world, &mut ev),
            Mode::Idle { .. } | Mode::Walk { .. } => self.step_ground(dt, world),
        }
        ev
    }

    fn step_air(&mut self, dt: f64, world: &World, ev: &mut Vec<Event>) {
        let k = self.k;
        self.vy = (self.vy + GRAVITY * k * dt).min(TERMINAL * k);
        let mut nx = self.x + self.vx * dt;
        let ny = self.y + self.vy * dt;
        if self.vx.abs() > 1.0 {
            self.facing_right = self.vx > 0.0;
        }
        // pareti: rimbalzo morbido
        if !world.fits(nx, self.y.min(ny), self.half_w) && world.fits(self.x, self.y, self.half_w) {
            nx = self.x;
            self.vx = -self.vx * 0.4;
        }
        // soffitto
        let mut ny = ny;
        if self.vy < 0.0 {
            if let Some(a) = world.area_at(nx, self.y - 1.0) {
                if ny - self.height < a.t {
                    ny = a.t + self.height;
                    self.vy = 0.0;
                }
            }
        }
        // atterraggio: la prima piattaforma attraversata scendendo
        if self.vy >= 0.0 {
            let land = world
                .platforms
                .iter()
                .filter(|p| self.y <= p.y + 0.5 && ny >= p.y && nx >= p.x0 && nx <= p.x1)
                .min_by(|a, b| a.y.total_cmp(&b.y))
                .copied();
            if let Some(p) = land {
                self.x = nx;
                self.y = p.y;
                if p.y - self.fall_from > HARD_FALL * k {
                    ev.push(Event::HardLanding);
                }
                self.vx = 0.0;
                self.vy = 0.0;
                self.on = Some((p.id, p.anchor, p.y));
                let pause = self.range(0.3, 0.9);
                self.mode = Mode::Idle { left: pause };
                return;
            }
        }
        self.x = nx;
        self.y = ny;
        if self.vy <= 0.0 {
            self.fall_from = self.y; // l'altezza di caduta si conta dal punto più alto
        }
        // finito fuori dagli schermi (monitor staccato, lancio strano): torna sul pavimento più vicino
        if !world.inside(self.x, self.y - 1.0) {
            self.respawn(world);
        }
    }

    fn respawn(&mut self, world: &World) {
        let Some(a) = world
            .areas
            .iter()
            .min_by(|a, b| dist(a, self.x, self.y).total_cmp(&dist(b, self.x, self.y)))
            .copied()
        else {
            return;
        };
        self.x = self.x.clamp(a.l + self.half_w, a.r - self.half_w);
        self.y = a.t + self.height;
        self.vx = 0.0;
        self.vy = 0.0;
        self.fall_from = self.y;
    }

    fn step_ground(&mut self, dt: f64, world: &World) {
        let Some((id, anchor, py)) = self.on else {
            self.mode = Mode::Air;
            return;
        };
        // la finestra sotto i piedi si è spostata: la seguiamo
        if let Some(p) = world.platforms.iter().find(|p| p.id == id) {
            if (p.anchor - anchor).abs() > 0.5 || (p.y - py).abs() > 0.5 {
                self.x += p.anchor - anchor;
                self.y = p.y;
            }
        }
        let Some(p) = world.supports(id, self.x).copied() else {
            // finestra chiusa, minimizzata, coperta o finita oltre il bordo: si cade
            self.fall();
            return;
        };
        self.y = p.y;
        self.on = Some((id, p.anchor, p.y));
        match self.mode {
            Mode::Idle { left } => {
                let left = left - dt;
                self.mode = Mode::Idle { left };
                if left <= 0.0 {
                    self.decide(world, p);
                }
            }
            Mode::Walk { target } => {
                let dir = (target - self.x).signum();
                self.facing_right = dir > 0.0;
                let nx = self.x + dir * WALK * self.k * dt;
                if !world.fits(nx, self.y, self.half_w) {
                    self.idle(); // muro: si ferma
                } else if (target - nx) * dir <= 0.0 {
                    self.x = target;
                    self.idle();
                } else {
                    self.x = nx;
                }
                if world.supports(id, self.x).is_none() {
                    self.fall(); // oltre il bordo della finestra
                }
            }
            _ => {}
        }
    }

    fn fall(&mut self) {
        self.on = None;
        self.mode = Mode::Air;
        self.vx = 0.0;
        self.vy = 0.0;
        self.fall_from = self.y;
    }

    fn idle(&mut self) {
        let t = self.range(1.2, 4.5);
        self.mode = Mode::Idle { left: t };
    }

    /// Prossima cosa da fare, ferma su `p`.
    fn decide(&mut self, world: &World, p: Platform) {
        let r = self.rand();
        if r < 0.35 {
            self.idle();
        } else if r < 0.72 {
            let dist = self.range(60.0, 380.0) * self.k;
            let dir = if self.rand() < 0.5 { -1.0 } else { 1.0 };
            let mut target = self.x + dir * dist;
            // quasi sempre resta sulla sua finestra; ogni tanto scende dal bordo
            if self.rand() < 0.75 {
                target = target.clamp(p.x0 + self.half_w, (p.x1 - self.half_w).max(p.x0 + self.half_w));
            }
            if (target - self.x).abs() < 8.0 * self.k {
                self.idle();
            } else {
                self.mode = Mode::Walk { target };
            }
        } else if r < 0.93 {
            let cands = self.reachable(world, p.id);
            if cands.is_empty() {
                self.hop();
            } else {
                let i = ((self.rand() * cands.len() as f64) as usize).min(cands.len() - 1);
                let (tx, ty) = cands[i];
                self.jump_to(tx, ty);
            }
        } else {
            self.hop();
        }
    }

    /// Punti d'atterraggio raggiungibili con un salto (centro di un pezzo di piattaforma).
    pub fn reachable(&mut self, world: &World, current: u64) -> Vec<(f64, f64)> {
        let k = self.k;
        let mut out = Vec::new();
        for p in &world.platforms {
            let dy = self.y - p.y; // > 0: più in alto
            if p.id == current && dy.abs() < 2.0 {
                continue;
            }
            if dy > JUMP_UP * k || dy < -900.0 * k {
                continue;
            }
            let (lo, hi) = (p.x0 + self.half_w, p.x1 - self.half_w);
            if hi <= lo {
                continue;
            }
            let tx = self.x.clamp(lo, hi) + (self.rand() - 0.5) * (hi - lo).min(120.0 * k);
            let tx = tx.clamp(lo, hi);
            if (tx - self.x).abs() > JUMP_DX * k || (tx - self.x).abs() < 20.0 * k && dy.abs() < 20.0 * k {
                continue;
            }
            // niente salti verso un punto fuori dagli schermi
            if !world.fits(tx, p.y, self.half_w) {
                continue;
            }
            if self.jump_velocity(tx, p.y).is_some() {
                out.push((tx, p.y));
            }
        }
        out
    }

    /// Velocità iniziale per atterrare in (tx, ty) con un arco sopra il punto più alto.
    fn jump_velocity(&self, tx: f64, ty: f64) -> Option<(f64, f64)> {
        let g = GRAVITY * self.k;
        let rise = (self.y - ty).max(0.0) + 50.0 * self.k; // altezza dell'apice sopra i piedi
        let vy = -(2.0 * g * rise).sqrt();
        let t_up = (2.0 * rise / g).sqrt();
        let drop = rise - (self.y - ty); // dall'apice al bersaglio
        let t_down = (2.0 * drop / g).sqrt();
        let vx = (tx - self.x) / (t_up + t_down);
        (vx.abs() <= MAX_VX * self.k).then_some((vx, vy))
    }

    fn jump_to(&mut self, tx: f64, ty: f64) {
        let Some((vx, vy)) = self.jump_velocity(tx, ty) else {
            return self.hop();
        };
        self.vx = vx;
        self.vy = vy;
        self.facing_right = vx > 0.0;
        self.on = None;
        self.mode = Mode::Air;
        self.fall_from = self.y;
    }
}

fn dist(a: &Area, x: f64, y: f64) -> f64 {
    let dx = (a.l - x).max(0.0).max(x - a.r);
    let dy = (a.t - y).max(0.0).max(y - a.b);
    dx * dx + dy * dy
}

// ---------- piattaforme dalle finestre ----------

/// Rettangolo di una finestra vera (in ordine di z: la prima è quella davanti).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WinRect {
    pub id: u64,
    pub l: f64,
    pub t: f64,
    pub r: f64,
    pub b: f64,
    /// Si può stare sopra (false per le finestre che coprono e basta, es. massimizzate).
    pub standable: bool,
}

/// Bordi superiori visibili delle finestre: si tolgono i pezzi coperti da finestre davanti
/// e quelli troppo corti per starci sopra. `min_len` in pixel fisici.
pub fn platforms_from_windows(wins: &[WinRect], areas: &[Area], min_len: f64) -> Vec<Platform> {
    let mut out = Vec::new();
    for (i, w) in wins.iter().enumerate() {
        if !w.standable || w.r - w.l < min_len {
            continue;
        }
        // il bordo deve stare dentro un'area di lavoro (non sotto la barra né sopra lo schermo)
        let Some(area) = areas.iter().find(|a| w.t > a.t + 4.0 && w.t < a.b - 4.0 && w.r > a.l && w.l < a.r) else {
            continue;
        };
        let mut segs = vec![(w.l.max(area.l), w.r.min(area.r))];
        for above in &wins[..i] {
            if above.t <= w.t + 2.0 && above.b >= w.t - 2.0 {
                segs = segs
                    .into_iter()
                    .flat_map(|(a, b)| {
                        let mut v = Vec::new();
                        if above.l > a {
                            v.push((a, above.l.min(b)));
                        }
                        if above.r < b {
                            v.push((above.r.max(a), b));
                        }
                        v
                    })
                    .filter(|(a, b)| b - a >= min_len)
                    .collect();
            }
        }
        out.extend(segs.into_iter().map(|(x0, x1)| Platform { id: w.id, x0, x1, y: w.t, anchor: w.l }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> World {
        World::from_areas(vec![Area { l: 0.0, t: 0.0, r: 1920.0, b: 1040.0 }])
    }

    fn run(p: &mut Pet, w: &World, secs: f64) -> Vec<Event> {
        let mut ev = Vec::new();
        for _ in 0..(secs * 60.0) as usize {
            ev.extend(p.step(1.0 / 60.0, w));
        }
        ev
    }

    #[test]
    fn falls_onto_taskbar() {
        let w = screen();
        let mut p = Pet::new(500.0, 200.0, 1.0, 25.0, 80.0, 7);
        let mut ev = Vec::new();
        for _ in 0..120 {
            ev.extend(p.step(1.0 / 60.0, &w));
            if p.on_ground() {
                break;
            }
        }
        assert!(p.on_ground());
        assert_eq!(p.y, 1040.0);
        assert_eq!(p.standing_on(), Some(FLOOR));
        assert!(ev.contains(&Event::HardLanding));
    }

    #[test]
    fn lands_on_window_and_follows_it() {
        let mut w = screen();
        let win = [WinRect { id: 42, l: 300.0, t: 600.0, r: 900.0, b: 900.0, standable: true }];
        w.platforms.extend(platforms_from_windows(&win, &w.areas, 40.0));
        let mut p = Pet::new(500.0, 300.0, 1.0, 25.0, 80.0, 7);
        for _ in 0..40 {
            p.step(1.0 / 60.0, &w);
            if p.on_ground() {
                break;
            }
        }
        assert_eq!(p.standing_on(), Some(42));
        assert_eq!(p.y, 600.0);
        // la finestra si sposta di 100 a destra e 50 in alto: il pet la segue
        let x = p.x;
        let mut w2 = screen();
        let moved = [WinRect { id: 42, l: 400.0, t: 550.0, r: 1000.0, b: 850.0, standable: true }];
        w2.platforms.extend(platforms_from_windows(&moved, &w2.areas, 40.0));
        p.step(1.0 / 60.0, &w2);
        assert_eq!(p.standing_on(), Some(42));
        assert_eq!(p.y, 550.0);
        assert!((p.x - (x + 100.0)).abs() < 3.0, "{} vs {}", p.x, x + 100.0);
        // la finestra sparisce: cade sulla barra
        run(&mut p, &screen(), 2.0);
        assert_eq!(p.standing_on(), Some(FLOOR));
    }

    #[test]
    fn covered_edges_are_not_platforms() {
        let areas = [Area { l: 0.0, t: 0.0, r: 1920.0, b: 1040.0 }];
        let wins = [
            WinRect { id: 1, l: 200.0, t: 300.0, r: 500.0, b: 700.0, standable: true }, // davanti
            WinRect { id: 2, l: 100.0, t: 400.0, r: 900.0, b: 800.0, standable: true }, // dietro, bordo coperto in mezzo
            WinRect { id: 3, l: 0.0, t: 0.0, r: 1920.0, b: 1040.0, standable: false },  // massimizzata in fondo
        ];
        let p = platforms_from_windows(&wins, &areas, 40.0);
        let two: Vec<_> = p.iter().filter(|p| p.id == 2).map(|p| (p.x0, p.x1)).collect();
        assert_eq!(two, vec![(100.0, 200.0), (500.0, 900.0)]);
        assert!(p.iter().all(|p| p.id != 3));
        // una finestra davanti nasconde tutto quello che c'è dietro
        let behind = [WinRect { id: 9, l: 0.0, t: 0.0, r: 1920.0, b: 1040.0, standable: false }, wins[0]];
        assert!(platforms_from_windows(&behind, &areas, 40.0).is_empty());
    }

    #[test]
    fn walks_off_the_edge_and_falls() {
        let mut w = screen();
        w.platforms.push(Platform { id: 5, x0: 300.0, x1: 400.0, y: 500.0, anchor: 300.0 });
        let mut p = Pet::new(390.0, 500.0, 1.0, 25.0, 80.0, 3);
        p.on = Some((5, 300.0, 500.0));
        p.mode = Mode::Walk { target: 600.0 };
        run(&mut p, &w, 3.0);
        assert_eq!(p.standing_on(), Some(FLOOR));
    }

    #[test]
    fn walls_stop_walking_and_throws_bounce() {
        let w = screen();
        let mut p = Pet::new(1880.0, 1040.0, 1.0, 25.0, 80.0, 3);
        p.on = Some((FLOOR, 0.0, 1040.0));
        p.mode = Mode::Walk { target: 2500.0 };
        p.step(1.0 / 60.0, &w);
        p.step(1.0 / 60.0, &w);
        assert!(p.x + p.half_w <= 1920.0);
        // lanciato forte contro il bordo sinistro: resta dentro lo schermo
        let mut p = Pet::new(100.0, 500.0, 1.0, 25.0, 80.0, 3);
        p.grab();
        p.release(-5000.0, -3000.0);
        run(&mut p, &w, 4.0);
        assert!(p.x - p.half_w >= 0.0 && p.y - p.height >= 0.0);
        assert_eq!(p.standing_on(), Some(FLOOR));
    }

    #[test]
    fn jumps_land_where_aimed() {
        let mut w = screen();
        let win = [WinRect { id: 77, l: 700.0, t: 840.0, r: 1000.0, b: 1040.0, standable: true }];
        w.platforms.extend(platforms_from_windows(&win, &w.areas, 40.0));
        let mut p = Pet::new(500.0, 1040.0, 1.0, 25.0, 80.0, 11);
        p.on = Some((FLOOR, 0.0, 1040.0));
        let cands = p.reachable(&w, FLOOR);
        let &(tx, ty) = cands.iter().find(|c| c.1 == 840.0).expect("la finestra è raggiungibile");
        p.jump_to(tx, ty);
        for _ in 0..240 {
            p.step(1.0 / 60.0, &w);
            if p.on_ground() {
                break;
            }
        }
        assert_eq!(p.standing_on(), Some(77));
        assert!((p.x - tx).abs() < 15.0, "{} vs {tx}", p.x);
        // troppo in alto: non si prova
        let mut p = Pet::new(500.0, 1040.0, 1.0, 25.0, 80.0, 11);
        let mut w = screen();
        w.platforms.push(Platform { id: 8, x0: 450.0, x1: 600.0, y: 300.0, anchor: 450.0 });
        assert!(p.reachable(&w, FLOOR).iter().all(|c| c.1 != 300.0));
    }

    #[test]
    fn wanders_forever_without_leaving_the_screen() {
        let mut w = screen();
        let wins = [
            WinRect { id: 1, l: 200.0, t: 700.0, r: 700.0, b: 1000.0, standable: true },
            WinRect { id: 2, l: 600.0, t: 500.0, r: 1200.0, b: 900.0, standable: true },
            WinRect { id: 3, l: 1300.0, t: 800.0, r: 1900.0, b: 1040.0, standable: true },
        ];
        w.platforms.extend(platforms_from_windows(&wins, &w.areas, 40.0));
        let mut p = Pet::new(960.0, 100.0, 1.0, 25.0, 80.0, 1234);
        let mut ids = std::collections::BTreeSet::new();
        for _ in 0..60 * 600 {
            p.step(1.0 / 60.0, &w);
            assert!(p.x >= 0.0 && p.x <= 1920.0 && p.y <= 1040.0 + 0.01 && p.y >= 0.0, "{} {}", p.x, p.y);
            if let Some(id) = p.standing_on() {
                ids.insert(id);
            }
        }
        assert!(ids.len() >= 3, "visitate: {ids:?}");
    }

    #[test]
    fn two_monitors() {
        let w = World::from_areas(vec![Area { l: 0.0, t: 0.0, r: 1920.0, b: 1040.0 }, Area { l: 1920.0, t: 0.0, r: 3840.0, b: 2120.0 }]);
        let mut p = Pet::new(1900.0, 1040.0, 1.0, 25.0, 80.0, 5);
        p.on = Some((FLOOR, 0.0, 1040.0));
        p.mode = Mode::Walk { target: 2300.0 };
        run(&mut p, &w, 8.0);
        // passa sul secondo monitor (più alto) e ci cade dentro
        assert_eq!(p.standing_on(), Some(FLOOR + 1));
        assert_eq!(p.y, 2120.0);
    }
}
