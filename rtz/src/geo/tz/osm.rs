//! The [OpenStreetMap](https://www.openstreetmap.org/) timezone lookup module.

use std::{collections::HashMap, sync::OnceLock};

use rtz_core::geo::{
    shared::{ConcreteVec, RoundLngLat},
    tz::{
        osm::OsmTimezone,
        shared::{i16_vec_to_timezoneids, TimezoneIds},
    },
};

use crate::geo::shared::{HasItemData, HasLookupData};

// Trait impls.

impl HasItemData for OsmTimezone {
    fn get_mem_items() -> &'static ConcreteVec<OsmTimezone> {
        static TIMEZONES: OnceLock<ConcreteVec<OsmTimezone>> = OnceLock::new();

        #[cfg(feature = "self-contained")]
        {
            TIMEZONES.get_or_init(|| {
                let (timezones, _len): (ConcreteVec<OsmTimezone>, usize) = bincode::serde::decode_from_slice(TZ_BINCODE, bincode::config::standard()).expect("Failed to decode timezones from binary: likely caused by precision difference between generated assets and current build.  Please rebuild the assets with `feature = \"force-rebuild\"` enabled.");

                timezones
            })
        }

        #[cfg(not(feature = "self-contained"))]
        {
            use rtz_core::geo::{shared::get_items_from_features, tz::osm::get_geojson_features_from_source};

            TIMEZONES.get_or_init(|| {
                let features = get_geojson_features_from_source();

                get_items_from_features(features)
            })
        }
    }
}

impl HasLookupData for OsmTimezone {
    type Lookup = TimezoneIds;

    fn get_mem_lookup() -> &'static HashMap<RoundLngLat, Self::Lookup> {
        static CACHE: OnceLock<HashMap<RoundLngLat, TimezoneIds>> = OnceLock::new();

        #[cfg(feature = "self-contained")]
        {
            use rtz_core::geo::shared::RoundInt;

            CACHE.get_or_init(|| {
                let (cache, _len): (HashMap<RoundLngLat, Vec<RoundInt>>, usize) = bincode::serde::decode_from_slice(LOOKUP_BINCODE, bincode::config::standard()).unwrap();

                cache
                    .into_iter()
                    .map(|(key, value)| {
                        let value = i16_vec_to_timezoneids(value);

                        (key, value)
                    })
                    .collect::<HashMap<_, _>>()
            })
        }

        #[cfg(not(feature = "self-contained"))]
        {
            use rtz_core::geo::shared::get_lookup_from_geometries;

            CACHE.get_or_init(|| {
                let cache = get_lookup_from_geometries(OsmTimezone::get_items());

                cache
                    .into_iter()
                    .map(|(key, value)| {
                        let value = i16_vec_to_timezoneids(value);

                        (key, value)
                    })
                    .collect::<HashMap<_, _>>()
            })
        }
    }
}

// Statics.

#[cfg(all(host_family_unix, feature = "self-contained"))]
static TZ_BINCODE: &[u8] = include_bytes!("../../../../assets/osm_time_zones.bincode");
#[cfg(all(host_family_windows, feature = "self-contained"))]
static TZ_BINCODE: &[u8] = include_bytes!("..\\..\\..\\..\\assets\\osm_time_zones.bincode");

#[cfg(all(host_family_unix, feature = "self-contained"))]
static LOOKUP_BINCODE: &[u8] = include_bytes!("../../../../assets/osm_time_zone_lookup.bincode");
#[cfg(all(host_family_windows, feature = "self-contained"))]
static LOOKUP_BINCODE: &[u8] = include_bytes!("..\\..\\..\\..\\assets\\osm_time_zone_lookup.bincode");

// Tests.

#[cfg(test)]
mod tests {
    use crate::geo::shared::{CanPerformGeoLookup, HasItemData, MapIntoItems};

