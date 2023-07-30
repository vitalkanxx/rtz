//! Shared functionality for geo operations in the `rtz` crate.

// Traits.

use geo::{Contains, Coord};
use rtz_core::{
    base::types::Float,
    geo::shared::{ConcreteVec, HasGeometry, HasProperties, RoundInt, RoundLngLat, ToGeoJson},
};
use std::collections::HashMap;

/// Trait that abstracts away getting the in-memory items.
pub trait HasItemData
where
    Self: Sized,
{
    /// Gets the items from the in-memory cache for the given type.
    fn get_mem_items() -> &'static ConcreteVec<Self>;
}

/// Trait that abstracts away getting the in-memory timezones / cache.
pub trait HasLookupData: HasItemData
where
    Self: Sized,
{
    /// The type to which the lookup hash table resolves.
    type Lookup: AsRef<[RoundInt]>;

    /// Gets the lookup hash table from the in-memory cache for the given type.
    fn get_mem_lookup() -> &'static HashMap<RoundLngLat, Self::Lookup>;
}

/// Trait that allows converting a [`u16`] into the item to which the id refers (from the global list).
pub(crate) trait MapIntoItem<T> {
    fn map_into_item(self) -> Option<&'static T>;
}

impl<T> MapIntoItem<T> for Option<&u16>
where
    T: HasItemData,
{
    fn map_into_item(self) -> Option<&'static T> {
        let Some(value) = self else {
            return None;
        };

        let timezones = T::get_mem_items();

        timezones.get(*value as usize)
    }
}

/// Trait that allows converting a [`u16`] into the items to which the ids refer (from the global list).
pub(crate) trait MapIntoItems<T> {
    fn map_into_items(self) -> Option<Vec<&'static T>>;
}

impl<A, T> MapIntoItems<T> for Option<A>
where
    A: AsRef<[RoundInt]>,
    T: HasItemData,
{
    fn map_into_items(self) -> Option<Vec<&'static T>> {
        let Some(value) = self else {
            return None;
        };

        let timezones = T::get_mem_items();

        let mut result = Vec::with_capacity(10);
        for id in value.as_ref() {
            if *id == -1 {
                continue;
            }

            let tz = timezones.get(*id as usize);

            if let Some(tz) = tz {
                result.push(tz);
            }
        }

        Some(result)
    }
}

/// Trait that abstracts away the primary end-user functionality of geo lookups.
pub trait CanPerformGeoLookup: HasLookupData + HasGeometry + HasProperties
where
    Self: 'static
{
    /// Get the cache-driven item for a given longitude (x) and latitude (y).
    ///
    /// Some data sources allow for multiple results, so this is a vector.
    fn lookup(xf: Float, yf: Float) -> Vec<&'static Self> {
        let x = xf.floor() as i16;
        let y = yf.floor() as i16;

        let Some(suggestions) = Self::get_lookup_suggestions(x, y) else {
            return Vec::new();
        };

        suggestions.into_iter().filter(|&i| i.geometry().contains(&Coord { x: xf, y: yf })).collect()
    }

    /// Get the exact item for a given longitude (x) and latitude (y).
    #[allow(dead_code)]
    fn lookup_slow(xf: Float, yf: Float) -> Vec<&'static Self> {
        Self::get_mem_items().into_iter().filter(|&i| i.geometry().contains(&Coord { x: xf, y: yf })).collect()
    }

    /// Gets the geojson representation of the memory cache.
    fn memory_data_to_geojson() -> String {
        let geojson = Self::get_mem_items().to_geojson();
        geojson.to_json_value().to_string()
    }

    /// Get value from the static memory cache.
    fn get_lookup_suggestions(x: i16, y: i16) -> Option<Vec<&'static Self>> {
        let cache = Self::get_mem_lookup();

        cache.get(&(x, y)).map_into_items()
    }
}