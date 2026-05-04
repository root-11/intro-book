//! Reference implementation for §5–§10. The deck program grown across the
//! Identity & structure phase. Each section's contributions are commented.
//!
//! Run:    cargo run --release
//! Tests:  cargo test --release
//!
//! The deck has no `Card` struct. The card at slot `i` is the row
//! `(suits[i], ranks[i], locations[i], dealt_at[i], ids[i])`. Every
//! reordering must touch all five columns — that is the §6 contract.

// =================================================================
// §5 — Identity is an integer
// =================================================================

const N_CARDS: usize = 52;

// Suits: 0=Spades, 1=Hearts, 2=Diamonds, 3=Clubs.
// Ranks: 1=Ace, 2-10=pip, 11=J, 12=Q, 13=K.
// Locations: 0=deck, 1-4=players, 5=discard.

fn new_deck() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut suits = Vec::with_capacity(N_CARDS);
    let mut ranks = Vec::with_capacity(N_CARDS);
    let locations = vec![0u8; N_CARDS];
    for s in 0..4u8 {
        for r in 1..=13u8 {
            suits.push(s);
            ranks.push(r);
        }
    }
    (suits, ranks, locations)
}

fn card_to_string(suit: u8, rank: u8) -> String {
    let suit_char = match suit { 0 => '♠', 1 => '♥', 2 => '♦', _ => '♣' };
    let rank_str = match rank {
        1 => "A".to_string(),
        11 => "J".to_string(),
        12 => "Q".to_string(),
        13 => "K".to_string(),
        n => n.to_string(),
    };
    format!("{rank_str}{suit_char}")
}

// LCG random — Numerical Recipes constants. Seeded; deterministic across runs.
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self { Self(seed) }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0
    }
    fn next_range(&mut self, n: u64) -> u64 { self.next() % n }
}

fn fisher_yates_shuffle(order: &mut [usize], rng: &mut Lcg) {
    for i in (1..order.len()).rev() {
        let j = rng.next_range(i as u64 + 1) as usize;
        order.swap(i, j);
    }
}

// =================================================================
// §6 — A row is a tuple
// =================================================================

/// Return the row at slot `i`. The same `i` against every column gives the row.
fn row(
    suits: &[u8], ranks: &[u8], locations: &[u8], dealt_at: &[u32], ids: &[u32], i: usize,
) -> (u8, u8, u8, u32, u32) {
    (suits[i], ranks[i], locations[i], dealt_at[i], ids[i])
}

/// The single-writer rule: the only function that may reorder any column
/// of the deck. All five columns are reordered in lockstep.
fn reorder_deck(
    suits:     &mut Vec<u8>,
    ranks:     &mut Vec<u8>,
    locations: &mut Vec<u8>,
    dealt_at:  &mut Vec<u32>,
    ids:       &mut Vec<u32>,
    order:     &[usize],
) {
    let new_suits:     Vec<u8>  = order.iter().map(|&i| suits[i]).collect();
    let new_ranks:     Vec<u8>  = order.iter().map(|&i| ranks[i]).collect();
    let new_locations: Vec<u8>  = order.iter().map(|&i| locations[i]).collect();
    let new_dealt_at:  Vec<u32> = order.iter().map(|&i| dealt_at[i]).collect();
    let new_ids:       Vec<u32> = order.iter().map(|&i| ids[i]).collect();
    *suits     = new_suits;
    *ranks     = new_ranks;
    *locations = new_locations;
    *dealt_at  = new_dealt_at;
    *ids       = new_ids;
}

fn sort_deck_by_suit_then_rank(
    suits: &mut Vec<u8>, ranks: &mut Vec<u8>, locations: &mut Vec<u8>,
    dealt_at: &mut Vec<u32>, ids: &mut Vec<u32>,
) {
    let mut order: Vec<usize> = (0..suits.len()).collect();
    order.sort_by_key(|&i| (suits[i], ranks[i]));
    reorder_deck(suits, ranks, locations, dealt_at, ids, &order);
}

// =================================================================
// §8 — Where there's one, there's many. Every query is a function over slices.
// =================================================================

