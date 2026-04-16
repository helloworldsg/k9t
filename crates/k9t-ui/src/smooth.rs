#[derive(Debug, Clone)]
pub struct SmoothedValue {
    current: f64,
    target: f64,
    alpha: f64,
}

impl SmoothedValue {
    pub fn new(initial: f64, alpha: f64) -> Self {
        Self {
            current: initial,
            target: initial,
            alpha,
        }
    }

    pub fn set_target(&mut self, target: f64) {
        self.target = target;
    }

    pub fn update(&mut self) -> f64 {
        self.current = self.current + self.alpha * (self.target - self.current);
        self.current
    }

    pub fn current(&self) -> f64 {
        self.current
    }

    pub fn target(&self) -> f64 {
        self.target
    }
}

pub struct SmoothedGauge {
    pub values: [SmoothedValue; 4],
}

impl SmoothedGauge {
    pub fn new(alpha: f64) -> Self {
        Self {
            values: [
                SmoothedValue::new(0.0, alpha),
                SmoothedValue::new(0.0, alpha),
                SmoothedValue::new(0.0, alpha),
                SmoothedValue::new(0.0, alpha),
            ],
        }
    }

    pub fn set_targets(&mut self, cpu: f64, mem: f64, net_rx: f64, net_tx: f64) {
        self.values[0].set_target(cpu);
        self.values[1].set_target(mem);
        self.values[2].set_target(net_rx);
        self.values[3].set_target(net_tx);
    }

    pub fn update(&mut self) -> [f64; 4] {
        [
            self.values[0].update(),
            self.values[1].update(),
            self.values[2].update(),
            self.values[3].update(),
        ]
    }
}
