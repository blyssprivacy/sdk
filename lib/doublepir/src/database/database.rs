use std::slice;

use crate::{
    arith::arith::{base_p, reconstruct_from_base_p},
    matrix::{Matrix, MatrixRef, SquishParams, Squishable},
    params::Params,
};

fn bits_from_byte(byte: u8) -> [u8; 8] {
    [
        byte & (1),
        (byte & (1 << 1)) >> 1,
        (byte & (1 << 2)) >> 2,
        (byte & (1 << 3)) >> 3,
        (byte & (1 << 4)) >> 4,
        (byte & (1 << 5)) >> 5,
        (byte & (1 << 6)) >> 6,
        (byte & (1 << 7)) >> 7,
    ]
}

/// Structure specifying the layout of the database.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct DbInfo {
    /// Number of DB entries.
    pub num_entries: u64,
    /// Number of bits per DB entry.
    pub bits_per_entry: u64,

    /// Number of DB entries per Z_p elem, if log(p) > DB entry size.
    pub packing: usize,
    /// Number of Z_p elems per DB entry, if DB entry size > log(p).
    pub ne: usize,

    /// Tunable param that governs communication.
    ///
    /// Must be in range [1, ne] and must be a divisor of ne;
    /// represents the number of times the scheme is repeated.
    pub x: usize,

    /// Plaintext modulus.
    pub p: u64,
    /// (Logarithm of) ciphertext modulus.
    pub logq: u64,

    /// Parameters for in-memory DB compression.
    pub squish_params: SquishParams,

    /// Original number of columns in the database, pre-squish.
    pub orig_cols: usize,
}

impl DbInfo {
    /// Generates the database info.
    ///
    /// Takes in the number of entries, and the number of bits per entry,
    /// in the database. Does not allocate or fill any underlying data.
    pub fn new(num_entries: u64, bits_per_entry: u64, params: &Params) -> Self {
        assert!(num_entries > 0);
        assert!(bits_per_entry > 0);
        assert!(
            bits_per_entry < 64,
            "{} is too many bits per entry; the max supported is {}",
            bits_per_entry,
            64
        );

        let (db_elems, elems_per_entry, entries_per_elem) =
            num_db_entries(num_entries, bits_per_entry, params.p);

        let mut info = DbInfo {
            num_entries,
            bits_per_entry,
            p: params.p,
            packing: entries_per_elem,
            ne: elems_per_entry,
            x: elems_per_entry,
            logq: params.logq,
            squish_params: SquishParams::default(),
            orig_cols: 0,
        };

        while info.ne % info.x != 0 {
            info.x += 1;
        }

        assert!(db_elems <= params.l * params.m);
        info
    }
}

/// The PIR database and its layout information.
#[derive(Debug, PartialEq)]
pub struct Db {
    pub info: DbInfo,
    pub data: Matrix,

    pub db_rows: usize,
    pub db_cols: usize,
    pub raw_data: Vec<u8>,
}

impl Db {
    /// Create a new, empty database.
    ///
    /// Takes in the number of entries, and the number of bits per entry,
    /// in the database. Does not allocate or fill any underlying data.
    pub fn new(num_entries: u64, bits_per_entry: u64, params: &Params) -> Self {
        let info = DbInfo::new(num_entries, bits_per_entry, params);
        let dummy = Matrix::new(0, 0);
        Db {
            info,
            data: dummy,
            db_rows: 0,
            db_cols: 0,
            raw_data: Vec::new(),
        }
    }

    pub fn num_rows(&self) -> usize {
        if self.db_rows > 0 {
            self.db_rows
        } else {
            self.data.rows
        }
    }

    pub fn num_cols(&self) -> usize {
        if self.db_cols > 0 {
            self.db_cols
        } else {
            self.data.cols
        }
    }

    pub fn get_nice_slice(&self) -> &[u32] {
        if self.raw_data.len() > 0 {
            unsafe {
                let ptr = self.raw_data.as_ptr() as *const u32;
                let slice: &[u32] = slice::from_raw_parts(ptr, self.raw_data.len() / 4);
                slice
            }
        } else {
            &self.data.data
        }
    }

    pub fn get_mat_ref(&self) -> MatrixRef {
        MatrixRef {
            rows: self.num_rows(),
            cols: self.num_cols(),
            data: self.get_nice_slice(),
        }
    }

    /// Creates a new database, filled with random data.
    pub fn random(num_entries: u64, bits_per_entry: u64, params: &Params) -> Self {
        let mut db = Self::new(num_entries, bits_per_entry, params);
        db.data = Matrix::random_mod(params.l, params.m, params.p as u32);
        db
    }