/// Returns the *ids* (not slots) of cards held by `player`. Ids survive a sort;
/// slots do not. See §9 and §10 for why.
fn cards_held_by(locations: &[u8], ids: &[u32], player: u8) -> Vec<u32> {
    let mut out = Vec::new();
    for i in 0..locations.len() {
        if locations[i] == player { out.push(ids[i]); }
    }
    out
}

fn highest_rank_in_hand(hand_ids: &[u32], ids: &[u32], ranks: &[u8]) -> Option<u8> {
    let mut best: Option<u8> = None;
    for &id in hand_ids {
        if let Some(slot) = slot_of(ids, id) {
            best = Some(best.map_or(ranks[slot], |b| b.max(ranks[slot])));
        }
    }
    best
}

fn count_by_location(locations: &[u8]) -> [usize; 6] {
    let mut counts = [0usize; 6];
    for &l in locations { counts[l as usize] += 1; }
    counts
}

// =================================================================
// §10 — Stable IDs. `slot_of` is O(N) linear search; for 52 cards it's
// nothing. For variable-quantity tables, see §23 (Index maps).
// =================================================================

fn slot_of(ids: &[u32], target: u32) -> Option<usize> {
    ids.iter().position(|&id| id == target)
}

// =================================================================
// Demonstration
// =================================================================

fn main() {
    let (mut suits, mut ranks, mut locations) = new_deck();
    let mut dealt_at: Vec<u32> = vec![u32::MAX; N_CARDS];
    let mut ids:      Vec<u32> = (0..52u32).collect();

    // Shuffle (§5.3). Reorder via order vector — the columns themselves
    // are touched only by reorder_deck.
    let mut order: Vec<usize> = (0..N_CARDS).collect();
    let mut rng = Lcg::new(0x1234_5678);
    fisher_yates_shuffle(&mut order, &mut rng);
    reorder_deck(&mut suits, &mut ranks, &mut locations, &mut dealt_at, &mut ids, &order);

    println!("Deck after shuffle (first 10):");
    for i in 0..10 {
        println!("  slot {i:>2}: id={:>2} {}", ids[i], card_to_string(suits[i], ranks[i]));
    }

    // §5.5 — deal 5 cards each to 4 players from the top of the deck.
    let tick: u32 = 0;
    for player in 1u8..=4 {
        for _ in 0..5 {
            let slot = locations.iter().position(|&l| l == 0).expect("deck not empty");
            locations[slot] = player;
            dealt_at[slot]  = tick;
        }
    }

    println!("\nCounts by location: {:?}", count_by_location(&locations));

    // §10.4 — query holdings by stable ids.
    println!();
    for player in 1u8..=4 {
        let held = cards_held_by(&locations, &ids, player);
        let strs: Vec<String> = held.iter()
            .map(|&id| {
                let s = slot_of(&ids, id).unwrap();
                card_to_string(suits[s], ranks[s])
            })
            .collect();
        println!("Player {player} (ids {:?}): {}", held, strs.join(" "));
    }

    // §10.3 — sort the deck after the deal. Player 1's id list is unchanged;
    // their slots have moved. cards_held_by(locations, ids, 1) returns the
    // same five ids as before the sort, and slot_of(ids, id) finds them.
    let p1_before = cards_held_by(&locations, &ids, 1);
    sort_deck_by_suit_then_rank(&mut suits, &mut ranks, &mut locations, &mut dealt_at, &mut ids);
    let p1_after = cards_held_by(&locations, &ids, 1);

    println!("\nPlayer 1 ids before sort: {:?}", p1_before);
    println!("Player 1 ids after sort:  {:?}", p1_after);
    println!("(same set; their slots changed but their identity did not)");

    // Use a few of the §6/§8 utilities to silence unused warnings.
    let r17 = row(&suits, &ranks, &locations, &dealt_at, &ids, 17);
    println!("\nrow(17) = (suit={}, rank={}, loc={}, dealt_at={:?}, id={})",
             r17.0, r17.1, r17.2, r17.3, r17.4);
    if let Some(h) = highest_rank_in_hand(&p1_after, &ids, &ranks) {
        println!("Player 1 highest rank: {h}");
    }
}

