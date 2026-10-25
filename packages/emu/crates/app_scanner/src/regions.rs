const REGIONS: [&str; 7] = [
    "Japan",
    "North America",
    "Europe",
    "Australia",
    "China",
    "Korea",
    "Taiwan",
];

pub const REGION_FREE: &str = "Region free";
pub const INVALID_REGION: &str = "Invalid region";

pub fn regions_string(region_lockout: u32) -> String {
    let regions: Vec<&str> = REGIONS
        .iter()
        .enumerate()
        .filter(|(index, _)| region_lockout & (1 << index) != 0)
        .map(|(_, name)| *name)
        .collect();
    match regions.len() {
        0 => INVALID_REGION.to_owned(),
        count if count == REGIONS.len() => REGION_FREE.to_owned(),
        _ => regions.join(", "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_regions_in_bit_order() {
        assert_eq!(regions_string(0b0000_0110), "North America, Europe");
        assert_eq!(regions_string(0b0100_0001), "Japan, Taiwan");
    }

    #[test]
    fn distinguishes_free_and_invalid() {
        assert_eq!(regions_string(0x7F), REGION_FREE);
        assert_eq!(regions_string(0), INVALID_REGION);
        assert_eq!(regions_string(0x80), INVALID_REGION);
        assert_eq!(regions_string(0xFF), REGION_FREE);
    }
}
