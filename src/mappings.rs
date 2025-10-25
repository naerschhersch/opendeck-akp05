use mirajazz::{
    device::DeviceQuery,
    types::{HidDeviceInfo, ImageFormat, ImageMirroring, ImageMode, ImageRotation},
};

// Must be unique between all the plugins, 2 characters long and match `DeviceNamespace` field in `manifest.json`
pub const DEVICE_NAMESPACE: &str = "n4";

// Mirabox N4 layout (verified with hardware):
// - 10 regular LCD buttons: 2 rows x 5 columns
// - 4 wide LCD buttons for encoder touch zones
// - 4 rotary encoders with push function
// - Layout in OpenDeck: 2 rows x 5 columns + 4 encoders with touch zones
//
// Hardware button indices:
// [0] [1] [2] [3]              <- 4 wide touch zone buttons (one per encoder)
// Unused: indices 4
// [5] [6] [7] [8] [9]          <- Bottom row (5 regular buttons)
// [10] [11] [12] [13] [14]     <- Top row (5 regular buttons)
//
// OpenDeck mapping:
// Encoder 0-3 → Touch buttons 0-3
// Grid 0-4 (top row) → Hardware buttons 10-14
// Grid 5-9 (bottom row) → Hardware buttons 5-9
pub const ROW_COUNT: usize = 2;
pub const COL_COUNT: usize = 5;
pub const KEY_COUNT: usize = 15; // Hardware uses indices 0-14 (4 touch buttons + 10 regular buttons)
pub const ENCODER_COUNT: usize = 4;

// OpenDeck device type: 7 = StreamDeckPlus (with encoders and touch zones)
// This enables automatic encoder function rendering on the 4 wide touch zone buttons
pub const DEVICE_TYPE: u8 = 7;

#[derive(Debug, Clone)]
pub enum Kind {
    Akp05,
    N4,
}

// Mirabox N4: VID and PID confirmed with actual hardware
pub const MIRABOX_VID: u16 = 0x6603;
pub const N4_PID: u16 = 0x1007;

// Ajazz AKP05: VID/PID not yet known - hardware not available
// Placeholder values set to 0 so build succeeds; update with real USB IDs when available
pub const AJAZZ_VID: u16 = 0x0300;
pub const AKP05_PID: u16 = 0x3004;

// Usage page and usage id need verification with actual hardware testing
// TODO: Verify usage page (65440) and usage id (1) are correct for N4 and AKP05
pub const AKP05_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP05_PID);
pub const N4_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MIRABOX_VID, N4_PID);

pub const QUERIES: [DeviceQuery; 2] = [AKP05_QUERY, N4_QUERY];

impl Kind {
    /// Matches devices VID+PID pairs to correct kinds
    pub fn from_vid_pid(vid: u16, pid: u16) -> Option<Self> {
        match vid {
            AJAZZ_VID => match pid {
                AKP05_PID => Some(Kind::Akp05),
                _ => None,
            },

            MIRABOX_VID => match pid {
                N4_PID => Some(Kind::N4),
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
            Self::N4 => "Mirabox N4",
        }
        .to_string()
    }

    /// Returns protocol version for device
    pub fn protocol_version(&self) -> usize {
        match self {
            Self::Akp05 => 3, // TODO: Verify this with actual AKP05 hardware
            Self::N4 => 3,    // TODO: Verify this with N4 hardware testing
        }
    }

    /// Image format for regular LCD buttons (2x5 grid, positions 0-9)
    pub fn image_format(&self) -> ImageFormat {
        ImageFormat {
            mode: ImageMode::JPEG,
            size: (112, 112),
            rotation: ImageRotation::Rot180,
            mirror: ImageMirroring::None,
        }
    }

    /// Image format for wide touch zone buttons (4 buttons, hardware indices 0-3)
    /// These are discrete LCD buttons used to display encoder functions
    /// Testing wider dimension to reach the top
    pub fn image_format_touchzone(&self) -> ImageFormat {
        ImageFormat {
            mode: ImageMode::JPEG,
            size: (184, 120),
            rotation: ImageRotation::Rot180,
            mirror: ImageMirroring::None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CandidateDevice {
    pub id: String,
    pub dev: HidDeviceInfo,
    pub kind: Kind,
}
