extern crate rscam;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

pub use self::rscam::{Camera, Config, FormatInfo, FormatIter, ResolutionInfo, IntervalInfo};
pub use self::rscam::Result as V4l2Result;
pub use self::rscam::consts;

pub enum DisStepInfo {
    Discretes(Vec<(u32, u32)>),
    Stepwise {
        min: (u32, u32),
        max: (u32, u32),
        step: (u32, u32),
    },
}

#[derive(Clone)]
pub enum Dir {
    Highest,
    Lowest,
}

pub enum Pref {
    Only,
    Never,
    Prefer,
    DoNotPrefer,
    NoPreference,
}

pub struct Fmt {
    pub emulate: Pref,
    pub compress: Pref,
    pub priorities: Option<Vec<&'static [u8; 4]>>,
}

impl Default for Fmt {
    fn default() -> Self {
        Fmt {
            emulate: Pref::NoPreference,
            compress: Pref::NoPreference,
            priorities: None,
        }
    }
}

#[derive(Clone)]
struct DisStepConstraint {
    pub dir: Dir,
    pub limit: Option<(u32, u32)>,
}

pub type Res = DisStepConstraint;
pub type Speed = DisStepConstraint;

pub struct Constraints {
    pub formats: Option<Fmt>,
    pub resolutions: Option<Res>,
    pub speeds: Option<Speed>,
    pub field: u32,
    pub nbuffers: u32,
}

impl Default for Constraints {
    fn default() -> Self {
        Constraints {
            formats: None,
            resolutions: None,
            speeds: None,
            field: consts::FIELD_NONE,
            nbuffers: 2,
        }
    }
}

#[derive(Clone)]
pub struct ConfigSummary {
    pub interval: (u32, u32),
    pub resolution: (u32, u32),
    pub format: [u8; 4],
    pub field: u32,
    pub nbuffers: u32,
}

impl Debug for ConfigSummary {
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        try!(fmt.write_str("{\n"));
        try!(fmt.write_fmt(format_args!(
            "    interval: {}/{},\n", self.interval.0, self.interval.1)));
        try!(fmt.write_fmt(format_args!(
            "    resolution: {}x{},\n", self.resolution.0, self.resolution.1)));
        try!(fmt.write_fmt(format_args!(
            "    picture_format: {},\n", String::from_utf8_lossy(&self.format))));
        try!(fmt.write_fmt(format_args!(
            "    field: {}\n", self.field)));
        try!(fmt.write_fmt(format_args!(
            "    nbuffers: {}\n", self.nbuffers)));
        fmt.write_str("}")
    }
}

struct FormatPicker {
    formats: Vec<FormatInfo>,
    constraints: Option<Fmt>,
    sorted: bool,
}

impl FormatPicker {
    pub fn new(formats: FormatIter, constraints: Option<Fmt>) -> Self {
        FormatPicker {
            constraints: constraints,
            formats: formats.filter_map(Result::ok).collect(),
            sorted: false,
        }
    }

    fn sort(constraints: &Fmt, formats: &mut Vec<FormatInfo>) {
        // This has to only be done once, it can be slow
        // for the sake of clarity
        // Filter if user cares about emulated formats
        let filter_emulate = match constraints.emulate {
            Pref::Only => Some(true),
            Pref::Never => Some(false),
            _ => None,
        };
        if let Some(emulate) = filter_emulate {
            formats.retain(|f| {
                f.emulated && emulate || !f.emulated && !emulate
            });
        }
        // Filter is user cares about compression
        let filter_compress = match constraints.compress {
            Pref::Only => Some(true),
            Pref::Never => Some(false),
            _ => None,
        };
        if let Some(compress) = filter_compress {
            formats.retain(|f| {
                f.compressed && compress || !f.compressed && !compress
            });
        }
        // Create a map of formats to their priorities
        let wanted: Option<HashMap<&[u8; 4], usize>> = constraints.priorities.as_ref().map(|vec| {
            vec.into_iter()
                .enumerate()
                .fold(HashMap::new(), |mut map, (index, format)| {
                    map.insert(*format, index);
                    map
                })
        });
        // Remove all formats not in the priorities list
        if let Some(ref priorities) = wanted {
            formats.retain(|f| {
                priorities.get(&f.format).is_some()
            });
        }
        // Sort formats based on preferences and priorities
        formats.sort_by(|a, b| {
            // Sort first by priority
            if let Some(ref priorities) = wanted {
                let a_priority = priorities.get(&a.format);
                let b_priority = priorities.get(&b.format);
                if let (Some(ap), Some(bp)) = (a_priority, b_priority) {
                    if ap < bp {
                        return Ordering::Greater;
                    } else if ap > bp {
                        return Ordering::Less;
                    }
                }
            }
            // Next sort by emulation preferences
            let decision = match (a.emulated, b.emulated, &constraints.emulate) {
                (true, false, &Pref::Prefer) => Some(Ordering::Greater),
                (false, true, &Pref::Prefer) => Some(Ordering::Less),
                (true, false, &Pref::DoNotPrefer) => Some(Ordering::Less),
                (false, true, &Pref::DoNotPrefer) => Some(Ordering::Greater),
                _ => None,
            };
            if let Some(concrete) = decision {
                return concrete;
            }
            // Finally sort by compression preferences
            let decision = match (a.compressed, b.compressed, &constraints.compress) {
                (true, false, &Pref::Prefer) => Some(Ordering::Greater),
                (false, true, &Pref::Prefer) => Some(Ordering::Less),
                (true, false, &Pref::DoNotPrefer) => Some(Ordering::Less),
                (false, true, &Pref::DoNotPrefer) => Some(Ordering::Greater),
                _ => None,
            };
            if let Some(concrete) = decision {
                return concrete;
            }
            // We cannot decide! They are equal
            Ordering::Equal
        });
    }
}

