use rand::prelude::IndexedRandom;
use rand::rng;
use rand::seq::SliceRandom;

const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUWXYZ";
const DIGITS: &[u8] = b"1234567890";
const SPECIAL: &[u8] = b"!@%#%^&*()-=_+";
const LENGTH: usize = 16;

/// Port of the PHP `randomPassword()`: 16 chars — letters, at least two
/// digits, two specials — shuffled so the first char is a letter.
pub fn random_password() -> String {
    let mut rng = rng();
    let mut pool: Vec<u8> = Vec::with_capacity(LENGTH);

    pool.extend(LETTERS.choose_multiple(&mut rng, LENGTH - 4));
    pool.extend(DIGITS.choose_multiple(&mut rng, 2));
    pool.extend(SPECIAL.choose_multiple(&mut rng, 2));

    loop {
        pool.shuffle(&mut rng);
        if LETTERS.contains(&pool[0]) {
            break;
        }
    }

    String::from_utf8(pool).expect("password alphabet is ASCII")
}
