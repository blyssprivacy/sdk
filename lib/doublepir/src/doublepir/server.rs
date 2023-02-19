use crate::{
    database::*,
    doublepir::*,
    matrix::Matrix,
    params::Params,
    pir::*,
    serializer::{Deserialize, DeserializeSlice, Serialize, State},
};

use std::{
    fmt::Debug,
    fs::{self, File},
    io::Write,
    slice,
    time::Instant,
};

pub struct DoublePirServer {
    num_entries: u64,
    bits_per_entry: usize,
    params: Params,
    shared_state: State,
    db: Db,
    pub server_state: State,
    hint: State,
    pub adjustments: Vec<u32>,
}

impl DoublePirServer {
    pub fn db_ref(&self) -> &Db {
        &self.db
    }

    pub fn server_state_ref(&self) -> &State {
        &self.server_state
    }

    pub fn hint_ref(&self) -> &State {
        &self.hint
    }

    pub fn params_ref(&self) -> &Params {
        &self.params
    }

    pub fn dbinfo_ref(&self) -> &DbInfo {
        &self.db.info
    }

    pub fn get_file_names(fname_base: &str) -> (String, String, String, String, String, String) {
        (
            format!("{}.hint", &fname_base),
            format!("{}.state", &fname_base),
            format!("{}.dbp", &fname_base),
            format!("{}.dbinfo", &fname_base),
            format!("{}.params", &fname_base),
            format!("{}.txt", &fname_base),
        )
    }

    pub fn restore_from_files(
        &mut self,
        fname_base: &str,
        load_server_state: bool,
        load_db_data: bool,
    ) {
        let (hint_fname, server_state_fname, db_fname, dbinfo_fname, _params_fname, txt_fname) =
            Self::get_file_names(fname_base);

        println!("1");
        std::io::stdout().flush().unwrap();

        self.hint = State::deserialize(&std::fs::read(hint_fname).unwrap());
        println!("2");
        std::io::stdout().flush().unwrap();

        if load_server_state {
            self.server_state = State::deserialize_iter(
                &mut std::fs::read(server_state_fname).unwrap().into_iter(),
            );
        }

        println!("3");
        std::io::stdout().flush().unwrap();

        let dbinfo = DbInfo::deserialize(&std::fs::read(dbinfo_fname).unwrap());
        println!("4");
        std::io::stdout().flush().unwrap();

        let raw_data = if load_db_data {
            // let f = File::open(db_fname).unwrap();
            // let mut reader = BufReader::new(f);
            // let mut b: [u8; 4] = unsafe { mem::uninitialized() };
            // reader.bytes().map.collect()
            let start = Instant::now();
            let val = fs::read(db_fname).unwrap();
            println!("load took: {} us", start.elapsed().as_micros());
            val
        } else {
            Vec::new()
        };
        println!("5");
        std::io::stdout().flush().unwrap();

        let txt_bytes = std::fs::read(txt_fname).unwrap();
        println!("6");
        std::io::stdout().flush().unwrap();

        let txt_val = std::str::from_utf8(&txt_bytes).unwrap();
        println!("7");
        std::io::stdout().flush().unwrap();

        let parts: Vec<&str> = txt_val.split(",").collect();
        let db_rows = parts[0].parse::<usize>().unwrap();
        let db_cols = parts[1].parse::<usize>().unwrap();
        println!("8");
        std::io::stdout().flush().unwrap();

        // don't want to do a copy here
        self.db = Db {
            info: dbinfo,
            data: Matrix::new(1, 1),
            db_rows,
            db_cols,
            raw_data,
        }
    }