impl Iterator for FormatPicker {
    type Item = FormatInfo;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref constraints) = self.constraints {
            if !self.sorted {
                FormatPicker::sort(constraints, &mut self.formats);
            }
        }
        self.formats.pop()
    }
}

impl Into<DisStepInfo> for ResolutionInfo {
    fn into(self) -> DisStepInfo {
        match self {
            ResolutionInfo::Discretes(d) => DisStepInfo::Discretes(d),
            ResolutionInfo::Stepwise{min, max, step} => {
                DisStepInfo::Stepwise {
                    min: min,
                    max: max,
                    step: step
                }
            },
        }
    }
}

impl Into<DisStepInfo> for IntervalInfo {
    fn into(self) -> DisStepInfo {
        match self {
            IntervalInfo::Discretes(d) => DisStepInfo::Discretes(d),
            IntervalInfo::Stepwise{min, max, step} => {
                DisStepInfo::Stepwise {
                    min: min,
                    max: max,
                    step: step
                }
            },
        }
    }
}

pub struct DisStepPicker {
    info: DisStepInfo,
    constraints: Option<DisStepConstraint>,
    sorted: bool,
}

impl DisStepPicker {
    fn new<I>(info: I, constraints: Option<DisStepConstraint>) -> Self
    where I: Into<DisStepInfo> {
        DisStepPicker {
            info: info.into(),
            constraints: constraints,
            sorted: false,
        }
    }

    fn sort_discretes(discretes: &mut Vec<(u32, u32)>, constraints: &DisStepConstraint) {
        let want_highest = match constraints.dir {
            Dir::Highest => true,
            Dir::Lowest => false,
        };
        if let Some(ref limit) = constraints.limit {
            discretes.retain(|r| {
                (want_highest && r <= limit) || (!want_highest && r >= limit)
            });
        }
        if want_highest {
            discretes.sort();
        } else {
            // Sort in reverse
            discretes.sort_by(|a, b| b.cmp(a));
        }
    }
}

impl Iterator for DisStepPicker {
    type Item = (u32, u32);
    fn next(&mut self) -> Option<Self::Item> {
        match self.info {
            DisStepInfo::Discretes(ref mut discretes) => {
                if let Some(ref constraints) = self.constraints {
                    if !self.sorted {
                        DisStepPicker::sort_discretes(discretes, constraints);
                    }
                }
                discretes.pop()
            },
            DisStepInfo::Stepwise{ .. } => {
                unimplemented!();
            },
        }
    }
}

fn get_config(camera: &Camera, reqs: Constraints) -> Option<ConfigSummary> {
    let formats = FormatPicker::new(camera.formats(), reqs.formats);
    for format in formats {
        if let Ok(resolutions) = camera.resolutions(&format.format) {
            let resolutions = DisStepPicker::new(resolutions, reqs.resolutions.clone());
            for resolution in resolutions {
                if let Ok(intervals) = camera.intervals(&format.format, resolution) {
                    let intervals = DisStepPicker::new(intervals, reqs.speeds.clone());
                    for interval in intervals {
                        return Some(ConfigSummary {
                            interval: interval,
                            resolution: resolution,
                            format: format.format,
                            field: reqs.field,
                            nbuffers: reqs.nbuffers,
                        });
                    }
                }
            }
        }
    }
    None
}

pub fn camera(path: &str, reqs: Constraints) -> V4l2Result<(Camera, Option<ConfigSummary>)> {
    let mut camera = try!(Camera::new(path));
    let possible_config = get_config(&camera, reqs);
    if let Some(config) = possible_config.clone() {
        try!(camera.start(&Config {
            interval: config.interval,
            resolution: config.resolution,
            format: &config.format[..],
            field: config.field,
            nbuffers: config.nbuffers,
        }));
    }
    Ok((camera, possible_config))
}

#[test]
fn optomize_framerate() {
    let constraints = Constraints {
        formats: Some(Fmt {
            emulate: Pref::DoNotPrefer,
            compress: Pref::Prefer,
            priorities: Some(vec![b"MJPG"]),
        }),
        resolutions: Some(Res {
            dir: Dir::Lowest,
            limit: Some((640, 480)),
        }),
        speeds: Some(Speed {
            dir: Dir::Highest,
            limit: None,
        }),
        .. Default::default()
    };

    if let Ok((_, possible_config)) = camera("/dev/video1", constraints) {
        println!("Camera started successfuly");
        let config = possible_config.expect("Could not find a configuration!");
        println!("Camera Configuration for Framerate: {:?}", &config);
    } else {
        panic!("Could not get camera!");
    }
}

#[test]
fn optomize_quality() {
    let constraints = Constraints {
        formats: Some(Fmt {
            emulate: Pref::NoPreference,
            compress: Pref::DoNotPrefer,
            priorities: Some(vec![b"MJPG"]),
        }),
        resolutions: Some(Res {
            dir: Dir::Highest,
            limit: None,
        }),
        speeds: None,
        .. Default::default()
    };

    if let Ok((_, possible_config)) = camera("/dev/video1", constraints) {
        println!("Camera started successfuly");
        let config = possible_config.expect("Could not find a configuration!");
        println!("Camera Configuration for Quality: {:?}", &config);
    } else {
        panic!("Could not get camera!");
    }
}
