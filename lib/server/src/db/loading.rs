use std::cell::RefCell;
use std::convert::TryInto;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::time::Instant;

use rand::thread_rng;
use rand::Rng;
use rand::RngCore;
use rand::SeedableRng;
use spiral_rs::arith::*;
use spiral_rs::params::*;
use spiral_rs::poly::*;
use spiral_rs::util::*;

use rayon::prelude::*;

use crate::compute::dot_product::*;
use crate::db::aligned_memory::*;
use crate::error::Error;

use super::sparse_db::SparseDb;

pub fn get_inv_idx(params: &Params, item_idx: usize) -> usize {
    let row = item_idx / (1 << params.db_dim_2);
    let col = item_idx % (1 << params.db_dim_2);
    let rows = 1 << params.db_dim_1;
    let inv_idx = col * rows + row;
    inv_idx
}

pub fn pack_ntt_poly(poly: &PolyMatrixNTT) -> Vec<u64> {
    let mut v = vec![0u64; poly.params.poly_len];
    for z in 0..poly.params.poly_len {
        v[z] = poly.data[z]
            | (poly.data[poly.params.poly_len + z] << crate::compute::dot_product::PACKED_OFFSET_2);
    }
    v
}

pub fn pack_ntt_poly_inplace(poly: &PolyMatrixNTT, out: &mut [u64]) {
    for z in 0..poly.params.poly_len {
        out[z] = poly.data[z]
            | (poly.data[poly.params.poly_len + z] << crate::compute::dot_product::PACKED_OFFSET_2);
    }
}

pub fn generate_random_db_and_get_item<'a>(
    params: &'a Params,
    item_idx: usize,
) -> (PolyMatrixRaw<'a>, AlignedMemory64) {
    let mut rng = get_seeded_rng();

    let instances = params.instances;
    let trials = params.n * params.n;
    let dim0 = 1 << params.db_dim_1;
    let num_per = 1 << params.db_dim_2;
    let num_items = dim0 * num_per;
    let db_size_words = instances * trials * num_items * params.poly_len;
    let mut v = AlignedMemory64::new(db_size_words);

    let mut item = PolyMatrixRaw::zero(params, params.instances * params.n, params.n);

    for instance in 0..instances {
        for trial in 0..trials {
            for i in 0..num_items {
                let ii = i % num_per;
                let j = i / num_per;

                let mut db_item = PolyMatrixRaw::random_rng(params, 1, 1, &mut rng);
                db_item.reduce_mod(params.pt_modulus);

                if i == item_idx {
                    item.copy_into(
                        &db_item,
                        instance * params.n + trial / params.n,
                        trial % params.n,
                    );
                }

                for z in 0..params.poly_len {
                    db_item.data[z] =
                        recenter_mod(db_item.data[z], params.pt_modulus, params.modulus);
                }

                let db_item_ntt = db_item.ntt();
                for z in 0..params.poly_len {
                    let idx_dst = calc_index(
                        &[instance, trial, z, ii, j],
                        &[instances, trials, params.poly_len, num_per, dim0],
                    );

                    v[idx_dst] = db_item_ntt.data[z]
                        | (db_item_ntt.data[params.poly_len + z] << PACKED_OFFSET_2);
                }
            }
        }
    }
    (item, v)
}

pub fn generate_fake_sparse_db_and_get_item<'a>(
    params: &'a Params,
    item_idx: usize,
    dummy_items: usize,
) -> (PolyMatrixRaw<'a>, SparseDb) {
    let instances = params.instances;
    let trials = params.n * params.n;
    let update_req_sz = 4 + instances * trials * params.bytes_per_chunk();

    let inst_trials = params.instances * params.n * params.n;
    let db_row_size = params.poly_len * inst_trials * std::mem::size_of::<u64>();
    let mut db = SparseDb::new(None, db_row_size, params.num_items(), None);

    let mut rng = thread_rng();
    let mut corr_db_item =
        PolyMatrixRaw::random_rng(params, params.instances * params.n, params.n, &mut rng);
    corr_db_item.reduce_mod(params.pt_modulus);
    for i in (update_req_sz - 4)..corr_db_item.data.len() {
        corr_db_item.data[i] = 0;
    }
    let corr_bytes: Vec<u8> = corr_db_item
        .data
        .as_slice()
        .iter()
        .map(|x| *x as u8)
        .collect();

    let dummy_row_indices = if dummy_items >= params.num_items() {
        (0..params.num_items()).collect::<Vec<_>>()
    } else {
        // warn: collisions mean that the database will be sparser than expected
        (0..dummy_items)
            .map(|_| rng.gen::<usize>() % params.num_items())
            .collect::<Vec<_>>()
    };

    let (total_rng_time, total_preprocess_time, total_upsert_time) = dummy_row_indices
        .par_iter()
        .map(|&dest_idx| {
            let stamp = Instant::now();
            let mut drng = rand::rngs::SmallRng::seed_from_u64(dest_idx as u64);
            let mut update_req = vec![0u8; update_req_sz];
            for byte in &mut update_req[4..] {
                *byte = drng.gen();
            }
            let rng_time = stamp.elapsed().as_micros() as u64;

            let stamp = Instant::now();
            let row_data = preprocess_item(params, &update_req[4..]);
            let preprocess_time = stamp.elapsed().as_micros() as u64;

            let stamp = Instant::now();
            db.upsert(dest_idx, &row_data);
            let upsert_time = stamp.elapsed().as_micros() as u64;

            (rng_time, preprocess_time, upsert_time)
        })
        .reduce(|| (0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2));

    println!(
        "RNG: {} ms\nPreprocess: {} ms\nUpsert: {} ms",
        total_rng_time / 1000,
        total_preprocess_time / 1000,
        total_upsert_time / 1000
    );
    // inject target item
    let mut update_req = vec![0u8; update_req_sz];
    (&mut update_req[4..]).copy_from_slice(&corr_bytes[..update_req_sz - 4]);
    update_req[0..4].copy_from_slice(&(item_idx as u32).to_be_bytes());
    update_item(params, &update_req, &mut db).unwrap();

    (corr_db_item, db)
}

