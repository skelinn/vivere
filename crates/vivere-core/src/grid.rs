//! A dense uniform grid for neighbor queries — the degenerate (and fastest,
//! and deterministic) form of a spatial hash for a bounded wraparound world.
//! Buckets are plain `Vec`s filled in insertion order and iterated in fixed
//! cell order, so query results never depend on hashing or allocation state.
//! Rebuilt every tick; never serialized.

#[derive(Default)]
pub struct Grid {
    nx: usize,
    ny: usize,
    cell_w: f32,
    cell_h: f32,
    width: f32,
    height: f32,
    buckets: Vec<Vec<u32>>,
}

impl Grid {
    /// (Re)shape for the given world and clear all buckets. Cells are
    /// stretched so they tile the world exactly and are at least
    /// `min_cell` wide, so a 3×3 neighborhood always covers that radius.
    /// At least 3 cells per axis, or the wrap would visit cells twice.
    pub fn reset(&mut self, width: f32, height: f32, min_cell: f32) {
        let nx = ((width / min_cell) as usize).clamp(3, 1024);
        let ny = ((height / min_cell) as usize).clamp(3, 1024);
        if self.nx != nx || self.ny != ny || self.width != width || self.height != height {
            *self = Grid {
                nx,
                ny,
                cell_w: width / nx as f32,
                cell_h: height / ny as f32,
                width,
                height,
                buckets: vec![Vec::new(); nx * ny],
            };
        } else {
            for b in &mut self.buckets {
                b.clear();
            }
        }
    }

    fn cell(&self, x: f32, y: f32) -> (isize, isize) {
        let cx = ((x / self.cell_w) as isize).clamp(0, self.nx as isize - 1);
        let cy = ((y / self.cell_h) as isize).clamp(0, self.ny as isize - 1);
        (cx, cy)
    }

    pub fn insert(&mut self, x: f32, y: f32, idx: u32) {
        let (cx, cy) = self.cell(x, y);
        self.buckets[cy as usize * self.nx + cx as usize].push(idx);
    }

    /// Visit every entry in the wrapped 3×3 cell neighborhood of (x, y),
    /// in deterministic order.
    pub fn for_each_neighbor(&self, x: f32, y: f32, mut f: impl FnMut(u32)) {
        let (cx, cy) = self.cell(x, y);
        for dy in -1isize..=1 {
            for dx in -1isize..=1 {
                let gx = (cx + dx).rem_euclid(self.nx as isize) as usize;
                let gy = (cy + dy).rem_euclid(self.ny as isize) as usize;
                for &i in &self.buckets[gy * self.nx + gx] {
                    f(i);
                }
            }
        }
    }
}

/// Shortest signed displacement from `a` to `b` along a wrapped axis.
#[inline]
pub fn wrap_delta(a: f32, b: f32, extent: f32) -> f32 {
    let mut d = b - a;
    if d > extent * 0.5 {
        d -= extent;
    } else if d < -extent * 0.5 {
        d += extent;
    }
    d
}

/// Wrap an angle to [-π, π].
#[inline]
pub fn wrap_angle(a: f32) -> f32 {
    let mut a = a.rem_euclid(core::f32::consts::TAU);
    if a > core::f32::consts::PI {
        a -= core::f32::consts::TAU;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_delta_takes_shortest_path() {
        assert_eq!(wrap_delta(10.0, 20.0, 100.0), 10.0);
        assert_eq!(wrap_delta(95.0, 5.0, 100.0), 10.0);
        assert_eq!(wrap_delta(5.0, 95.0, 100.0), -10.0);
    }

    #[test]
    fn wrap_angle_stays_in_range() {
        for i in -100..100 {
            let a = wrap_angle(i as f32 * 0.37);
            assert!((-core::f32::consts::PI..=core::f32::consts::PI).contains(&a));
        }
    }

    #[test]
    fn grid_finds_everything_a_naive_scan_finds() {
        use crate::rng::Rng;
        let (w, h, r) = (400.0f32, 300.0f32, 64.0f32);
        let mut rng = Rng::from_seed(5);
        let pts: Vec<(f32, f32)> = (0..300)
            .map(|_| (rng.range_f32(0.0, w), rng.range_f32(0.0, h)))
            .collect();
        let mut grid = Grid::default();
        grid.reset(w, h, r);
        for (i, &(x, y)) in pts.iter().enumerate() {
            grid.insert(x, y, i as u32);
        }
        for _ in 0..200 {
            let (qx, qy) = (rng.range_f32(0.0, w), rng.range_f32(0.0, h));
            let mut from_grid: Vec<u32> = Vec::new();
            grid.for_each_neighbor(qx, qy, |i| {
                let (px, py) = pts[i as usize];
                let (dx, dy) = (wrap_delta(qx, px, w), wrap_delta(qy, py, h));
                if dx * dx + dy * dy <= r * r {
                    from_grid.push(i);
                }
            });
            let mut naive: Vec<u32> = (0..pts.len() as u32)
                .filter(|&i| {
                    let (px, py) = pts[i as usize];
                    let (dx, dy) = (wrap_delta(qx, px, w), wrap_delta(qy, py, h));
                    dx * dx + dy * dy <= r * r
                })
                .collect();
            from_grid.sort_unstable();
            naive.sort_unstable();
            assert_eq!(from_grid, naive);
        }
    }
}