    pub fn save_to_files(&mut self, fname_base: &str) {
        let (hint_fname, server_state_fname, db_fname, dbinfo_fname, params_fname, txt_fname) =
            Self::get_file_names(fname_base);

        let mut out_file = File::create(&hint_fname).unwrap();
        out_file.write_all(&self.hint.serialize()).unwrap();

        let mut out_file = File::create(&server_state_fname).unwrap();
        out_file.write_all(&self.server_state.serialize()).unwrap();

        let mut out_file = File::create(&dbinfo_fname).unwrap();
        out_file.write_all(&self.db.info.serialize()).unwrap();

        let mut out_file = File::create(&params_fname).unwrap();
        out_file
            .write_all(&self.params.to_string().as_bytes())
            .unwrap();

        let mut out_file = File::create(&db_fname).unwrap();
        let data = unsafe {
            let ptr = self.db.data.data.as_ptr() as *const u8;
            let slice: &[u8] = slice::from_raw_parts(ptr, self.db.data.data.len() * 4);
            slice
        };
        out_file.write_all(&data).unwrap();

        let mut out_file = File::create(&txt_fname).unwrap();
        write!(out_file, "{},{}", self.db.data.rows, self.db.data.cols).unwrap();
        out_file.flush().unwrap();
    }

    pub fn load_data_fast(&mut self, data_fname: &str) {
        self.db
            .load_data_fast(self.bits_per_entry as u64, &self.params, data_fname);
        println!("loaded!");
        (self.server_state, self.hint) = setup(&mut self.db, &self.shared_state, &self.params);
    }

    pub fn answer_inline(&self, query: &[u8], data: &[u32], chunk_idx: Option<usize>) -> Vec<u8> {
        let query_data = Vec::<State>::deserialize(query);
        let response = answer(
            &self.db,
            &query_data,
            &self.server_state,
            &self.shared_state,
            &self.params,
            Some(data),
            chunk_idx,
        );
        println!("response of len: {}", response.len());
        response.serialize()
    }

    pub fn generate_adjustments(params: &Params, shared_state: &State) -> Vec<u32> {
        let mut out = Vec::new();
        let ratio = params.p / 2;
        let a_2 = &shared_state[1];
        for j1 in 0..params.n {
            let mut val3 = 0u64;
            for j2 in 0..a_2.rows {
                val3 += ratio * (a_2[j2][j1] as u64);
            }
            val3 %= 1 << params.logq;
            val3 = (1 << params.logq) - val3;

            let v = val3 as u32;
            out.push(v);
        }
        out
    }
}

impl PirServer for DoublePirServer {
    fn new(num_entries: u64, bits_per_entry: usize) -> Self {
        let bits_per_entry_u64 = bits_per_entry as u64;
        println!("picking");
        let params = pick_params(num_entries, bits_per_entry_u64, SEC_PARAM, LOGQ);
        println!("picked");
        let db = Db::new(num_entries, bits_per_entry_u64, &params);
        let shared_state = init(&db.info, &params);
        let server_state = Vec::new();
        let hint = Vec::new();
        let adjustments = Self::generate_adjustments(&params, &shared_state);
        Self {
            num_entries,
            bits_per_entry,
            params,
            db,
            shared_state,
            server_state,
            hint,
            adjustments,
        }
    }

    fn load_data<I: Iterator<Item = u8>>(&mut self, data: I) {
        self.db
            .load_data(self.bits_per_entry as u64, &self.params, data);
        println!("loaded!");
        (self.server_state, self.hint) = setup(&mut self.db, &self.shared_state, &self.params);
    }

    fn get_hint(&self) -> Vec<u8> {
        self.hint.serialize()
    }

    fn answer(&self, query: &[u8]) -> Vec<u8> {
        let query_data = Vec::<State>::deserialize(query);
        let response = answer(
            &self.db,
            &query_data,
            &self.server_state,
            &self.shared_state,
            &self.params,
            None,
            None,
        );
        response.serialize()
    }
}

impl Debug for DoublePirServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoublePirServer")
            .field("num_entries", &self.num_entries)
            .field("bits_per_entry", &self.bits_per_entry)
            .field("params", &self.params)
            .finish()
    }
}
