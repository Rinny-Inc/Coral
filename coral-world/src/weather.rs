use rand::RngExt;

#[derive(Debug, Clone, PartialEq)]
pub enum WeatherState {
    Clear,
    Rain,
    Thunder,
}

pub struct Weather {
    pub state: WeatherState,
    pub ticks_remaining: i32,
    pub thunder_ticks_remaining: i32,
}
impl Weather {
    pub fn new() -> Self {
        Self {
            state: WeatherState::Clear,
            ticks_remaining: Self::random_clear_duration(),
            thunder_ticks_remaining: 0,
        }
    }

    fn random_clear_duration() -> i32 {
        rand::rng().random_range(12000..=180000)
    }
    fn random_rain_duration() -> i32 {
        rand::rng().random_range(12000..=24000)
    }
    fn random_thunder_duration() -> i32 {
        rand::rng().random_range(12000..=180000)
    }

    pub fn tick(&mut self) -> Option<WeatherState> {
        self.ticks_remaining -= 1;

        if self.ticks_remaining <= 0 {
            let new_state = match self.state {
                WeatherState::Clear => {
                    self.ticks_remaining = Self::random_rain_duration();
                    self.thunder_ticks_remaining = Self::random_thunder_duration();
                    WeatherState::Rain
                }
                WeatherState::Rain | WeatherState::Thunder => {
                    self.ticks_remaining = Self::random_clear_duration();
                    self.thunder_ticks_remaining = 0;
                    WeatherState::Clear
                }
            };
            self.state = new_state.clone();
            return Some(new_state);
        }

        if self.state == WeatherState::Rain {
            self.thunder_ticks_remaining -= 1;
            if self.thunder_ticks_remaining <= 0 {
                self.state = WeatherState::Thunder;
                self.thunder_ticks_remaining = Self::random_thunder_duration();
                return Some(WeatherState::Thunder);
            }
        }

        None
    }
}
