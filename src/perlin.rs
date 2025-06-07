use rand::{Rng, SeedableRng};

#[derive(Clone)]
pub struct PerlinNoise {
    perm: [usize; 512],
    octaves: usize,
    fallout: f64,
}


impl PerlinNoise {
    pub fn new(seed: u64) -> PerlinNoise {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
        let mut perm = [0; 512];

        for i in 0..256 {
            perm[i] = i;
        }

        for i in 0..256 {
            let j = rng.random_range(0..256) & 0xFF;
            let t = perm[j];

            perm[j] = perm[i];
            perm[i] = t;
        }


        for i in 0..256 {
            perm[i + 256] = perm[i];
        }


        PerlinNoise {
            perm,
            octaves: 4,
            fallout: 0.7,
        }

    }


    /// Returns Perlin noise in 2D
    pub fn get2d(&self, args: [f64; 2]) -> f64 {
        let mut effect = 1.0;
        let mut frequency = 1.0;
        let mut sum = 0.0;
        let mut max_amp = 0.0;

        for _ in 0..self.octaves {
            let x = frequency * args[0];
            let y = frequency * args[1];

            sum += effect * self.noise2d(x, y);
            max_amp += effect;

            frequency *= 2.0;
            effect *= self.fallout;
        }

        // Normalize to [0, 1]
        (sum / max_amp + 1.0) / 2.0
    }

    fn noise2d(&self, mut x: f64, mut y: f64) -> f64 {
        let xi = x.floor() as usize & 255;
        let yi = y.floor() as usize & 255;

        x -= x.floor();
        y -= y.floor();

        let u = fade(x);
        let v = fade(y);

        let aa = self.perm[self.perm[xi] + yi];
        let ab = self.perm[self.perm[xi] + (yi + 1) & 255];
        let ba = self.perm[self.perm[(xi + 1) & 255] + yi];
        let bb = self.perm[self.perm[(xi + 1) & 255] + (yi + 1) & 255];

        let x1 = lerp(u, grad2d(aa as _, x, y), grad2d(ba as _, x - 1.0, y));
        let x2 = lerp(u, grad2d(ab as _, x, y - 1.0), grad2d(bb as _, x - 1.0, y - 1.0));

        lerp(v, x1, x2)
    }
}


/*
fn grad2d(hash: usize, x: f64, y: f64) -> f64 {
    let v = if hash & 1 == 0 { x } else { y };

    if (hash & 1) == 0 {
        -v
    } else {
        v
    }
}


fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
*/

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

fn grad2d(hash: u8, x: f64, y: f64) -> f64 {
    match hash & 3 {
        0 =>  x + y,
        1 => -x + y,
        2 =>  x - y,
        3 => -x - y,
        _ => 0.0, // unreachable
    }
}
