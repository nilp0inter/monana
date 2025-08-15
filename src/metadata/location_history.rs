// Location History Module
// This module is responsible for parsing and querying Google Maps Timeline Location History data.

use std::cmp::Ordering;

/// Represents a single point in time and space from Google Location History.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationPoint {
    pub timestamp_ms: u64,
    pub latitude_e7: i32,
    pub longitude_e7: i32,
}

impl Ord for LocationPoint {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp_ms.cmp(&other.timestamp_ms)
    }
}

impl PartialOrd for LocationPoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Holds a sorted collection of `LocationPoint` instances for efficient querying.
#[derive(Debug, Default)]
pub struct LocationHistory {
    /// A collection of location points, guaranteed to be sorted by `timestamp_ms`.
    data: Vec<LocationPoint>,
}

// Private structs for deserializing the JSON file.
use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;

// Deserializes a string timestamp into a u64.
fn parse_str_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    s.parse::<u64>().map_err(serde::de::Error::custom)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakeoutRoot {
    locations: Vec<TakeoutLocation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakeoutLocation {
    #[serde(deserialize_with = "parse_str_to_u64")]
    timestamp_ms: u64,
    latitude_e7: i32,
    longitude_e7: i32,
    activity: Option<Vec<TakeoutActivity>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakeoutActivity {
    #[serde(deserialize_with = "parse_str_to_u64")]
    timestamp_ms: u64,
    // The nested 'activity' array with type/confidence is ignored by serde
}

impl LocationHistory {
    /// Loads location history from a Google Takeout JSON file.
    pub fn from_json_file(path: &str) -> Result<Self, Box<dyn Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let root: TakeoutRoot = serde_json::from_reader(reader)?;

        let mut points = Vec::new();

        for loc in root.locations {
            // Add the main location point
            points.push(LocationPoint {
                timestamp_ms: loc.timestamp_ms,
                latitude_e7: loc.latitude_e7,
                longitude_e7: loc.longitude_e7,
            });

            // Add points from activities, if any
            if let Some(activities) = loc.activity {
                for activity in activities {
                    points.push(LocationPoint {
                        timestamp_ms: activity.timestamp_ms,
                        latitude_e7: loc.latitude_e7,
                        longitude_e7: loc.longitude_e7,
                    });
                }
            }
        }

        // The spec guarantees the top-level locations are sorted, but activities can be out of order.
        // We need to sort the entire collection of points.
        points.sort_unstable();

        Ok(LocationHistory { data: points })
    }

    /// Finds the two closest location points for a given timestamp.
    pub fn find_closest_points(
        &self,
        target_timestamp_ms: u64,
    ) -> (Option<&LocationPoint>, Option<&LocationPoint>) {
        if self.data.is_empty() {
            return (None, None);
        }

        match self
            .data
            .binary_search_by_key(&target_timestamp_ms, |p| p.timestamp_ms)
        {
            Ok(index) => {
                // Exact match found. This point is both <= and >= the target.
                let point = self.data.get(index);
                (point, point)
            }
            Err(index) => {
                // No exact match. `index` is the insertion point.
                // The point before is at `index - 1`.
                // The point after is at `index`.
                let before = if index > 0 {
                    self.data.get(index - 1)
                } else {
                    None
                };
                let after = self.data.get(index);
                (before, after)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_json_file_ok() {
        let history = LocationHistory::from_json_file("test_data/location_history.json").unwrap();

        // Should have 3 locations + 2 activities = 5 points
        assert_eq!(history.data.len(), 5);

        // Check if the points are sorted by timestamp
        let timestamps: Vec<u64> = history.data.iter().map(|p| p.timestamp_ms).collect();
        assert_eq!(timestamps, vec![10000, 20000, 21000, 22000, 30000]);

        // Check a point from an activity
        let activity_point = history
            .data
            .iter()
            .find(|p| p.timestamp_ms == 21000)
            .unwrap();
        // It should have the coordinates of its parent location (ts=20000)
        assert_eq!(activity_point.latitude_e7, 20000000);
        assert_eq!(activity_point.longitude_e7, 20000000);

        // Check a top-level point
        let top_level_point = history
            .data
            .iter()
            .find(|p| p.timestamp_ms == 30000)
            .unwrap();
        assert_eq!(top_level_point.latitude_e7, 30000000);
    }

    #[test]
    fn test_from_json_file_not_found() {
        let result = LocationHistory::from_json_file("test_data/non_existent_file.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_json_file_malformed() {
        let result = LocationHistory::from_json_file("test_data/malformed.json");
        assert!(result.is_err());
    }

    fn create_test_history() -> LocationHistory {
        LocationHistory {
            data: vec![
                LocationPoint {
                    timestamp_ms: 100,
                    latitude_e7: 1,
                    longitude_e7: 1,
                },
                LocationPoint {
                    timestamp_ms: 200,
                    latitude_e7: 2,
                    longitude_e7: 2,
                },
                LocationPoint {
                    timestamp_ms: 300,
                    latitude_e7: 3,
                    longitude_e7: 3,
                },
                LocationPoint {
                    timestamp_ms: 400,
                    latitude_e7: 4,
                    longitude_e7: 4,
                },
            ],
        }
    }

    #[test]
    fn test_find_closest_points_between() {
        let history = create_test_history();
        let (before, after) = history.find_closest_points(250);
        assert_eq!(before.unwrap().timestamp_ms, 200);
        assert_eq!(after.unwrap().timestamp_ms, 300);
    }

    #[test]
    fn test_find_closest_points_exact() {
        let history = create_test_history();
        let (before, after) = history.find_closest_points(300);
        assert_eq!(before.unwrap().timestamp_ms, 300);
        assert_eq!(after.unwrap().timestamp_ms, 300);
    }

    #[test]
    fn test_find_closest_points_before_all() {
        let history = create_test_history();
        let (before, after) = history.find_closest_points(50);
        assert!(before.is_none());
        assert_eq!(after.unwrap().timestamp_ms, 100);
    }

    #[test]
    fn test_find_closest_points_after_all() {
        let history = create_test_history();
        let (before, after) = history.find_closest_points(450);
        assert_eq!(before.unwrap().timestamp_ms, 400);
        assert!(after.is_none());
    }

    #[test]
    fn test_find_closest_points_empty() {
        let history = LocationHistory { data: vec![] };
        let (before, after) = history.find_closest_points(100);
        assert!(before.is_none());
        assert!(after.is_none());
    }

    #[test]
    fn test_find_closest_points_single_entry() {
        let history = LocationHistory {
            data: vec![LocationPoint {
                timestamp_ms: 100,
                latitude_e7: 1,
                longitude_e7: 1,
            }],
        };
        // Before
        let (before, after) = history.find_closest_points(50);
        assert!(before.is_none());
        assert_eq!(after.unwrap().timestamp_ms, 100);
        // After
        let (before, after) = history.find_closest_points(150);
        assert_eq!(before.unwrap().timestamp_ms, 100);
        assert!(after.is_none());
        // Exact
        let (before, after) = history.find_closest_points(100);
        assert_eq!(before.unwrap().timestamp_ms, 100);
        assert_eq!(after.unwrap().timestamp_ms, 100);
    }
}