pub fn load_item_from_seek<'a, T: Seek + Read + Send + Sync>(
    params: &'a Params,
    seekable: &mut T,
    instance: usize,
    trial: usize,
    item_idx: usize,
) -> PolyMatrixRaw<'a> {
    let db_item_size = params.db_item_size;
    let instances = params.instances;
    let trials = params.n * params.n;

    let chunks = instances * trials;
    let bytes_per_chunk = f64::ceil(db_item_size as f64 / chunks as f64) as usize;
    let logp = f64::ceil(f64::log2(params.pt_modulus as f64)) as usize;
    let modp_words_per_chunk = f64::ceil((bytes_per_chunk * 8) as f64 / logp as f64) as usize;
    assert!(modp_words_per_chunk <= params.poly_len);

    let idx_item_in_file = item_idx * db_item_size;
    let idx_chunk = instance * trials + trial;
    let idx_poly_in_file = idx_item_in_file + idx_chunk * bytes_per_chunk;

    let mut out = PolyMatrixRaw::zero(params, 1, 1);

    let seek_result = seekable.seek(SeekFrom::Start(idx_poly_in_file as u64));
    if seek_result.is_err() {
        return out;
    }
    let mut data = vec![0u8; 2 * bytes_per_chunk];
    let bytes_read = seekable
        .read(&mut data.as_mut_slice()[0..bytes_per_chunk])
        .unwrap();

    let modp_words_read = f64::ceil((bytes_read * 8) as f64 / logp as f64) as usize;
    assert!(modp_words_read <= params.poly_len);

    for i in 0..modp_words_read {
        out.data[i] = read_arbitrary_bits(&data, i * logp, logp);
        assert!(out.data[i] <= params.pt_modulus);
    }

    out
}

pub fn load_db_from_seek(params: &Params, fname: &String) -> AlignedMemory64 {
    let instances = params.instances;
    let trials = params.n * params.n;
    let dim0 = 1 << params.db_dim_1;
    let num_per = 1 << params.db_dim_2;
    let num_items = dim0 * num_per;
    let db_size_words = instances * trials * num_items * params.poly_len;
    let v = AlignedMemory64::new(db_size_words);

    let total_idx = instances * trials * num_items;

    thread_local!(static STORE: RefCell<Option<File>> = RefCell::new(None));

    (0..total_idx).into_par_iter().for_each(|idx| {
        STORE.with(|cell| {
            let mut local_store = cell.borrow_mut();
            if local_store.is_none() {
                *local_store = Some(File::open(fname).unwrap());
            }

            let mut indices = [0, 0, 0];
            decompose_index(&mut indices, idx, &[instances, trials, num_items]);
            let instance = indices[0];
            let trial = indices[1];
            let i = indices[2];

            let ii = i % num_per;
            let j = i / num_per;

            let mut file = File::open(fname).unwrap();

            let mut db_item = load_item_from_seek(params, &mut file, instance, trial, i);
            // db_item.reduce_mod(params.pt_modulus);

            for z in 0..params.poly_len {
                db_item.data[z] = recenter_mod(db_item.data[z], params.pt_modulus, params.modulus);
            }

            let db_item_ntt = db_item.ntt();
            for z in 0..params.poly_len {
                let idx_dst = calc_index(
                    &[instance, trial, z, ii, j],
                    &[instances, trials, params.poly_len, num_per, dim0],
                );

                let val = db_item_ntt.data[z]
                    | (db_item_ntt.data[params.poly_len + z] << PACKED_OFFSET_2);

                unsafe {
                    *(v.as_ptr() as *mut u64).offset(idx_dst as isize) = val;
                }
            }
        });
    });
    v
}

