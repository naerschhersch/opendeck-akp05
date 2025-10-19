use mirajazz::{
    device::DeviceQuery,
    types::{HidDeviceInfo, ImageFormat, ImageMirroring, ImageMode, ImageRotation},
};

// Must be unique between all the plugins, 2 characters long and match `DeviceNamespace` field in `manifest.json`
pub const DEVICE_NAMESPACE: &str = "n4";

// Layout similar to Elgato Stream Deck+ but with 2x5 instead of 2x4 keys:
// - 10 LCD keys (2 rows x 5 columns) - differs from Stream Deck+ which has 8 keys
// - 4 rotary encoders with push function
// - 4 touchscreen zones (110x14mm LCD touch strip, similar to Stream Deck+'s 800x100px)
//
// In OpenDeck, the layout is reported as 2x5 for the physical buttons
// The 4 touchscreen zones belong to the encoders (one zone per encoder)
pub const ROW_COUNT: usize = 2;
pub const COL_COUNT: usize = 5;
pub const KEY_COUNT: usize = 10; // Physical LCD buttons only
pub const ENCODER_COUNT: usize = 4;
pub const TOUCH_ZONES: usize = ENCODER_COUNT; // Each encoder has an associated touch zone

// OpenDeck device type reported via `openaction` during registration.
// Match official Stream Deck device type values so OpenDeck treats the device
// like an Elgato Stream Deck Plus (value 7 in the Stream Deck SDK).
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum DeviceType {
    /// Generic keypad-only device (Stream Deck classic)
    StreamDeck = 0,
    /// Stream Deck Plus with encoders and touchscreen strip
    StreamDeckPlus = 7,
}

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
pub const AJAZZ_VID: u16 = 0x0000;
pub const AKP05_PID: u16 = 0x0000;

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

    pub fn image_format(&self) -> ImageFormat {
        // Static in-file configuration for key image rendering
        // Adjust size and rotation here as needed for your device
        ImageFormat {
            mode: ImageMode::JPEG,
            size: (112, 112),
            rotation: ImageRotation::Rot180,
            mirror: ImageMirroring::None,
        }
    }

    /// Image format for encoder touch strip zones
    pub fn touch_image_format(&self) -> ImageFormat {
        ImageFormat {
            mode: ImageMode::JPEG,
            size: (200, 100),
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

impl CandidateDevice {
    // Derive OpenDeck device type from capabilities
    pub fn device_type(&self) -> DeviceType {
        if ENCODER_COUNT > 0 && TOUCH_ZONES > 0 {
            DeviceType::StreamDeckPlus
        } else {
            DeviceType::StreamDeck
        }
    }
}