    use super::*;
    use pretty_assertions::assert_eq;
    use rayon::prelude::{IntoParallelIterator, ParallelIterator};
    use rtz_core::base::types::Float;

    #[test]
    fn can_get_timezones() {
        let timezones = OsmTimezone::get_mem_items();
        assert_eq!(timezones.len(), 444);
    }

    #[test]
    fn can_get_lookup() {
        let cache = OsmTimezone::get_mem_lookup();
        assert_eq!(cache.len(), 64_800);
    }

    #[test]
    fn can_get_from_lookup() {
        let cache = OsmTimezone::get_lookup_suggestions(-121, 46).unwrap();
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn can_perform_exact_lookup() {
        assert_eq!(OsmTimezone::lookup_slow(-177.0, -15.0).len(), 1);
        assert_eq!(OsmTimezone::lookup_slow(-121.0, 46.0)[0].identifier, "America/Los_Angeles");

        assert_eq!(OsmTimezone::lookup_slow(179.9968, -67.0959).len(), 1);
    }

    #[test]
    fn can_access_lookup() {
        let cache = OsmTimezone::get_mem_lookup();

        let tzs = cache.get(&(-177, -15)).map_into_items().unwrap() as Vec<&OsmTimezone>;
        assert_eq!(tzs.len(), 1);

        let tzs = cache.get(&(-121, 46)).map_into_items().unwrap() as Vec<&OsmTimezone>;
        assert_eq!(tzs.len(), 1);

        let tz = cache.get(&(-121, 46)).map_into_items().unwrap()[0] as &OsmTimezone;
        assert_eq!(tz.identifier, "America/Los_Angeles");

        let tzs = cache.get(&(-87, 38)).map_into_items().unwrap() as Vec<&OsmTimezone>;
        assert_eq!(tzs.len(), 7);
    }

    #[test]
    fn can_verify_lookup_assisted_accuracy() {
        (0..100).into_par_iter().for_each(|_| {
            let x = rand::random::<Float>() * 360.0 - 180.0;
            let y = rand::random::<Float>() * 180.0 - 90.0;
            let full = OsmTimezone::lookup_slow(x, y);
            let lookup_assisted = OsmTimezone::lookup(x, y);

            assert_eq!(
                full.into_iter().map(|t| t.id).collect::<Vec<_>>(),
                lookup_assisted.into_iter().map(|t| t.id).collect::<Vec<_>>(),
                "({}, {})",
                x,
                y
            );
        });
    }
}

#[cfg(test)]
mod bench {
    extern crate test;

    use rtz_core::base::types::Float;
    use test::{black_box, Bencher};

    use crate::geo::shared::CanPerformGeoLookup;

    use super::*;

    #[bench]
    #[ignore]
    fn bench_full_lookup_sweep(b: &mut Bencher) {
        let xs = (-179..180).step_by(10);
        let ys = (-89..90).step_by(10);

        b.iter(|| {
            for x in xs.clone() {
                for y in ys.clone() {
                    black_box(OsmTimezone::lookup_slow(x as Float, y as Float));
                }
            }
        });
    }

    #[bench]
    fn bench_lookup_assisted_sweep(b: &mut Bencher) {
        let xs = (-179..180).step_by(10);
        let ys = (-89..90).step_by(10);

        b.iter(|| {
            for x in xs.clone() {
                for y in ys.clone() {
                    black_box(OsmTimezone::lookup(x as Float, y as Float));
                }
            }
        });
    }

    #[bench]
    fn bench_worst_case_full_lookup_single(b: &mut Bencher) {
        let x = -86.5;
        let y = 38.5;

        b.iter(|| {
            black_box(OsmTimezone::lookup_slow(x as Float, y as Float));
        });
    }

    #[bench]
    fn bench_worst_case_lookup_assisted_single(b: &mut Bencher) {
        let x = -86.5;
        let y = 38.5;

        b.iter(|| {
            black_box(OsmTimezone::lookup(x, y));
        });
    }
}