pub fn load_file_unsafe(data: &mut [u64], file: &mut File) {
    let data_as_u8_mut = unsafe { data.align_to_mut::<u8>().1 };
    file.read_exact(data_as_u8_mut).unwrap();
}

pub fn load_file(data: &mut [u64], file: &mut File) {
    let mut reader = BufReader::with_capacity(1 << 24, file);
    let mut buf = [0u8; 8];
    for i in 0..data.len() {
        reader.read(&mut buf).unwrap();
        data[i] = u64::from_ne_bytes(buf);
    }
}

pub fn load_preprocessed_db_from_file(params: &Params, file: &mut File) -> AlignedMemory64 {
    let instances = params.instances;
    let trials = params.n * params.n;
    let dim0 = 1 << params.db_dim_1;
    let num_per = 1 << params.db_dim_2;
    let num_items = dim0 * num_per;
    let db_size_words = instances * trials * num_items * params.poly_len;
    let mut v = AlignedMemory64::new(db_size_words);
    let v_mut_slice = v.as_mut_slice();

    load_file(v_mut_slice, file);

    v
}

pub fn convert_pt_to_poly<'a>(params: &'a Params, data: &[u8]) -> PolyMatrixNTT<'a> {
    let logp = f64::ceil(f64::log2(params.pt_modulus as f64)) as usize;
    let modp_words_per_chunk = params.poly_len; //params.modp_words_per_chunk();
    assert!(modp_words_per_chunk <= params.poly_len);

    let mut item = PolyMatrixRaw::zero(params, 1, 1);

    let modp_words_read = f64::ceil((data.len() * 8) as f64 / logp as f64) as usize;
    assert!(modp_words_read <= params.poly_len);

    // optimization for logp = 8
    assert_eq!(logp, 8);

    for i in 0..modp_words_read {
        item.data[i] = data[i] as u64;
        // item.data[i] = read_arbitrary_bits(&data, i * logp, logp) % params.pt_modulus;
        assert!(item.data[i] < params.pt_modulus);
        item.data[i] = recenter_mod(item.data[i], params.pt_modulus, params.modulus);
    }

    item.ntt()
}

pub fn update_item(params: &Params, body: &[u8], db: &SparseDb) -> Result<u64, Error> {
    let instances = params.instances;
    let trials = params.n * params.n;

    let pt_data_len = params.bytes_per_chunk();
    let max_update_len = 4 + instances * trials * pt_data_len;

    if body.len() > max_update_len {
        return Err(Error::InvalidLength(body.len(), max_update_len));
    }

    let db_idx = u32::from_be_bytes(body[..4].try_into().unwrap()) as usize;

    update_item_raw(params, db_idx, &body[4..], db)
}

pub fn preprocess_item(params: &Params, data: &[u8]) -> Vec<u64> {
    let instances = params.instances;
    let trials = params.n * params.n;
    let pt_data_len = params.bytes_per_chunk();

    let mut new_bucket = vec![0u8; instances * trials * pt_data_len];
    new_bucket[..data.len()].copy_from_slice(&data);
    let inp = new_bucket.as_slice();

    assert_eq!(inp.len() % pt_data_len, 0);

    let results: Vec<_> = inp
        .par_chunks_exact(pt_data_len)
        .map(|pt_data| {
            let ntt = convert_pt_to_poly(params, pt_data);
            pack_ntt_poly(&ntt)
        })
        .collect();

    let concatenated_results: Vec<u64> = results.iter().flat_map(|result| result.clone()).collect();
    concatenated_results
}

pub fn update_item_raw(
    params: &Params,
    db_idx: usize,
    data: &[u8],
    db: &SparseDb,
) -> Result<u64, Error> {
    if db_idx >= params.num_items() {
        println!(
            "bad db idx {} (expected less than {})",
            db_idx,
            params.num_items()
        );
        return Err(Error::Unknown);
    }

    let now = Instant::now();
    let concatenated_results = preprocess_item(params, data);
    let upsert_time = now.elapsed().as_micros();

    db.upsert(db_idx, &concatenated_results);

    Ok(upsert_time as u64)
}

pub fn update_many_items(params: &Params, body: &[u8], db: &mut SparseDb) -> Result<u64, Error> {
    let mut offs = 0;
    let mut largest_update = 0;

    while offs < body.len() {
        let chunk_len = u32::from_be_bytes(body[offs..offs + 4].try_into().unwrap()) as usize;
        let data = &body[offs + 4..offs + 4 + chunk_len];
        if data.len() > largest_update {
            largest_update = data.len();
        }
        update_item(params, data, db)?;

        offs += 4 + chunk_len;
    }

    Ok(largest_update as u64)
}
