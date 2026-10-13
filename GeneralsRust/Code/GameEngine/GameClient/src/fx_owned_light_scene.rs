//! W3DDisplay light admission and W3DDynamicLight frame fade under one scene owner.
use super::{DisplayDynamicLight, DisplayLightPulse, light_pulse_too_small};

#[derive(Default, Debug)]
pub struct DisplayLightScene {
    pulses: Vec<DisplayLightPulse>,
    lights: Vec<DisplayDynamicLight>,
}
impl DisplayLightScene {
    pub fn create_pulse(&mut self, pulse: DisplayLightPulse) -> bool {
        if light_pulse_too_small(pulse.inner_radius, pulse.outer_radius) {
            return false;
        }
        self.pulses.push(pulse.clone());
        self.lights.push(DisplayDynamicLight {
            pos: pulse.pos,
            color: pulse.color,
            far_atten_start: pulse.inner_radius,
            far_atten_end: pulse.inner_radius + pulse.outer_radius,
            increase_frames: pulse.increase_frames,
            decay_frames: pulse.decay_frames,
            cur_increase_frames: pulse.increase_frames,
            cur_decay_frames: pulse.decay_frames,
            target_color: pulse.color,
            target_far_atten_end: pulse.inner_radius + pulse.outer_radius,
            decay_range: true,
            decay_color: true,
            far_attenuation: true,
            enabled: true,
        });
        true
    }
    pub fn lights(&self) -> &[DisplayDynamicLight] {
        &self.lights
    }
    pub fn drain_pulses(&mut self) -> Vec<DisplayLightPulse> {
        self.pulses.drain(..).collect()
    }
    pub fn clear_lights(&mut self) {
        self.lights.clear();
    }
    pub fn update(&mut self) {
        for light in self.lights.iter_mut() {
            if !light.enabled {
                continue;
            }
            let factor = if light.cur_increase_frames > 0 && light.increase_frames > 0 {
                light.cur_increase_frames -= 1;
                (light.increase_frames - light.cur_increase_frames) as f32
                    / light.increase_frames as f32
            } else if light.decay_frames == 0 {
                1.0
            } else {
                light.cur_decay_frames = light.cur_decay_frames.saturating_sub(1);
                if light.cur_decay_frames == 0 {
                    light.enabled = false;
                    continue;
                }
                light.cur_decay_frames as f32 / light.decay_frames as f32
            };
            if light.decay_range {
                light.far_atten_end =
                    (factor * light.target_far_atten_end).max(light.far_atten_start);
            }
            if light.decay_color {
                light.color = [
                    light.target_color[0] * factor,
                    light.target_color[1] * factor,
                    light.target_color[2] * factor,
                ];
            }
        }
        self.lights.retain(|light| light.enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_admission_fade_and_clear_belong_to_the_receiving_scene() {
        let pulse = DisplayLightPulse {
            pos: [1.0, 2.0, 3.0],
            color: [1.0, 0.5, 0.25],
            inner_radius: 1.0,
            outer_radius: 30.0,
            increase_frames: 2,
            decay_frames: 2,
        };
        let mut first = DisplayLightScene::default();
        let mut peer = DisplayLightScene::default();
        assert!(first.create_pulse(pulse.clone()));
        assert!(peer.create_pulse(pulse.clone()));
        first.update();
        assert_eq!(first.lights()[0].color, [0.5, 0.25, 0.125]);
        assert_eq!(first.lights()[0].far_atten_end, 15.5);
        assert_eq!(peer.lights()[0].color, pulse.color);
        peer.clear_lights();
        assert_eq!(first.lights().len(), 1);
        assert!(peer.lights().is_empty());
        assert_eq!(first.drain_pulses(), vec![pulse.clone()]);
        assert_eq!(peer.drain_pulses(), vec![pulse]);
        first.update();
        assert_eq!(first.lights()[0].color, [1.0, 0.5, 0.25]);
        first.update();
        assert_eq!(first.lights()[0].color, [0.5, 0.25, 0.125]);
        first.update();
        assert!(first.lights().is_empty());
    }

    #[test]
    fn cpp_size_cull_admits_no_scene_state() {
        let mut scene = DisplayLightScene::default();
        assert!(!scene.create_pulse(DisplayLightPulse {
            pos: [0.0; 3],
            color: [1.0; 3],
            inner_radius: 1.0,
            outer_radius: 19.0,
            increase_frames: 0,
            decay_frames: 0,
        }));
        assert!(scene.lights().is_empty());
        assert!(scene.drain_pulses().is_empty());
    }
}
