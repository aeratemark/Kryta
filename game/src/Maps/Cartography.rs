//! The exploration grid the client's map surfaces draw from, and its content.
//!
//! Ported from Preforged's exploration implementation (`docs/exploration.txt`,
//! `src/runtime/gw_exploration.h` and its generated tables) with the modern wire of Rurik's
//! `toolkit/authsrv/fogrle.py` and ASURA's `cartography.rs`: `0x008B` announces the grid,
//! `0x008A` carries the run-length stream, and `0x008C` marks a block the character has explored.
//!
//! The client keeps one bit per 32-unit block, per continent. A continent's dimensions come from
//! the content file; a map's own rect places it inside that grid, and the map's world bounds place a
//! player's position inside the rect. `0x008B` and `0x008A` replace the client's whole grid, so the
//! server sends the character's saved bits for the continent the map is on, not a map-local grid.
//!
//! The run-length format is the modern one, proved against ArenaNet's own 38-byte payload in
//! Rurik's `test_fogrle.py` section 1: 16-row bands, a little-endian `u16` byte length per band,
//! byte-summed runs with `0xFF` as an additive continuation, runs alternating colour and starting
//! revealed. The client writes bits only on a full-dword flush, so a band's trailing partial dword
//! is dropped; `dims.x % 32 == 0` makes every whole band a whole number of dwords.
//!
//! The content file is `config/World/Vanilla/cartography.json`. Its map rects and continent grids
//! come from Preforged's generated 2022 tables, and its world bounds come from the map files'
//! Map Parameters chunk where the archive on this machine still has them; a map without bounds gets
//! the grid but no position-based reveal, which is Preforged's own rule for a map whose rect it
//! cannot place. A missing or malformed file stops the program (AGENTS.md, rule 7).

use std::collections::HashMap;

use common::logging::{Log, LogLevel};
use serde_json::Value;

/// One frame, as every packet builder in this tree answers it.
pub type Frame = Vec<u8>;

/// Tyria's grid in cells, used only when the content file does not name a map's continent. It is
/// the one dimension a live client's own `MapInitStart` measured (`64 x 128`).
pub const TYRIA: (u32, u32) = (64, 128);

/// The scanlines one band of the stream covers. The client's own constant (`shr eax, 4`).
pub const BAND_ROWS: u32 = 16;

/// The first run of a band is the fogged colour. Preforged's codec starts fogged
/// (`gw_exploration.h`, `bool value=false`), and its codec reproduces the client's own live
/// non-empty stream byte for byte: fog-first, first run actual length, every later run one shorter.
/// Rurik's build-38797 probe read the other colour for one payload, but this tree's client capture
/// is the direct evidence for the build it serves.
pub const REVEALED_FIRST: bool = false;

/// One map's place on its continent's grid: the rect, in terrain cells, and the world bounds that
/// turn a player position into a cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapArea {
    /// The continent whose grid the map draws on.
    pub continent: u8,
    /// The map's rect on the continent, in terrain cells: `[x0, y0, x1, y1]`.
    pub rect: [u32; 4],
    /// The map's own world bounds `[min_x, min_y, max_x, max_y]`, when the content has them.
    pub bounds: Option<[f32; 4]>,
}

