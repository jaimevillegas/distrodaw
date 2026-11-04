//! Envolvente ADSR. Los coeficientes se calculan fuera del bucle de audio
//! (al cambiar un parámetro), de modo que `next` solo hace sumas y productos.

/// Con este exponente la curva exponencial recorre ~99 % del camino en el
/// tiempo indicado.
const TARGET: f32 = 4.6;
/// Por debajo de este nivel (-60 dB) la relajación se da por terminada.
const SILENCE: f32 = 1e-3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvCoefs {
    attack_inc: f32,
    decay_coef: f32,
    sustain: f32,
    release_coef: f32,
}

impl EnvCoefs {
    /// Tiempos en segundos; `sustain` en 0.0..=1.0.
    pub fn new(sample_rate: f32, attack_s: f32, decay_s: f32, sustain: f32, release_s: f32) -> Self {
        Self {
            attack_inc: 1.0 / (attack_s * sample_rate).max(1.0),
            decay_coef: Self::coef(decay_s, sample_rate),
            sustain: sustain.clamp(0.0, 1.0),
            release_coef: Self::coef(release_s, sample_rate),
        }
    }

    fn coef(time_s: f32, sample_rate: f32) -> f32 {
        1.0 - (-TARGET / (time_s * sample_rate).max(1.0)).exp()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone, Copy)]
pub struct Adsr {
    stage: Stage,
    level: f32,
}

impl Default for Adsr {
    fn default() -> Self {
        Self { stage: Stage::Idle, level: 0.0 }
    }
}

impl Adsr {
    /// Inicia el ataque desde el nivel actual, sin saltos si la voz se reutiliza.
    pub fn trigger(&mut self) {
        self.stage = Stage::Attack;
    }

    /// Pasa a relajación (nota soltada).
    pub fn release(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn level(&self) -> f32 {
        self.level
    }

    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    /// Siguiente nivel de la envolvente, en 0.0..=1.0.
    #[inline]
    pub fn next(&mut self, c: &EnvCoefs) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += c.attack_inc;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level += (c.sustain - self.level) * c.decay_coef;
                if (self.level - c.sustain).abs() < 1e-4 {
                    self.level = c.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            // Sigue suavemente el sustain si cambia mientras suena.
            Stage::Sustain => self.level += (c.sustain - self.level) * c.decay_coef,
            Stage::Release => {
                self.level -= self.level * c.release_coef;
                if self.level < SILENCE {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn run(env: &mut Adsr, c: &EnvCoefs, samples: usize) {
        for _ in 0..samples {
            env.next(c);
        }
    }

    #[test]
    fn idle_stays_silent() {
        let c = EnvCoefs::new(SR, 0.01, 0.1, 0.5, 0.1);
        let mut env = Adsr::default();
        run(&mut env, &c, 100);
        assert!(env.is_idle());
        assert_eq!(env.level(), 0.0);
    }

    #[test]
    fn attack_reaches_peak_in_attack_time() {
        let c = EnvCoefs::new(SR, 0.01, 0.1, 0.5, 0.1);
        let mut env = Adsr::default();
        env.trigger();
        run(&mut env, &c, 479);
        assert_eq!(env.stage(), Stage::Attack);
        run(&mut env, &c, 2);
        assert_eq!(env.stage(), Stage::Decay);
        // Ya alcanzó el pico y empieza a caer hacia el sustain.
        assert!(env.level() > 0.99);
    }

    #[test]
    fn decay_settles_on_sustain() {
        let c = EnvCoefs::new(SR, 0.001, 0.05, 0.4, 0.1);
        let mut env = Adsr::default();
        env.trigger();
        run(&mut env, &c, (SR * 0.5) as usize);
        assert_eq!(env.stage(), Stage::Sustain);
        assert!((env.level() - 0.4).abs() < 1e-3);
    }

    #[test]
    fn release_falls_to_idle() {
        let c = EnvCoefs::new(SR, 0.001, 0.01, 0.8, 0.05);
        let mut env = Adsr::default();
        env.trigger();
        run(&mut env, &c, 4_800);
        env.release();
        assert_eq!(env.stage(), Stage::Release);
        let mut prev = env.level();
        for _ in 0..(SR * 0.5) as usize {
            let v = env.next(&c);
            assert!(v <= prev);
            prev = v;
        }
        assert!(env.is_idle());
        assert_eq!(env.level(), 0.0);
    }

    #[test]
    fn retrigger_continues_from_current_level() {
        let c = EnvCoefs::new(SR, 0.01, 0.1, 0.5, 0.2);
        let mut env = Adsr::default();
        env.trigger();
        run(&mut env, &c, 300);
        env.release();
        run(&mut env, &c, 100);
        let before = env.level();
        env.trigger();
        let after = env.next(&c);
        assert!(after >= before);
        assert!(after - before < 0.01);
    }

    #[test]
    fn release_of_idle_is_ignored() {
        let mut env = Adsr::default();
        env.release();
        assert!(env.is_idle());
    }
}
