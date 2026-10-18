use std::f64::consts::PI;

const EARTH_RADIUS_KM: f64 = 6371.0;
const FIBER_SPEED_KM_MS: f64 = 200.0; // ~200 km per ms in optical fiber

/// Calculate great-circle distance between two geographic coordinates using the Haversine formula
pub fn haversine_distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();

    let a = (d_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (d_lon / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().asin();

    EARTH_RADIUS_KM * c
}

/// Estimate round-trip time (RTT in ms) based on geographic distance in optical fiber plus routing overhead
pub fn estimate_fiber_rtt_ms(distance_km: f64) -> f64 {
    // 2x for round-trip, divided by fiber propagation speed + 2ms PoP switching/queuing floor
    (2.0 * distance_km / FIBER_SPEED_KM_MS) + 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_haversine_sfo_to_iad() {
        // SFO (37.6213, -122.3790) to IAD (38.9531, -77.4565) ~ 3900 km
        let dist = haversine_distance_km(37.6213, -122.3790, 38.9531, -77.4565);
        assert!((dist - 3900.0).abs() < 100.0);
    }

    #[test]
    fn test_haversine_identical_coords() {
        let dist = haversine_distance_km(51.5, -0.1, 51.5, -0.1);
        assert!(dist < 0.001);
    }
}
