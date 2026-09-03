pub mod bt;
pub mod usb;

pub use bt::Ds4BtParser;
pub use usb::Ds4UsbParser;

/// Sony DS4 USB vendor/product IDs (original + v2 revision).
pub const DS4_VENDOR_ID: u16 = 0x054c;
pub const DS4_PRODUCT_ID_V1: u16 = 0x05c4;
pub const DS4_PRODUCT_ID_V2: u16 = 0x09cc;
