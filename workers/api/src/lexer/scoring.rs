/// Score a list of keyword matches for a single document into a single score.
pub fn score_collective_keywords(data: &Vec<(String, f64)>) -> f64 {
    let total_matches = data.len() as u32;
    if total_matches == 1u32 {
        data[0].1
    } else {
        data.iter().map(|(_, score)| *score).sum::<f64>() / (total_matches as f64)
    }
}
