use rand::{thread_rng, Rng};

use crate::{
    database::*,
    params::Params,
    pir::*,
    serializer::{DeserializeSlice, Serialize, State},
};

use crate::doublepir;

use std::{fmt::Debug, future::Future};

pub struct DoublePirClient {
    num_entries: u64,
    bits_per_entry: usize,
    params: Params,
    shared_state: State,
    db_info: DbInfo,
    hint: State,
}

impl PirClient for DoublePirClient {
    fn new(num_entries: u64, bits_per_entry: usize) -> Self {
        let bits_per_entry_u64 = bits_per_entry as u64;
        let params = doublepir::pick_params(
            num_entries,
            bits_per_entry_u64,
            doublepir::SEC_PARAM,
            doublepir::LOGQ,
        );
        let db_info = Db::new(num_entries, bits_per_entry_u64, &params).info;
        let shared_state = doublepir::init(&db_info, &params);
        let hint = State::new();
        Self {
            num_entries,
            bits_per_entry,
            params,
            shared_state,
            db_info,
            hint,
        }
    }

    fn load_hint(&mut self, hint: &[u8]) {
        self.hint = State::deserialize(hint);
    }

    fn generate_query(&self, index: u64) -> (Vec<u8>, Vec<u8>) {
        let (client_state, query_data) =
            doublepir::query(index, &self.shared_state, &self.params, &self.db_info);

        (
            query_data.serialize(),
            vec![client_state, query_data].serialize(),
        )
    }

    fn decode_response(&self, response: &[u8], index: u64, client_query_data: &[u8]) -> Vec<u8> {
        let answer = State::deserialize(response);
        let query_state = Vec::<State>::deserialize(client_query_data);
        let (client_state, query) = (&query_state[0], &query_state[1]);
        let result = doublepir::recover(
            index,
            0,
            &self.hint,
            &query,
            &answer,
            &self.shared_state,
            &client_state,
            &self.params,
            &self.db_info,
        );
        result.to_ne_bytes().to_vec()
    }
}

impl DoublePirClient {
    pub fn with_params(params: &Params, db_info: &DbInfo) -> Self {
        let shared_state = doublepir::init(db_info, params);
        let hint = State::new();
        Self {
            num_entries: db_info.num_entries,
            bits_per_entry: db_info.bits_per_entry as usize,
            params: *params,
            shared_state,
            db_info: *db_info,
            hint,
        }
    }

    pub async fn with_params_derive_fast<T, Fut>(
        params: &Params,
        db_info: &DbInfo,
        derive: fn(&[u8; 16], u64, &mut [u8]) -> Fut,
    ) -> Self
    where
        Fut: Future<Output = T>,
        T: Sized,
    {
        let shared_state = doublepir::init_derive_fast(db_info, params, derive).await;
        let hint = State::new();
        Self {
            num_entries: db_info.num_entries,
            bits_per_entry: db_info.bits_per_entry as usize,
            params: *params,
            shared_state,
            db_info: *db_info,
            hint,
        }
    }

    pub fn load_hint_from_file(&mut self, hint_file_name: &str) {
        self.hint = State::deserialize(&std::fs::read(hint_file_name).unwrap());
    }

    pub fn params_from_file(params_file_name: &str) -> Params {
        let str_bytes = std::fs::read(params_file_name).unwrap();
        Params::from_string(std::str::from_utf8(&str_bytes).unwrap())
    }

    pub fn dbinfo_from_file(dbinfo_file_name: &str) -> DbInfo {
        DbInfo::deserialize(&std::fs::read(dbinfo_file_name).unwrap())
    }

    pub fn num_entries(&self) -> u64 {
        self.num_entries
    }

    pub fn params_ref(&self) -> &Params {
        &self.params
    }

    pub fn dbinfo_ref(&self) -> &DbInfo {
        &self.db_info
    }

    pub fn decode_response_impl(
        &self,
        response: &[u8],
        index: u64,
        query_index: usize,
        client_query_data: &[u8],
    ) -> Vec<u8> {
        let answer = State::deserialize(response);
        let query_state = Vec::<State>::deserialize(client_query_data);
        assert_eq!(query_state.len(), 2);
        let (client_state, query) = (&query_state[0], &query_state[1]);
        let result = doublepir::recover(
            index,
            query_index,
            &self.hint,
            &query,
            &answer,
            &self.shared_state,
            &client_state,
            &self.params,
            &self.db_info,
        );
        result.to_ne_bytes().to_vec()
    }

    pub fn generate_query_batch(
        &self,
        indices: &[u64],
    ) -> (Vec<State>, Vec<Vec<u8>>, Vec<Option<(u64, u64)>>) {
        let params = self.params_ref();
        let dbinfo = self.dbinfo_ref();

        let batch_num = indices.len();
        let batch_sz = params.l / batch_num;
        let batch_sz_words = batch_sz * params.m * dbinfo.packing;
        let mut query_plan = vec![None; batch_num];

        for (query_idx, i) in indices.iter().enumerate() {
            let db_elem = *i / (dbinfo.packing as u64);
            let row = db_elem / (params.m as u64);
            let batch = row / (batch_sz as u64);
            let idx_within_batch = *i;

            println!("gave {} batch {} (row = {})", idx_within_batch, batch, row);

            // assert_eq!((*i) % 2, idx_within_batch % 2);

            let cur_val = query_plan[batch as usize];
            if cur_val.is_some() {
                println!("can't query #{} (batch {} already taken)", query_idx, batch);
            } else {
                query_plan[batch as usize] = Some((*i, idx_within_batch));
            }
        }

        // replace any None's in batch_plan with a random index
        let mut rng = thread_rng();
        let mut target_indices = Vec::<u64>::new();
        for (i, query) in query_plan.iter().enumerate() {
            if let Some((_, target_idx)) = query {
                target_indices.push(*target_idx);
            } else {
                let rand_idx = rng.gen::<u64>() % (batch_sz_words as u64);
                let rand_target_idx = (batch_sz_words as u64) * (i as u64) + rand_idx;
                target_indices.push(rand_target_idx);
            }
        }

        let mut queries: Vec<State> = Vec::new();
        let mut client_states: Vec<Vec<u8>> = Vec::new();
        for target_idx in target_indices {
            let (query, client_state) = self.generate_query(target_idx);
            let query_data = State::deserialize(&query);
            queries.push(query_data);
            client_states.push(client_state);
        }

        (queries, client_states, query_plan)
    }
}

impl Debug for DoublePirClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoublePirClient")
            .field("num_entries", &self.num_entries)
            .field("bits_per_entry", &self.bits_per_entry)
            .field("params", &self.params)
            .finish()
    }
}
