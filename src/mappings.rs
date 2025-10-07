use mirajazz::{
    device::DeviceQuery,
    types::{HidDeviceInfo, ImageFormat, ImageMirroring, ImageMode, ImageRotation},
};

// Must be unique between all the plugins, 2 characters long and match `DeviceNamespace` field in `manifest.json`
pub const DEVICE_NAMESPACE: &str = "n5";

// Layout similar to Elgato Stream Deck+ but with 2x5 instead of 2x4 keys:
// - 10 LCD keys (2 rows x 5 columns) - differs from Stream Deck+ which has 8 keys
// - 4 rotary encoders with push function
// - 4 touchscreen zones (110x14mm LCD touch strip, similar to Stream Deck+'s 800x100px)
//
// In OpenDeck, the layout is reported as 2x5 for the physical buttons
// The 4 touchscreen zones are treated as virtual buttons in mirajazz (see state.rs: "Buttons include Touch Points")
pub const ROW_COUNT: usize = 2;
pub const COL_COUNT: usize = 5;
pub const KEY_COUNT: usize = 10;      // Physical LCD buttons only
pub const TOUCH_COUNT: usize = 4;     // Touchscreen zones (treated as additional buttons internally)
pub const ENCODER_COUNT: usize = 4;

#[derive(Debug, Clone)]
pub enum Kind {
    Akp05,
    N5,
}

// TODO: Replace XXXX with actual VID/PID when device is available
pub const AJAZZ_VID: u16 = 0xXXXX;
pub const MIRABOX_VID: u16 = 0xYYYY;

pub const AKP05_PID: u16 = 0xXXXX;
pub const N5_PID: u16 = 0xYYYY;

// Map all queries to usage page 65440 and usage id 1 for now
// TODO: Verify usage page and usage id for AKP05/N5
pub const AKP05_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP05_PID);
pub const N5_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MIRABOX_VID, N5_PID);

pub const QUERIES: [DeviceQuery; 2] = [
    AKP05_QUERY,
    N5_QUERY,
];

impl Kind {
    /// Matches devices VID+PID pairs to correct kinds
    pub fn from_vid_pid(vid: u16, pid: u16) -> Option<Self> {
        match vid {
            AJAZZ_VID => match pid {
                AKP05_PID => Some(Kind::Akp05),
                _ => None,
            },

            MIRABOX_VID => match pid {
                N5_PID => Some(Kind::N5),
                _ => None,
            },

            _ => None,
        }
    }

    /// There is no point relying on manufacturer/device names reported by the USB stack,
    /// so we return custom names for all the kinds of devices
    pub fn human_name(&self) -> String {
        match &self {
            Self::Akp05 => "Ajazz AKP05",
            Self::N5 => "Mirabox N5",
        }
        .to_string()
    }

    /// Returns protocol version for device
    /// TODO: Verify correct protocol version for AKP05/N5 (likely 3 for newer devices)
    pub fn protocol_version(&self) -> usize {
        match self {
            Self::Akp05 => 3,  // TODO: Verify this
            Self::N5 => 3,     // TODO: Verify this
        }
    }

    pub fn image_format(&self) -> ImageFormat {
        if self.protocol_version() == 3 {
            return ImageFormat {
                mode: ImageMode::JPEG,
                size: (60, 60),
                rotation: ImageRotation::Rot90,
                mirror: ImageMirroring::None,
            };
        }

        return ImageFormat {
            mode: ImageMode::JPEG,
            size: (60, 60),
            rotation: ImageRotation::Rot0,
            mirror: ImageMirroring::None,
        };
    }
}

#[derive(Debug, Clone)]
pub struct CandidateDevice {
    pub id: String,
    pub dev: HidDeviceInfo,
    pub kind: Kind,
}
