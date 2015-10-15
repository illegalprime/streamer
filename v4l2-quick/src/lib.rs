extern crate rscam;

use std::cmp::Ordering;

pub use self::rscam::{Camera, Config, FormatInfo, ResolutionInfo, IntervalInfo};
pub use self::rscam::Result as V4l2Result;

const MJPG: &'static [u8] = b"MJPG";
const MIN_RES: (u32, u32) = (640, 480);

pub fn camera(path: &str) -> V4l2Result<(Camera, u32)> {
    let mut camera = Camera::new(path).unwrap();

    let mut formats: Vec<FormatInfo> = camera.formats()
        .filter_map(Result::ok)
        .collect();
    formats.sort_by(|a, b| {
        if a.emulated && !b.emulated {
            return Ordering::Less;
        } else if b.emulated && !a.emulated {
            return Ordering::Greater;
        }
        if a.compressed && !b.compressed {
            return Ordering::Greater;
        } else if b.compressed && !a.compressed {
            return Ordering::Less;
        }
        if a.format == MJPG && b.format != MJPG {
            return Ordering::Greater;
        } else if b.format == MJPG && a.format != MJPG {
            return Ordering::Less;
        }
        Ordering::Equal
    });
    let format = formats.last().unwrap();
    let format = format.format;

    let resolution = match camera.resolutions(&format).unwrap() {
        ResolutionInfo::Discretes(discretes) => {
            let mut discretes: Vec<(u32, u32)> = discretes.into_iter()
                .filter(|r| r.0 >= MIN_RES.0 && r.1 >= MIN_RES.1)
                .collect();
            discretes.sort();
            discretes.first().unwrap().clone()
        },
        ResolutionInfo::Stepwise{ min, step, .. } => {
            if min >= MIN_RES {
                min
            } else {
                let steps_x = ((MIN_RES.0 - min.0 / step.0) as f64 + 0.5) as u32;
                let steps_y = ((MIN_RES.1 - min.1 / step.1) as f64 + 0.5) as u32;
                let steps = if steps_x > steps_y {
                    steps_x
                } else {
                    steps_y
                };
                (min.0 + step.0 * steps, min.1 + step.1 * steps)
            }
        },
    };

    let interval = match camera.intervals(&format, resolution).unwrap() {
        IntervalInfo::Discretes(mut discretes) => {
            discretes.sort_by(|a, b| {
                (a.0 as f32 / a.1 as f32).partial_cmp(&(b.0 as f32 / b.1 as f32)).unwrap()
            });
            discretes.first().unwrap().clone()
        },
        IntervalInfo::Stepwise{ min, .. } => min,
    };

    let config = Config {
        interval: interval,
        resolution: resolution,
        format: &format,
        .. Default::default()
    };

    camera.start(&config).unwrap();
    let refresh = ((interval.0 as f32 / interval.1 as f32) * 1000. + 0.5) as u32;
    Ok((camera, refresh))
}