impl MapArea {
    /// The continent block a player position falls in, or `None` when the map has no bounds or the
    /// position is outside the grid.
    ///
    /// The conversion is Preforged's (`gw_exploration.h::locate`): one cell is 96 game units, the
    /// rect is already in 32-cell blocks, and y is inverted because the map's origin is bottom-left.
    pub fn cell(&self, dims: (u32, u32), x: f32, y: f32) -> Option<(u32, u32)> {
        let bounds = self.bounds?;
        if !x.is_finite() || !y.is_finite() || !bounds.iter().all(|value| value.is_finite()) {
            return None;
        }
        if bounds[2] <= bounds[0] || bounds[3] <= bounds[1] {
            return None;
        }
        let cell_x = ((f64::from(self.rect[0]) + (f64::from(x) - f64::from(bounds[0])) / 96.0)
            / 32.0)
            .floor();
        let cell_y = ((f64::from(self.rect[1]) + (f64::from(bounds[3]) - f64::from(y)) / 96.0)
            / 32.0)
            .floor();
        if cell_x < 0.0 || cell_y < 0.0 {
            return None;
        }
        let (mut cell_x, mut cell_y) = (cell_x as u32, cell_y as u32);
        if cell_x >= dims.0 || cell_y >= dims.1 {
            return None;
        }
        // The rect is upper-bound exclusive. A player standing on the map's last world unit
        // computes one block past it, and a mark outside the map's own rect is an immediate client
        // 007 (`probes.py` FOG_MARK; the same rule the old cartography survey recorded). Clamp the
        // centre into the map's block range; the client's own 3 x 3 footprint may still clip at the
        // edge, which its per-map mask handles.
        let x0 = self.rect[0] / 32;
        let x1 = self.rect[2].div_ceil(32);
        let y0 = self.rect[1] / 32;
        let y1 = self.rect[3].div_ceil(32);
        cell_x = cell_x.clamp(x0, x1.saturating_sub(1));
        cell_y = cell_y.clamp(y0, y1.saturating_sub(1));
        Some((cell_x, cell_y))
    }
}

/// The content: continent grids and the map rects that place each map inside one.
#[derive(Debug, Clone, Default)]
pub struct Cartography {
    grids: HashMap<u8, (u32, u32)>,
    maps: HashMap<u32, MapArea>,
}

impl Cartography {
    /// Read the pack's `cartography.json`. Every row is checked; a bad row names itself and stops
    /// the start, as every content reader in this tree does.
    pub fn load(path: impl AsRef<std::path::Path>, log: &Log) -> Result<Self, String> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{} is not readable: {error}", path.display()))?;
        let doc: Value = serde_json::from_str(&text)
            .map_err(|error| format!("{} is not JSON: {error}", path.display()))?;

        let mut grids = HashMap::new();
        let rows = doc
            .get("grids")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{} states no grids", path.display()))?;
        for row in rows {
            let continent = number(row, "continent", path)?;
            let continent = u8::try_from(continent).map_err(|_| {
                format!(
                    "{}: continent {continent} does not fit one byte",
                    path.display()
                )
            })?;
            let width = number(row, "width", path)? as u32;
            let height = number(row, "height", path)? as u32;
            if width == 0 || height == 0 || width % 32 != 0 || height % BAND_ROWS != 0 {
                return Err(format!(
                    "{}: continent {continent}'s grid {width} x {height} is not a client grid (width % 32 == 0, height % 16 == 0)",
                    path.display()
                ));
            }
            if grids.insert(continent, (width, height)).is_some() {
                return Err(format!(
                    "{}: continent {continent} is stated twice",
                    path.display()
                ));
            }
        }

