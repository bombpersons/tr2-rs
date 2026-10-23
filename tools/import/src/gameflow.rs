//! `DATA/TOMBPSX.DAT`, the gameflow script: level names and files, the
//! game's menu and inventory strings, and each level's puzzle, pickup and
//! key names. Read in the order of its loader `0x8006BBC8`: a version (3),
//! a 256-byte description, a 128-byte header, then string tables of
//! `[u16 offset; n] [u16 size] [size bytes]`, XOR'd with the header's key
//! when its flag 0x100 is set (`0x8006BA94`).
use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, String>;

pub const PATH: &str = "/DATA/TOMBPSX.DAT";

/// The strings the game shows, per table of the script.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Gameflow {
    /// Level titles by level number (0 is Lara's Home).
    pub levels: Vec<String>,
    /// The level files, `data\wall.PSX`, by level number.
    pub level_files: Vec<String>,
    /// How many of the last levels are only for demos (header +0x4A,
    /// `0x8008C372` at run time): the game's own levels are the others.
    #[serde(default)]
    pub demo_levels: usize,
    /// The 89 game strings: ring headings, item names, statistics labels.
    pub game_strings: Vec<String>,
    /// The 80 PlayStation strings: options, memory card, button prompts.
    pub psx_strings: Vec<String>,
    /// Puzzle items 1-4, each by level number.
    pub puzzles: Vec<Vec<String>>,
    /// Pickup items 1-2, each by level number.
    pub pickups: Vec<Vec<String>>,
    /// Keys 1-4, each by level number.
    pub keys: Vec<Vec<String>>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    key: Option<u8>,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let bytes = self.bytes.get(self.at..self.at + n).ok_or_else(|| format!("script ends at {}", self.at))?;
        self.at += n;
        Ok(bytes)
    }
    fn u16(&mut self) -> Result<usize> {
        Ok(usize::from(u16::from_le_bytes(self.take(2)?.try_into().unwrap())))
    }
    fn u16s(&mut self, n: usize) -> Result<Vec<usize>> {
        (0..n).map(|_| self.u16()).collect()
    }
    fn strings(&mut self, n: usize) -> Result<Vec<String>> {
        let offsets = self.u16s(n)?;
        let size = self.u16()?;
        let key = self.key;
        let data: Vec<u8> = self.take(size)?.iter().map(|&b| key.map_or(b, |k| b ^ k)).collect();
        offsets
            .into_iter()
            .map(|at| {
                let text = data.get(at..).ok_or_else(|| format!("string at {at} past its table"))?;
                let end = text.iter().position(|&b| b == 0).ok_or("unterminated string")?;
                Ok(text[..end].iter().map(|&b| char::from(b)).collect())
            })
            .collect()
    }
}

pub fn decode(bytes: &[u8]) -> Result<Gameflow> {
    let r = &mut Reader { bytes, at: 0, key: None };
    let version = u32::from_le_bytes(r.take(4)?.try_into().unwrap());
    if version != 3 {
        return Err(format!("script version {version}, not 3"));
    }
    r.take(256)?;
    if r.u16()? != 128 {
        return Err("script header is not 128 bytes".into());
    }
    let header = r.take(128)?.to_vec();
    let count = |at: usize| usize::from(u16::from_le_bytes([header[at], header[at + 1]]));
    let (levels, pictures, titles, fmvs, cutscenes, demos) =
        (count(0x40), count(0x42), count(0x44), count(0x46), count(0x48), count(0x4A));
    if u32::from_le_bytes(header[0x70..0x74].try_into().unwrap()) & 0x100 != 0 {
        r.key = Some(header[0x78]);
    }
    let level_names = r.strings(levels)?;
    r.strings(pictures)?;
    r.strings(titles)?;
    r.strings(fmvs)?;
    let level_files = r.strings(levels)?;
    r.strings(cutscenes)?;
    // The sequence script: an offset per level and the frontend, then its words.
    r.u16s(levels + 1)?;
    let size = r.u16()?;
    r.take(size)?;
    r.take(2 * demos + 8 * fmvs)?;
    let game_count = r.u16()?;
    if game_count != 89 {
        return Err(format!("{game_count} game strings, not 89"));
    }
    let game_strings = r.strings(game_count)?;
    let psx_strings = r.strings(80)?;
    let mut tables = |n| (0..n).map(|_| r.strings(levels)).collect::<Result<Vec<_>>>();
    let (puzzles, pickups, keys) = (tables(4)?, tables(2)?, tables(4)?);
    if r.at != bytes.len() {
        return Err(format!("{} bytes after the last table", bytes.len() - r.at));
    }
    Ok(Gameflow {
        levels: level_names,
        level_files,
        demo_levels: demos,
        game_strings,
        psx_strings,
        puzzles,
        pickups,
        keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(strings: &[&str], key: u8) -> Vec<u8> {
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for s in strings {
            offsets.extend((data.len() as u16).to_le_bytes());
            data.extend(s.bytes().chain([0]).map(|b| b ^ key));
        }
        [offsets, (data.len() as u16).to_le_bytes().to_vec(), data].concat()
    }

    #[test]
    fn a_small_script_decodes_its_tables() {
        let key = 0xA6;
        let mut header = vec![0u8; 128];
        header[0x40] = 2; // levels
        header[0x42] = 1; // pictures
        header[0x70..0x74].copy_from_slice(&0x100u32.to_le_bytes());
        header[0x78] = key;
        let names = ["Home", "Wall"];
        let game: Vec<String> = (0..89).map(|i| format!("g{i}")).collect();
        let psx: Vec<String> = (0..80).map(|i| format!("p{i}")).collect();
        let mut bytes = [3u32.to_le_bytes().to_vec(), vec![0; 256], 128u16.to_le_bytes().to_vec(), header].concat();
        bytes.extend(table(&names, key));
        bytes.extend(table(&["pix"], key));
        bytes.extend(table(&[], key)); // titles
        bytes.extend(table(&[], key)); // FMVs
        bytes.extend(table(&["home.psx", "wall.psx"], key));
        bytes.extend(table(&[], key)); // cutscenes
        bytes.extend([0, 0, 2, 0, 0, 0, 4, 0, 2, 0, 9, 0]); // three sequence offsets, two words
        bytes.extend(89u16.to_le_bytes());
        bytes.extend(table(&game.iter().map(String::as_str).collect::<Vec<_>>(), key));
        bytes.extend(table(&psx.iter().map(String::as_str).collect::<Vec<_>>(), key));
        for slot in 0..10 {
            bytes.extend(table(&[&format!("s{slot}a"), &format!("s{slot}b")], key));
        }
        let flow = decode(&bytes).unwrap();
        assert_eq!(flow.levels, ["Home", "Wall"]);
        assert_eq!(flow.level_files[1], "wall.psx");
        assert_eq!(flow.game_strings[88], "g88");
        assert_eq!(flow.psx_strings[0], "p0");
        assert_eq!(flow.puzzles[3][1], "s3b");
        assert_eq!(flow.pickups[0][0], "s4a");
        assert_eq!(flow.keys[0][1], "s6b");
    }
}