    /// Loads a new database, filled data from the given iterator.
    ///
    /// The current contents of the database will be completely overridden.
    /// The iterator `data` should yield at least `num_entries` items, each a
    /// value of at most `bits_per_entry` bits.
    pub fn load_data<I: Iterator<Item = u8>>(
        &mut self,
        bits_per_entry: u64,
        params: &Params,
        data: I,
    ) {
        let mut iter = data.enumerate().peekable();
        self.data = Matrix::new(params.l, params.m);

        if self.info.packing > 0 {
            // Pack multiple DB elems into each Z_p elem
            let mut at = 0;
            let mut cur = 0u32;
            let mut coeff = 1u32;
            while let Some((i, elem)) = iter.next() {
                cur += (elem as u32) * coeff;
                coeff *= 1 << bits_per_entry;
                if ((i + 1) % (self.info.packing as usize) == 0) || (iter.peek().is_none()) {
                    self.data[at / params.m][at % params.m] = cur as u32;
                    at += 1;
                    cur = 0;
                    coeff = 1
                }
            }
        } else {
            // Use multiple Z_p elems to represent each DB elem
            for (i, elem) in iter {
                for j in 0..self.info.ne {
                    // let elem = u64::from_ne_bytes(elem_ref.as_slice().try_into().unwrap());
                    let row = (i / params.m) * (self.info.ne as usize) + (j as usize);
                    let col = i % (params.m as usize);
                    self.data[row][col] = base_p(self.info.p, elem as u64, j as u64) as u32;
                }
            }
        }

        // Map DB elems to [-p/2; p/2]
        self.data -= (params.p / 2) as u32;
    }

    pub fn load_data_fast(&mut self, bits_per_entry: u64, params: &Params, data_fname: &str) {
        let raw_data = std::fs::read(data_fname).unwrap();
        let mut iter = raw_data
            .into_iter()
            .flat_map(bits_from_byte)
            .enumerate()
            .peekable();
        self.data = Matrix::new(params.l, params.m);
        println!("allocated");

        if self.info.packing > 0 {
            // Pack multiple DB elems into each Z_p elem
            let mut at = 0;
            let mut cur = 0u32;
            let mut coeff = 1u32;
            while let Some((i, elem)) = iter.next() {
                cur += (elem as u32) * coeff;
                coeff *= 1 << bits_per_entry;
                if ((i + 1) % (self.info.packing as usize) == 0) || (iter.peek().is_none()) {
                    self.data[at / params.m][at % params.m] = cur as u32;
                    at += 1;
                    cur = 0;
                    coeff = 1
                }
            }
        } else {
            // Use multiple Z_p elems to represent each DB elem
            for (i, elem) in iter {
                for j in 0..self.info.ne {
                    // let elem = u64::from_ne_bytes(elem_ref.as_slice().try_into().unwrap());
                    let row = (i / params.m) * (self.info.ne as usize) + (j as usize);
                    let col = i % (params.m as usize);
                    self.data[row][col] = base_p(self.info.p, elem as u64, j as u64) as u32;
                }
            }
        }

        // Map DB elems to [-p/2; p/2]
        self.data -= (params.p / 2) as u32;
    }

    /// Creates a new database, filled data from the given iterator.
    ///
    /// The iterator `data` should yield at least `num_entries` items, each a
    /// value of at most `bits_per_entry` bits.
    pub fn with_data<I: Iterator<Item = u8>>(
        num_entries: u64,
        bits_per_entry: u64,
        params: &Params,
        data: I,
    ) -> Self {
        let mut db = Self::new(num_entries, bits_per_entry, params);
        db.load_data(bits_per_entry, params, data);
        db
    }

    fn _set(&mut self, _: u64, _: u64, _: &Params) {
        todo!("Eventually add updates");
    }

    /// Squishes the database, compressing it for faster processing in-memory.
    pub fn squish(&mut self) {
        self.info.squish_params = SquishParams::default();
        self.info.orig_cols = self.data.cols;
        self.data = self.data.squish(&self.info.squish_params);

        assert!(self.info.p <= (1 << self.info.squish_params.basis));
        assert!(
            self.info.logq >= self.info.squish_params.basis * self.info.squish_params.delta as u64
        );
    }

    /// Unsquishes the database, bringing it back to its state prior to `squish`.
    pub fn unsquish(&mut self) {
        self.data = self
            .data
            .unsquish(&self.info.squish_params, self.info.orig_cols);
    }

