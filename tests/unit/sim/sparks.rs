//! Tests for `src/sim/sparks.rs`: sparks are kept for their lifetime only.

use super::*;

#[test]
fn sparks_last_their_lifetime_and_no_more() {
    let mut sparks = HeatSparks::new();
    sparks.record(Vec3::X, 1.0);
    sparks.advance(SPARK_SECONDS / 2.0);
    sparks.record(Vec3::Y, -1.0);
    assert_eq!(sparks.recent().count(), 2);
    sparks.advance(SPARK_SECONDS * 0.75);
    let left: Vec<f32> = sparks.recent().map(|spark| spark.heat).collect();
    assert_eq!(left, [-1.0], "the first spark has expired");
}

#[test]
fn the_oldest_sparks_go_first_when_full() {
    let mut sparks = HeatSparks::new();
    for index in 0..MAX_SPARKS + 10 {
        sparks.record(Vec3::ZERO, index as f32);
    }
    assert_eq!(sparks.recent().count(), MAX_SPARKS);
    assert_eq!(sparks.recent().next().unwrap().heat, 10.0);
}
