#[derive(Debug, Clone, Copy, PartialEq, defmt::Format)]
pub enum FirmwareType {
    LD,  // ?
    AP2, // Primary Firmware
    AP1, // Backup Firmware
}

#[derive(Debug, Clone, Copy, PartialEq, defmt::Format)]
pub enum ModuleSetCommand {
    Reset,
    ShowLogo,
    UpdateBootLogo { slice: u8 },
    SetBrightness { value: u8 },
    SetIdleTime { seconds: i32 },
    SetKeyColor { key_index: u8, r: u8, g: u8, b: u8 }, // Module 15/32 only
    ShowBackgroundByIndex { index: u8 },                // Module 15/32 only
}

#[derive(Debug, Clone, Copy, PartialEq, defmt::Format)]
pub enum ModuleGetCommand {
    GetFirmwareVersion(FirmwareType),
    GetUnitSerialNumber,
    GetIdleTime,
    GetUnitInformation, // Module 15/32 only
}
