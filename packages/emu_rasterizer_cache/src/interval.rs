use std::collections::BTreeMap;
use std::ops::Bound;

/// A right open interval of addresses, as `boost::icl::right_open_interval` holds it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Interval {
    pub lower: u32,
    pub upper: u32,
}

impl Interval {
    pub const fn new(lower: u32, upper: u32) -> Self {
        Self { lower, upper }
    }

    pub const fn first(&self) -> u32 {
        self.lower
    }

    pub const fn last_next(&self) -> u32 {
        self.upper
    }

    pub const fn is_empty(&self) -> bool {
        self.lower >= self.upper
    }

    pub const fn length(&self) -> u32 {
        if self.is_empty() {
            0
        } else {
            self.upper - self.lower
        }
    }

    /// The intersection of the intervals, which is empty when they do not overlap.
    pub fn intersect(&self, other: &Interval) -> Interval {
        Interval::new(self.lower.max(other.lower), self.upper.min(other.upper))
    }

    fn overlaps(&self, other: &Interval) -> bool {
        !self.intersect(other).is_empty()
    }
}

impl PartialEq for Interval {
    fn eq(&self, other: &Self) -> bool {
        (self.is_empty() && other.is_empty())
            || (self.lower == other.lower && self.upper == other.upper)
    }
}

impl Eq for Interval {}

/// A value that an interval map associates with its segments.
pub trait IntervalValue: Copy + PartialEq {
    /// Whether the value is the identity element, which the map does not store
    /// (`boost::icl::partial_absorber`).
    fn is_identity(&self) -> bool;
}

impl IntervalValue for () {
    fn is_identity(&self) -> bool {
        false
    }
}

impl IntervalValue for i32 {
    fn is_identity(&self) -> bool {
        *self == 0
    }
}

/// A map from disjoint intervals to values that joins neighbouring segments with equal values, as
/// `boost::icl::interval_map` with `partial_absorber` and `inplace_plus` does.
#[derive(Clone, Debug, Default)]
pub struct IntervalMap<V> {
    segments: BTreeMap<u32, (u32, V)>,
}