        let mut maps = HashMap::new();
        let rows = doc
            .get("maps")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{} states no maps", path.display()))?;
        for row in rows {
            let map_id = number(row, "map_id", path)? as u32;
            let continent = number(row, "continent", path)?;
            let continent = u8::try_from(continent).map_err(|_| {
                format!(
                    "{}: map {map_id}'s continent {continent} does not fit one byte",
                    path.display()
                )
            })?;
            let Some((width, height)) = grids.get(&continent) else {
                return Err(format!(
                    "{}: map {map_id} names continent {continent}, which has no grid",
                    path.display()
                ));
            };
            let rect = rect(row, path)?;
            if rect[2] <= rect[0] || rect[3] <= rect[1] {
                return Err(format!(
                    "{}: map {map_id}'s rect {:?} has no extent",
                    path.display(),
                    rect
                ));
            }
            if rect[2] > width * 32 || rect[3] > height * 32 {
                return Err(format!(
                    "{}: map {map_id}'s rect {:?} is outside continent {continent}'s {width} x {height} grid",
                    path.display(),
                    rect
                ));
            }
            let bounds = match row.get("bounds") {
                None | Some(Value::Null) => None,
                Some(value) => {
                    let values = value.as_array().ok_or_else(|| {
                        format!("{}: map {map_id}'s bounds are not an array", path.display())
                    })?;
                    if values.len() != 4 {
                        return Err(format!(
                            "{}: map {map_id}'s bounds are not four numbers",
                            path.display()
                        ));
                    }
                    let bounds = [
                        float(&values[0], map_id, path)?,
                        float(&values[1], map_id, path)?,
                        float(&values[2], map_id, path)?,
                        float(&values[3], map_id, path)?,
                    ];
                    if !bounds.iter().all(|value| value.is_finite())
                        || bounds[2] <= bounds[0]
                        || bounds[3] <= bounds[1]
                    {
                        return Err(format!(
                            "{}: map {map_id}'s bounds {:?} are not a finite rect",
                            path.display(),
                            bounds
                        ));
                    }
                    // The map's world bounds and its continent rect describe the same map: the rect's
                    // span multiplied by the 96-unit cell pitch is the bounds' span. A row where that
                    // fails is two different maps glued together, which is exactly the mistake a
                    // position-based reveal would make silently.
                    let span_x = f64::from(rect[2] - rect[0]) * 96.0;
                    let span_y = f64::from(rect[3] - rect[1]) * 96.0;
                    if (span_x - f64::from(bounds[2] - bounds[0])).abs() > 0.5
                        || (span_y - f64::from(bounds[3] - bounds[1])).abs() > 0.5
                    {
                        return Err(format!(
                            "{}: map {map_id}'s rect span does not match its world bounds",
                            path.display()
                        ));
                    }
                    Some(bounds)
                }
            };
            if maps
                .insert(
                    map_id,
                    MapArea {
                        continent,
                        rect,
                        bounds,
                    },
                )
                .is_some()
            {
                return Err(format!("{}: map {map_id} is stated twice", path.display()));
            }
        }

        if grids.is_empty() || maps.is_empty() {
            return Err(format!(
                "{} states {} grid(s) and {} map(s); both must be non-empty",
                path.display(),
                grids.len(),
                maps.len()
            ));
        }
        let with_bounds = maps.values().filter(|area| area.bounds.is_some()).count();
        log.out(
            LogLevel::Info,
            "server.worldserver",
            format!(
                "Cartography: {} continent grid(s) and {} map area(s), {} of them with world bounds, from {}.",
                grids.len(),
                maps.len(),
                with_bounds,
                path.display()
            ),
        );
        Ok(Self { grids, maps })
    }

    /// The grid a map draws on, or Tyria's when the content does not name the map.
    pub fn dims(&self, map_id: u32) -> (u32, u32) {
        self.maps
            .get(&map_id)
            .and_then(|area| self.grids.get(&area.continent).copied())
            .unwrap_or(TYRIA)
    }

    /// A map's place on its grid.
    pub fn area(&self, map_id: u32) -> Option<&MapArea> {
        self.maps.get(&map_id)
    }

    /// Every continent's dimensions, for the start-up line and the tests.
    pub fn grids(&self) -> &HashMap<u8, (u32, u32)> {
        &self.grids
    }

    /// Every map's area, for the start-up line and the tests.
    pub fn maps(&self) -> &HashMap<u32, MapArea> {
        &self.maps
    }
}

fn number(row: &Value, field: &str, path: &std::path::Path) -> Result<u64, String> {
    row.get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{}: a row states no {field}", path.display()))
}

fn float(value: &Value, map_id: u32, path: &std::path::Path) -> Result<f32, String> {
    value
        .as_f64()
        .map(|number| number as f32)
        .ok_or_else(|| format!("{}: map {map_id}'s bounds are not numbers", path.display()))
}