// =================================================================
// Tests — exercises whose claims are verifiable.
// =================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_deck_has_52_unique_cards() {
        let (suits, ranks, locations) = new_deck();
        assert_eq!(suits.len(), 52);
        assert_eq!(ranks.len(), 52);
        assert_eq!(locations.len(), 52);
        assert!(locations.iter().all(|&l| l == 0));
        let mut seen = std::collections::HashSet::new();
        for i in 0..52 {
            assert!(seen.insert((suits[i], ranks[i])));
        }
        assert_eq!(seen.len(), 52);
    }

    #[test]
    fn shuffle_preserves_card_identities() {
        // §5: shuffling the order vector must not change the (suit, rank) set.
        let (suits0, ranks0, _) = new_deck();
        let original_pairs: std::collections::HashSet<(u8, u8)> =
            suits0.iter().zip(&ranks0).map(|(&s, &r)| (s, r)).collect();

        let (mut suits, mut ranks, mut locations) = new_deck();
        let mut dealt_at = vec![u32::MAX; N_CARDS];
        let mut ids: Vec<u32> = (0..52).collect();
        let mut order: Vec<usize> = (0..N_CARDS).collect();
        let mut rng = Lcg::new(42);
        fisher_yates_shuffle(&mut order, &mut rng);
        reorder_deck(&mut suits, &mut ranks, &mut locations, &mut dealt_at, &mut ids, &order);

        let after_pairs: std::collections::HashSet<(u8, u8)> =
            suits.iter().zip(&ranks).map(|(&s, &r)| (s, r)).collect();
        assert_eq!(original_pairs, after_pairs);
    }

    #[test]
    fn sort_does_not_break_id_lookup() {
        // §10.3: after sorting, slot_of(ids, id) still finds every card.
        let (mut suits, mut ranks, mut locations) = new_deck();
        let mut dealt_at = vec![u32::MAX; N_CARDS];
        let mut ids: Vec<u32> = (0..52).collect();

        // Shuffle, then deal player 1 the first five.
        let mut order: Vec<usize> = (0..N_CARDS).collect();
        let mut rng = Lcg::new(42);
        fisher_yates_shuffle(&mut order, &mut rng);
        reorder_deck(&mut suits, &mut ranks, &mut locations, &mut dealt_at, &mut ids, &order);
        for _ in 0..5 {
            let s = locations.iter().position(|&l| l == 0).unwrap();
            locations[s] = 1;
        }

        let held_before = cards_held_by(&locations, &ids, 1);
        let cards_before: Vec<(u8, u8)> = held_before.iter()
            .map(|&id| { let s = slot_of(&ids, id).unwrap(); (suits[s], ranks[s]) })
            .collect();

        sort_deck_by_suit_then_rank(&mut suits, &mut ranks, &mut locations, &mut dealt_at, &mut ids);

        let held_after = cards_held_by(&locations, &ids, 1);
        let cards_after: Vec<(u8, u8)> = held_after.iter()
            .map(|&id| { let s = slot_of(&ids, id).unwrap(); (suits[s], ranks[s]) })
            .collect();

        // The *set* of ids is unchanged. Their order is not — `cards_held_by`
        // walks in slot order, and the sort moves slots around.
        assert_eq!(
            held_before.iter().copied().collect::<std::collections::HashSet<_>>(),
            held_after.iter().copied().collect::<std::collections::HashSet<_>>(),
            "id set unchanged"
        );
        assert_eq!(
            cards_before.iter().collect::<std::collections::HashSet<_>>(),
            cards_after.iter().collect::<std::collections::HashSet<_>>(),
            "(suit, rank) pairs unchanged"
        );
    }

    #[test]
    fn deal_moves_cards_out_of_deck() {
        let (mut suits, mut ranks, mut locations) = new_deck();
        let mut dealt_at = vec![u32::MAX; N_CARDS];
        let mut ids: Vec<u32> = (0..52).collect();
        let _ = (&mut suits, &mut ranks, &mut dealt_at, &mut ids); // (unused-mut quietener)
        for _ in 0..5 {
            let s = locations.iter().position(|&l| l == 0).unwrap();
            locations[s] = 1;
        }
        let counts = count_by_location(&locations);
        assert_eq!(counts[0], 47);
        assert_eq!(counts[1], 5);
    }

    #[test]
    fn highest_rank_in_empty_hand_is_none() {
        let (_, ranks, _) = new_deck();
        let ids: Vec<u32> = (0..52).collect();
        assert_eq!(highest_rank_in_hand(&[], &ids, &ranks), None);
    }
}