impl<V: IntervalValue> IntervalMap<V> {
    pub fn new() -> Self {
        Self {
            segments: BTreeMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn clear(&mut self) {
        self.segments.clear();
    }

    /// The segments in address order.
    pub fn iter(&self) -> impl Iterator<Item = (Interval, V)> + '_ {
        self.segments
            .iter()
            .map(|(&lower, &(upper, value))| (Interval::new(lower, upper), value))
    }

    /// The segments that overlap the interval, unclipped, as `equal_range` returns them.
    pub fn equal_range(&self, interval: Interval) -> Vec<(Interval, V)> {
        if interval.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        if let Some((&lower, &(upper, value))) = self
            .segments
            .range((Bound::Unbounded, Bound::Excluded(interval.lower)))
            .next_back()
            && upper > interval.lower
        {
            result.push((Interval::new(lower, upper), value));
        }
        for (&lower, &(upper, value)) in self.segments.range((
            Bound::Included(interval.lower),
            Bound::Excluded(interval.upper),
        )) {
            result.push((Interval::new(lower, upper), value));
        }
        result
    }

    /// The first segment that overlaps the interval, as `find` returns it.
    pub fn find(&self, interval: Interval) -> Option<(Interval, V)> {
        self.equal_range(interval).into_iter().next()
    }

    /// Splits the segment that contains the point, so that no segment crosses it.
    fn split_at(&mut self, point: u32) {
        let Some((&lower, &(upper, value))) = self
            .segments
            .range((Bound::Unbounded, Bound::Excluded(point)))
            .next_back()
        else {
            return;
        };
        if upper > point {
            self.segments.insert(lower, (point, value));
            self.segments.insert(point, (upper, value));
        }
    }

    /// Joins the neighbouring segments with equal values around the interval.
    fn join(&mut self, interval: Interval) {
        let start = self
            .segments
            .range((Bound::Unbounded, Bound::Excluded(interval.lower)))
            .next_back()
            .map_or(interval.lower, |(&lower, _)| lower);
        let keys: Vec<u32> = self
            .segments
            .range((Bound::Included(start), Bound::Included(interval.upper)))
            .map(|(&lower, _)| lower)
            .collect();
        let mut previous: Option<u32> = None;
        for key in keys {
            let (upper, value) = self.segments[&key];
            if let Some(previous_key) = previous {
                let (previous_upper, previous_value) = self.segments[&previous_key];
                if previous_upper == key && previous_value == value {
                    self.segments.remove(&key);
                    self.segments.insert(previous_key, (upper, value));
                    continue;
                }
            }
            previous = Some(key);
        }
    }

    fn remove_range(&mut self, interval: Interval) {
        self.split_at(interval.lower);
        self.split_at(interval.upper);
        let keys: Vec<u32> = self
            .segments
            .range((
                Bound::Included(interval.lower),
                Bound::Excluded(interval.upper),
            ))
            .map(|(&lower, _)| lower)
            .collect();
        for key in keys {
            self.segments.remove(&key);
        }
    }

    /// Removes the interval from the map, as `erase` and `-=` with an interval do.
    pub fn erase(&mut self, interval: Interval) {
        if interval.is_empty() {
            return;
        }
        self.remove_range(interval);
    }

    /// Removes every interval of the set from the map, as `-=` with an interval set does.
    pub fn subtract(&mut self, set: &IntervalSet) {
        for (interval, _) in set.map.iter() {
            self.erase(interval);
        }
    }

    /// Associates the interval with the value, replacing what was there, as `set` does.
    pub fn set(&mut self, interval: Interval, value: V) {
        if interval.is_empty() {
            return;
        }
        self.remove_range(interval);
        if !value.is_identity() {
            self.segments
                .insert(interval.lower, (interval.upper, value));
        }
        self.join(interval);
    }
}

impl IntervalMap<i32> {
    /// Adds the value to the interval, aggregating it on overlapping segments and dropping the
    /// segments that reach zero, as `add` does.
    pub fn add(&mut self, interval: Interval, delta: i32) {
        if interval.is_empty() {
            return;
        }
        self.split_at(interval.lower);
        self.split_at(interval.upper);

        let mut cursor = interval.lower;
        let covered: Vec<(u32, u32, i32)> = self
            .segments
            .range((
                Bound::Included(interval.lower),
                Bound::Excluded(interval.upper),
            ))
            .map(|(&lower, &(upper, value))| (lower, upper, value))
            .collect();
        for (lower, upper, value) in covered {
            if cursor < lower && delta != 0 {
                self.segments.insert(cursor, (lower, delta));
            }
            let sum = value.wrapping_add(delta);
            if sum == 0 {
                self.segments.remove(&lower);
            } else {
                self.segments.insert(lower, (upper, sum));
            }
            cursor = upper;
        }
        if cursor < interval.upper && delta != 0 {
            self.segments.insert(cursor, (interval.upper, delta));
        }
        self.join(interval);
    }
}

/// A set of disjoint intervals that joins touching intervals, as `boost::icl::interval_set` does.
#[derive(Clone, Debug, Default)]
pub struct IntervalSet {
    map: IntervalMap<()>,
}

impl IntervalSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_interval(interval: Interval) -> Self {
        let mut set = Self::new();
        set.insert(interval);
        set
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The intervals in address order.
    pub fn iter(&self) -> impl Iterator<Item = Interval> + '_ {
        self.map.iter().map(|(interval, _)| interval)
    }

    /// The first interval, as `*begin()` returns it.
    pub fn first(&self) -> Option<Interval> {
        self.iter().next()
    }

    pub fn insert(&mut self, interval: Interval) {
        self.map.set(interval, ());
    }

    pub fn erase(&mut self, interval: Interval) {
        self.map.erase(interval);
    }

    /// The first interval that overlaps the interval, as `find` returns it.
    pub fn find(&self, interval: Interval) -> Option<Interval> {
        self.map.find(interval).map(|(interval, _)| interval)
    }

    /// The intervals of the set clipped to the interval, as `set & interval` returns them.
    pub fn intersect(&self, interval: Interval) -> IntervalSet {
        let mut result = IntervalSet::new();
        for (segment, _) in self.map.equal_range(interval) {
            let clipped = segment.intersect(&interval);
            if !clipped.is_empty() {
                result.insert(clipped);
            }
        }
        result
    }

    /// The intervals of the set that are not in the other set, as `set - set` returns them.
    pub fn difference(&self, other: &IntervalSet) -> IntervalSet {
        let mut result = self.clone();
        for interval in other.iter() {
            result.erase(interval);
        }
        result
    }

    pub fn overlaps(&self, interval: Interval) -> bool {
        self.iter().any(|segment| segment.overlaps(&interval))
    }
}