    pub fn reconstruct_elem(mut vals: Vec<u64>, index: u64, info: &DbInfo) -> u64 {
        let q = 1 << info.logq;

        for i in 0..vals.len() {
            vals[i] = (vals[i] + info.p / 2) % q;
            vals[i] = vals[i] % info.p;
        }

        let mut val = reconstruct_from_base_p(info.p, &vals);

        println!("val {}", val);

        if info.packing > 0 {
            val = base_p(1 << info.bits_per_entry, val, index % (info.packing as u64));
        }

        return val;
    }

    pub fn get_elem(&self, i: usize) -> u64 {
        assert!(i < self.info.num_entries as usize);

        let mut col = i % self.data.cols;
        let mut row = i / self.data.cols;
        let mut orig_col = 0;

        if self.info.packing > 0 {
            let new_i = i / self.info.packing as usize;
            col = new_i % self.data.cols;
            row = new_i / self.data.cols;
        }

        if self.info.squish_params.delta > 0 && self.info.orig_cols > 0 {
            let new_i = i / self.info.packing as usize;
            col = new_i % self.info.orig_cols;
            row = new_i / self.info.orig_cols;

            orig_col = col;
            col = col / self.info.squish_params.delta;
        }

        let mut vals = Vec::with_capacity(self.info.ne as usize);
        for j in 0..self.info.ne {
            let idx = row * self.info.ne as usize + j as usize;
            let mut val = self.data[idx][col] as u64;

            if self.info.squish_params.delta > 0 && self.info.orig_cols > 0 {
                let delta = self.info.squish_params.delta;
                let basis = self.info.squish_params.basis as usize;
                let k = orig_col % delta;
                val = (val >> (k * basis)) & ((1 << basis) - 1);

                // to account for the p/2 addition reconstruct_elem will do
                val = val.wrapping_sub(self.info.p / 2);
            }

            vals.push(val)
        }

        let result = Self::reconstruct_elem(vals, i as u64, &self.info);
        result
    }
}

/// Computes how many Z_p elements are needed to represent a single Z_q element.
fn compute_num_entries_base_p(p: u64, log_q: u64) -> usize {
    let log_p = (p as f64).log2();
    ((log_q as f64) / log_p).ceil() as usize
}

/// Returns how many Z_p elements are needed to represent a database of `num_entries` entries,
/// each consisting of `bits_per_entry` bits.
fn num_db_entries(num_entries: u64, bits_per_entry: u64, p: u64) -> (usize, usize, usize) {
    if (bits_per_entry as f64) <= (p as f64).log2() {
        // pack multiple DB entries into a single Z_p elem
        let logp = (p as f64).log2() as u64;
        let entries_per_elem = logp / bits_per_entry;
        let db_entries = ((num_entries as f64) / (entries_per_elem as f64)).ceil() as u64;
        assert!(db_entries > 0 && db_entries <= (num_entries as u64));
        return (db_entries as usize, 1, entries_per_elem as usize);
    }

    // use multiple Z_p elems to represent a single DB entry
    let ne = compute_num_entries_base_p(p, bits_per_entry);
    return (num_entries as usize * ne, ne, 0);
}

/// Find smallest l, m such that l*m >= num_entries*ne and ne divides l, where ne is
/// the number of Z_p elements per DB entry determined by bits_per_entry and p.
pub fn approx_square_database_dims(
    num_entries: u64,
    bits_per_entry: u64,
    p: u64,
) -> (usize, usize) {
    let (db_elems, elems_per_entry, _) = num_db_entries(num_entries, bits_per_entry, p);
    let mut l = (db_elems as f64).sqrt().floor() as usize;

    let rem = l % elems_per_entry;
    if rem != 0 {
        l += elems_per_entry - rem;
    }

    let m = ((db_elems as f64) / (l as f64)).ceil() as usize;

    (l, m)
}

/// Find smallest l, m such that l*m >= num_entries*ne and ne divides l, where ne is
/// the number of Z_p elements per DB entry determined by bits_per_entry and p,
/// and m >= lower_bound_m.
pub fn approx_database_dims(
    num_entries: u64,
    bits_per_entry: u64,
    p: u64,
    lower_bound_m: usize,
) -> (usize, usize) {
    let (l, m) = approx_square_database_dims(num_entries, bits_per_entry, p);
    if m >= lower_bound_m {
        return (l, m);
    }

    let m = lower_bound_m;
    let (db_elems, elems_per_entry, _) = num_db_entries(num_entries, bits_per_entry, p);
    let mut l = ((db_elems as f64) / (m as f64)).ceil() as usize;

    let rem = l % elems_per_entry;
    if rem != 0 {
        l += elems_per_entry - rem
    }

    (l, m)
}
