//! Monitor storage base helpers

/// Convert year, month, day to a compact DayId
pub fn to_day_id(year: &i32, month: &u32, day: &u32) -> Result<DayId, &'static str> {
    const MINIMAL_VALID_YEAR: i32 = 2000;
    let year_index: i32 = *year - MINIMAL_VALID_YEAR;

    if year_index < 0 {
        return Err("year less minimum");
    }

    let mut day_id: u32 = (year_index as u32) & 0x000000FF;
    day_id = (day_id << 4) | (*month & 0xF);
    day_id = (day_id << 8) | (*day & 0xFF);
    Ok(day_id)
}

/// Composite key of the day: 8 bits - year, 4 bits - month, 8 bits - day
pub type DayId = u32;
