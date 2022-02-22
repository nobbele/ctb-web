pub struct ScoreRecorder {
    pub combo: u32,
    pub top_combo: u32,
    pub max_combo: u32,

    pub hit_count: u32,
    pub miss_count: u32,

    /// This needs to be tracked separately due to floating point imprecision.
    pub internal_score: f32,
    pub chain_miss_count: u32,

    /// Max = 1,000,000
    pub score: u32,
    /// [0, 1]
    pub accuracy: f32,
    /// [0, 1]
    pub hp: f32,
}

fn polynomial(x: f32, coeffs: &[f32]) -> f32 {
    coeffs.iter().rev().fold(0., |acc, &c| acc * x + c)
}

impl ScoreRecorder {
    pub fn new(max_combo: u32) -> Self {
        ScoreRecorder {
            combo: 0,
            top_combo: 0,
            max_combo,
            hit_count: 0,
            miss_count: 0,
            internal_score: 0.,
            chain_miss_count: 0,
            score: 0,
            accuracy: 1.0,
            hp: 1.0,
        }
    }

    pub fn register_judgement(&mut self, hit: bool) {
        if hit {
            self.combo += 1;
            self.top_combo = self.top_combo.max(self.combo);

            self.internal_score += self.combo as f32 / self.max_combo as f32;
            self.score = (self.internal_score * 1_000_000. * 2. / (self.max_combo as f32 + 1.))
                .round() as u32;
            self.hit_count += 1;
            self.chain_miss_count = 0;

            self.hp += (self.combo as f32 / self.max_combo as f32) * 0.1;
            self.hp = self.hp.min(1.0);
        } else {
            self.combo = 0;
            self.miss_count += 1;

            #[allow(clippy::excessive_precision)]
            let hp_drain = polynomial(
                self.chain_miss_count as f32,
                &[
                    1.0029920966561545e+000,
                    7.4349034374388925e+000,
                    -9.1951466248253642e+000,
                    4.8111412580746844e+000,
                    -1.2397067078689683e+000,
                    1.7714300116489434e-001,
                    -1.4390229652509492e-002,
                    6.2392424752562498e-004,
                    -1.1231385529709802e-005,
                ],
            ) / 40.;
            dbg!(self.chain_miss_count);
            println!("{}%", hp_drain * 100.);
            self.hp -= hp_drain;
            self.hp = self.hp.max(0.);

            self.chain_miss_count += 1;
        }

        self.accuracy = self.hit_count as f32 / (self.hit_count + self.miss_count) as f32;
    }
}

#[test]
fn test_score_recorder_limits() {
    for max_combo in (1..256).step_by(13) {
        dbg!(max_combo);
        let mut recorder = ScoreRecorder::new(max_combo);
        for _ in 0..max_combo {
            recorder.register_judgement(true);
        }
        assert_eq!(recorder.score, 1_000_000);
    }
}

#[test]
fn test_hp() {
    let mut recorder = ScoreRecorder::new(100);
    assert_eq!(recorder.hp, 1.0);
    for _ in 0..10 {
        recorder.register_judgement(true);
    }
    assert_eq!(recorder.hp, 1.0);
    recorder.register_judgement(false);
    assert_eq!(recorder.hp, 0.9749252);
    for _ in 0..10 {
        recorder.register_judgement(true);
    }
    assert_eq!(recorder.hp, 1.0);
    for _ in 0..3 {
        recorder.register_judgement(false);
    }
    assert_eq!(recorder.hp, 0.8362208);
    recorder.register_judgement(true);
    for _ in 0..6 {
        recorder.register_judgement(false);
    }
    assert_eq!(recorder.hp, 0.22481588);
    recorder.register_judgement(true);
    for _ in 0..12 {
        recorder.register_judgement(false);
    }
    assert_eq!(recorder.hp, 0.0);
}