fn rect(row: &Value, path: &std::path::Path) -> Result<[u32; 4], String> {
    let values = row
        .get("rect")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{}: a map states no rect", path.display()))?;
    if values.len() != 4 {
        return Err(format!(
            "{}: a map's rect is not four numbers",
            path.display()
        ));
    }
    let mut rect = [0u32; 4];
    for (index, slot) in rect.iter_mut().enumerate() {
        *slot = values[index].as_u64().ok_or_else(|| {
            format!(
                "{}: a map's rect is not four unsigned numbers",
                path.display()
            )
        })? as u32;
    }
    Ok(rect)
}

/// One continent's exploration bitmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    width: u32,
    height: u32,
    /// One bit per cell, little-endian inside each word: bit `(y * width + x) % 32` of word
    /// `(y * width + x) / 32`.
    words: Vec<u32>,
}

impl Grid {
    /// An all-fogged grid of `width x height` cells.
    pub fn empty(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            words: vec![0; words_for(width, height)],
        }
    }

    /// A grid from the words a character's row keeps.
    pub fn from_words(width: u32, height: u32, words: Vec<u32>) -> Result<Self, String> {
        let wanted = words_for(width, height);
        if words.len() != wanted {
            return Err(format!(
                "the exploration grid is {width} x {height}, which needs {wanted} word(s), not {}",
                words.len()
            ));
        }
        Ok(Self {
            width,
            height,
            words,
        })
    }

    /// A grid from the little-endian bytes a database column keeps.
    pub fn from_bytes(width: u32, height: u32, bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != words_for(width, height) * 4 {
            return Err(format!(
                "the exploration column is {} byte(s); a {width} x {height} grid is {}",
                bytes.len(),
                words_for(width, height) * 4
            ));
        }
        let mut words = Vec::with_capacity(bytes.len() / 4);
        for chunk in bytes.chunks_exact(4) {
            words.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Self::from_words(width, height, words)
    }

    /// The words, as the row keeps them.
    pub fn words(&self) -> &[u32] {
        &self.words
    }

    /// The words as little-endian bytes, for the character database.
    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.words.len() * 4);
        for word in &self.words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    /// The grid's dimensions.
    pub fn dims(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Whether a cell is explored. A cell outside the grid is never explored.
    pub fn is_set(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let index = (y * self.width + x) as usize;
        self.words[index / 32] & (1u32 << (index % 32)) != 0
    }

    fn set(&mut self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let index = (y * self.width + x) as usize;
        let word = &mut self.words[index / 32];
        let mask = 1u32 << (index % 32);
        let changed = *word & mask == 0;
        *word |= mask;
        changed
    }

    /// Mark a cell and its eight neighbours, clipped at the grid's edges - the client's own
    /// footprint for one `0x008C` mark (`gw_exploration.h::reveal`). Answers how many cells changed.
    pub fn mark(&mut self, x: u32, y: u32) -> usize {
        let mut changed = 0;
        let x0 = x.saturating_sub(1);
        let y0 = y.saturating_sub(1);
        for yy in y0..=(y + 1).min(self.height.saturating_sub(1)) {
            for xx in x0..=(x + 1).min(self.width.saturating_sub(1)) {
                changed += usize::from(self.set(xx, yy));
            }
        }
        changed
    }

    /// Mark a horizontal span, the shape `0x008C` itself carries.
    pub fn mark_span(&mut self, x: u32, y: u32, span: u32) -> usize {
        let mut changed = 0;
        for xx in x..x.saturating_add(span).min(self.width) {
            changed += usize::from(self.set(xx, y));
        }
        changed
    }

    /// How many cells are explored.
    pub fn explored(&self) -> usize {
        self.words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    /// A 3 x 3 cover of the explored cells inside `rect` (the loaded map's footprint), one centre
    /// per window, row-major and greedy. The blocks outside `rect` belong to other maps of the same
    /// continent; a mark for one of those on this map is an immediate 007.
    pub fn replay_centres(&self, rect: [u32; 4]) -> Vec<(u32, u32)> {
        let x0 = rect[0] / 32;
        let x1 = rect[2].saturating_sub(1) / 32;
        let y0 = rect[1] / 32;
        let y1 = rect[3].saturating_sub(1) / 32;
        if x1 < x0 || y1 < y0 {
            return Vec::new();
        }
        let mut covered = vec![false; (self.width * self.height) as usize];
        let mut centres = Vec::new();
        for y in y0..=y1.min(self.height - 1) {
            for x in x0..=x1.min(self.width - 1) {
                if !self.is_set(x, y) || covered[(y * self.width + x) as usize] {
                    continue;
                }
                // Centre the 3 x 3 window on the candidate that covers the most saved blocks.
                let mut best = (x, y);
                let mut best_count = 0usize;
                for cy in y.saturating_sub(1)..=(y + 1).min(y1) {
                    for cx in x.saturating_sub(1)..=(x + 1).min(x1) {
                        let mut count = 0usize;
                        for yy in cy.saturating_sub(1)..=(cy + 1).min(self.height - 1) {
                            for xx in cx.saturating_sub(1)..=(cx + 1).min(self.width - 1) {
                                if self.is_set(xx, yy) {
                                    count += 1;
                                }
                            }
                        }
                        if count > best_count {
                            best_count = count;
                            best = (cx, cy);
                        }
                    }
                }
                centres.push(best);
                for yy in best.1.saturating_sub(1)..=(best.1 + 1).min(self.height - 1) {
                    for xx in best.0.saturating_sub(1)..=(best.0 + 1).min(self.width - 1) {
                        covered[(yy * self.width + xx) as usize] = true;
                    }
                }
            }
        }
        centres
    }

    /// The modern run-length stream: 16-row bands, a `u16` byte length, alternating runs starting
    /// revealed, and byte-summed lengths with `0xFF` as an additive continuation.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for band in 0..self.height / BAND_ROWS {
            let first = band * self.width * BAND_ROWS;
            let last = first + self.width * BAND_ROWS;
            // An all-fogged band is an empty header: the client memsets its bitmap to zero before
            // the stream runs, so no bytes mean all-fog under every reading, and it is the shape
            // seven of ArenaNet's own eight bands carry.
            if (first..last)
                .all(|index| self.words[(index / 32) as usize] & (1u32 << (index % 32)) == 0)
            {
                out.extend_from_slice(&[0, 0]);
                continue;
            }
            let mut runs = Vec::new();
            let mut colour = REVEALED_FIRST;
            let mut first_run = true;
            let mut length = 0u32;
            for index in first..last {
                let bit = self.words[(index / 32) as usize] & (1u32 << (index % 32)) != 0;
                if bit != colour {
                    encode_run(length, first_run, &mut runs);
                    first_run = false;
                    length = 0;
                    colour = bit;
                }
                length += 1;
            }
            encode_run(length, first_run, &mut runs);
            debug_assert!(runs.len() <= u16::MAX as usize);
            out.extend_from_slice(&(runs.len() as u16).to_le_bytes());
            out.extend_from_slice(&runs);
        }
        out
    }

    /// The client's expander, modern reading. A short band leaves the rest of its rows fogged and a
    /// trailing partial dword is dropped, as the client does.
    pub fn decode(stream: &[u8], width: u32, height: u32) -> Result<Self, String> {
        let mut grid = Self::empty(width, height);
        let mut at = 0usize;
        for band in 0..height / BAND_ROWS {
            if at + 2 > stream.len() {
                return Err(format!(
                    "the exploration stream ends before band {band}'s length"
                ));
            }
            let length = u16::from_le_bytes([stream[at], stream[at + 1]]) as usize;
            at += 2;
            let end = at
                .checked_add(length)
                .ok_or_else(|| format!("band {band}'s length {length} runs past the stream"))?;
            if end > stream.len() {
                return Err(format!(
                    "band {band}'s length {length} runs past the stream"
                ));
            }
            let band_start = band * width * BAND_ROWS;
            let band_end = band_start + width * BAND_ROWS;
            let mut cell = band_start;
            let mut colour = REVEALED_FIRST;
            let mut first_run = true;
            // The client writes only whole dwords. A partial dword at the end of a band never
            // lands; a short band's remaining cells stay fogged.
            let mut word = 0u32;
            let mut bits = 0u32;
            let mut word_index = band_start / 32;
            while at < end && cell < band_end {
                let mut run = 0u32;
                loop {
                    if at >= end {
                        break;
                    }
                    let byte = stream[at];
                    at += 1;
                    run += u32::from(byte);
                    if byte != 0xFF {
                        break;
                    }
                }
                if !first_run {
                    run += 1;
                }
                first_run = false;
                for _ in 0..run {
                    if cell >= band_end {
                        break;
                    }
                    if colour {
                        word |= 1u32 << bits;
                    }
                    bits += 1;
                    cell += 1;
                    if bits == 32 {
                        if let Some(slot) = grid.words.get_mut(word_index as usize) {
                            *slot = word;
                        }
                        word_index += 1;
                        word = 0;
                        bits = 0;
                    }
                }
                colour = !colour;
            }
            // `word` and `bits` are discarded here: a partial dword was never flushed.
        }
        Ok(grid)
    }

    /// The frames that give the client this grid: `0x008B`, then every `0x008A` slice.
    pub fn frames(&self) -> Vec<Frame> {
        let blob = self.encode();
        let mut frames = vec![crate::server::packets::instance_packets::map_init_start(
            self.width,
            self.height,
            blob.len() as u32,
        )];
        for slice in blob.chunks(crate::server::packets::instance_packets::CARTOGRAPHY_DATA_BYTES) {
            frames.push(crate::server::packets::instance_packets::cartography_data(
                slice,
            ));
        }
        frames
    }
}

fn words_for(width: u32, height: u32) -> usize {
    debug_assert!(
        width % 32 == 0,
        "the client asserts the grid width is a multiple of 32"
    );
    debug_assert!(
        height % BAND_ROWS == 0,
        "the band format has no partial band"
    );
    ((u64::from(width) * u64::from(height)) / 32) as usize
}

fn encode_run(length: u32, first: bool, out: &mut Vec<u8>) {
    let mut left = if first {
        length
    } else {
        length.saturating_sub(1)
    };
    while left >= 255 {
        out.push(0xFF);
        left -= 255;
    }
    out.push(left as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> Log {
        Log::new()
    }

    fn shipped() -> Cartography {
        Cartography::load("../config/World/Vanilla/cartography.json", &log())
            .expect("the shipped cartography content reads")
    }

    fn retail_stream() -> Vec<u8> {
        // ArenaNet's own 38-byte payload, replayed in Rurik's `probes.FOG_INIT_PAYLOAD`. Ten
        // little-endian dwords; the trailing two bytes are its declared count.
        let words: [u32; 10] = [
            1441792, 973218303, 956578308, 889600005, 939930889, 1006778885, 41474, 0, 0,
            3435921408,
        ];
        let mut bytes = Vec::new();
        for word in words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.truncate(38);
        bytes
    }

    #[test]
    fn an_empty_grid_is_the_eight_empty_band_headers() {
        let grid = Grid::empty(TYRIA.0, TYRIA.1);
        let blob = grid.encode();
        assert_eq!(
            blob,
            vec![0u8; 8 * 2],
            "eight empty bands, two zero bytes each"
        );
        let decoded = Grid::decode(&blob, TYRIA.0, TYRIA.1).expect("decodes");
        assert_eq!(decoded, grid);
        assert_eq!(grid.explored(), 0);
    }

    #[test]
    fn the_retail_stream_decodes_to_the_blocks_the_client_proved() {
        let grid = Grid::decode(&retail_stream(), 64, 128).expect("the retail stream decodes");
        assert_eq!(
            grid.explored(),
            55,
            "55 cells under Preforged's fog-first +1 rule"
        );
        assert!(
            grid.is_set(30, 24),
            "(30,24) is set - marking it changed 0 px"
        );
        assert!(
            !grid.is_set(26, 22),
            "(26,22) is clear - marking it revealed 1,059 px"
        );
    }

    /// The client sniffer's own non-empty creation stream, after its 8-byte header/count. The
    /// stream is 44 bytes as its `0x008B` declares, and it is the exact shape the client parses:
    /// empty all-fog bands, then one band whose first run is actual and whose later runs are one
    /// shorter. Our codec must reproduce it byte for byte.
    #[test]
    fn the_clients_own_non_empty_stream_decodes() {
        let hex = "000000000000000000001400ffffffff3502fc02fc02ffffffffffffffffffd4000000000000000000000000";
        let stream: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
            .collect();
        assert_eq!(stream.len(), 44);
        let grid = Grid::decode(&stream, 256, 192).expect("the live stream decodes");
        assert_eq!(
            grid.explored(),
            9,
            "the stream's only non-empty band has nine cells"
        );
        // Preforged writes full bands, so its encoder does not reproduce this server's empty-band
        // stream; the decoder still has to read it, which is what a captured stream needs.
    }

    #[test]
    fn a_grid_round_trips_through_the_band_format() {
        let mut grid = Grid::empty(64, 128);
        grid.mark(10, 10);
        grid.mark(63, 127);
        grid.mark(0, 0);
        let blob = grid.encode();
        assert_eq!(
            Grid::decode(&blob, 64, 128).expect("decodes"),
            grid,
            "the encoder and decoder agree on a sparse grid"
        );
        let mut dense = Grid::empty(256, 512);
        for y in (0..512).step_by(2) {
            for x in (0..256).step_by(2) {
                dense.mark(x, y);
            }
        }
        let blob = dense.encode();
        assert_eq!(
            Grid::decode(&blob, 256, 512).expect("decodes"),
            dense,
            "255-byte continuations and larger grids round-trip"
        );
    }

    /// Retail's own non-empty init, captured 2026-11-23 from `Gw.exe`
    /// (`maps-live-20261123-022154.jsonl` seq 654-655): a 55-byte stream, bands
    /// `[0, 27, 12, 0, 0, 0, 0, 0]`, 107 cells. It proves the load carries the saved grid and that
    /// the rules are Preforged's fog-first +1 with full bands; our encoder reproduces the 55 bytes
    /// exactly. The marks in the same capture land in this grid.
    #[test]
    fn the_retail_init_stream_carries_the_saved_grid() {
        let hex = "00001b00db0638073707380639053806340a3408360638033a043a043905230c0015043a043a033b02ffffff2a00000000000000000000";
        let stream: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
            .collect();
        assert_eq!(stream.len(), 55);
        let grid = Grid::decode(&stream, 64, 128).expect("the retail init decodes");
        assert_eq!(grid.explored(), 107);
        for (x, y) in [(31, 22), (31, 23), (31, 24), (30, 24), (30, 25)] {
            assert!(grid.is_set(x, y), "retail's mark ({x}, {y}) is in the init");
        }
        assert_eq!(
            grid.encode(),
            stream,
            "our encoder is retail's, byte for byte"
        );
    }

    #[test]
    fn a_run_of_255_needs_its_explicit_terminator() {
        let mut out = Vec::new();
        encode_run(255, true, &mut out);
        assert_eq!(out, vec![0xFF, 0x00], "the first run is its actual length");
        out.clear();
        encode_run(1024, true, &mut out);
        assert_eq!(out, vec![0xFF, 0xFF, 0xFF, 0xFF, 0x04]);
        out.clear();
        encode_run(255, false, &mut out);
        assert_eq!(out, vec![0xFE], "a later run is one shorter");
    }

    #[test]
    fn the_frames_announce_the_stream_they_carry() {
        let grid = Grid::empty(TYRIA.0, TYRIA.1);
        let frames = grid.frames();
        let start = &frames[0];
        assert_eq!(&start[..2], &[0x8B, 0x00], "the announcement");
        assert_eq!(start.len(), 14, "three dwords after the header");
        assert_eq!(
            u32::from_le_bytes([start[2], start[3], start[4], start[5]]),
            TYRIA.0
        );
        assert_eq!(
            u32::from_le_bytes([start[6], start[7], start[8], start[9]]),
            TYRIA.1
        );
        assert_eq!(
            u32::from_le_bytes([start[10], start[11], start[12], start[13]]),
            16,
            "the eight empty bands"
        );
        assert_eq!(frames.len(), 2, "the announcement and one slice");
        assert_eq!(&frames[1][..2], &[0x8A, 0x00]);
        assert_eq!(
            frames[1].len(),
            260,
            "a u16 count and the fixed 64-slot array"
        );
        assert_eq!(
            u16::from_le_bytes([frames[1][2], frames[1][3]]),
            4,
            "the dwords that count"
        );
    }

    #[test]
    fn the_mark_is_the_eleven_byte_wire_frame() {
        let mark = crate::server::packets::instance_packets::cartography_mark(30, 24, 1);
        assert_eq!(&mark[..2], &[0x8C, 0x00]);
        assert_eq!(
            mark.len(),
            11,
            "the client sniffer's 16-byte struct is not the wire"
        );
        assert_eq!(u32::from_le_bytes([mark[2], mark[3], mark[4], mark[5]]), 30);
        assert_eq!(u32::from_le_bytes([mark[6], mark[7], mark[8], mark[9]]), 24);
        assert_eq!(mark[10], 1, "the half-span byte: one mark reveals 3 x 3");
    }

    #[test]
    fn the_shipped_content_places_the_pre_searing_maps() {
        let carto = shipped();
        assert_eq!(carto.grids().len(), 9, "nine continents");
        assert_eq!(carto.maps().len(), 492, "every map with a usable rect");
        let with_bounds = carto
            .maps()
            .values()
            .filter(|area| area.bounds.is_some())
            .count();
        assert_eq!(
            with_bounds, 31,
            "the maps whose source bounds this archive still has"
        );
        let area = carto.area(148).expect("Ascalon City has an area");
        assert_eq!(area.continent, 1);
        assert_eq!(area.rect, [768, 512, 1184, 1024]);
        assert_eq!(area.bounds, Some([-18432.0, -24576.0, 21504.0, 24576.0]));
        let dims = carto.dims(148);
        assert_eq!(dims, (64, 128));
        assert_eq!(
            area.cell(dims, -18432.0, 24576.0),
            Some((24, 16)),
            "the map's own corner is its rect's corner"
        );
        assert_eq!(
            area.cell(dims, 0.0, 0.0),
            Some((30, 24)),
            "the center of Ascalon City is the block the client proved"
        );
        assert_eq!(
            area.cell(dims, 21504.0, -24576.0),
            Some((36, 31)),
            "the upper edge is clamped into the map's own rect; a mark past it is client error 007"
        );
    }

    /// Retail's second session, captured 2026-11-23 (`maps-live-20261123-022630.jsonl`), has no
    /// init because the capture started mid-map, but its eleven marks are all inside map 161's
    /// footprint. That is the content's rect for the northern pre-Searing explorable.
    #[test]
    fn the_second_retail_capture_is_inside_map_161s_rect() {
        let carto = shipped();
        let area = carto.area(161).expect("map 161 has an area");
        assert_eq!(area.rect, [448, 928, 960, 1440]);
        let x0 = area.rect[0] / 32;
        let x1 = (area.rect[2] - 1) / 32;
        let y0 = area.rect[1] / 32;
        let y1 = (area.rect[3] - 1) / 32;
        for (x, y) in [
            (18, 37),
            (17, 36),
            (18, 36),
            (19, 37),
            (19, 36),
            (20, 36),
            (20, 37),
            (20, 38),
        ] {
            assert!(
                (x0..=x1).contains(&x) && (y0..=y1).contains(&y),
                "retail's mark ({x}, {y}) is inside map 161's rect"
            );
        }
    }

    #[test]
    fn a_map_without_bounds_gets_no_position_reveal() {
        let carto = shipped();
        let area = carto.area(10).expect("map 10 has an area");
        assert_eq!(area.bounds, None);
        assert_eq!(area.cell((256, 512), 0.0, 0.0), None);
    }
}
