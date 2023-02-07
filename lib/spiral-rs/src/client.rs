use crate::{
    arith::*, discrete_gaussian::*, gadget::*, number_theory::*, params::*, poly::*, util::*,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use std::{iter::once, mem::size_of};
use subtle::ConditionallySelectable;
use subtle::ConstantTimeEq;

pub type Seed = <ChaCha20Rng as SeedableRng>::Seed;
pub const SEED_LENGTH: usize = 32;

pub const DEFAULT_PARAMS: &'static str = r#"
    {"n": 2,
    "nu_1": 10,
    "nu_2": 6,
    "p": 512,
    "q2_bits": 21,
    "s_e": 85.83255142749422,
    "t_gsw": 10,
    "t_conv": 4,
    "t_exp_left": 16,
    "t_exp_right": 56,
    "instances": 11,
    "db_item_size": 100000 }
"#;

const UUID_V4_LEN: usize = 36;

fn new_vec_raw<'a>(
    params: &'a Params,
    num: usize,
    rows: usize,
    cols: usize,
) -> Vec<PolyMatrixRaw<'a>> {
    let mut v = Vec::with_capacity(num);
    for _ in 0..num {
        v.push(PolyMatrixRaw::zero(params, rows, cols));
    }
    v
}

fn get_inv_from_rng(params: &Params, rng: &mut ChaCha20Rng) -> u64 {
    params.modulus - (rng.gen::<u64>() % params.modulus)
}

fn mat_sz_bytes_excl_first_row(a: &PolyMatrixRaw) -> usize {
    (a.rows - 1) * a.cols * a.params.poly_len * size_of::<u64>()
}

fn serialize_polymatrix_for_rng(vec: &mut Vec<u8>, a: &PolyMatrixRaw) {
    let offs = a.cols * a.params.poly_len; // skip the first row
    for i in 0..(a.rows - 1) * a.cols * a.params.poly_len {
        vec.extend_from_slice(&u64::to_ne_bytes(a.data[offs + i]));
    }
}

fn serialize_vec_polymatrix_for_rng(vec: &mut Vec<u8>, a: &Vec<PolyMatrixRaw>) {
    for i in 0..a.len() {
        serialize_polymatrix_for_rng(vec, &a[i]);
    }
}

fn deserialize_polymatrix_rng(a: &mut PolyMatrixRaw, data: &[u8], rng: &mut ChaCha20Rng) -> usize {
    let (first_row, rest) = a
        .data
        .as_mut_slice()
        .split_at_mut(a.cols * a.params.poly_len);
    for i in 0..first_row.len() {
        first_row[i] = get_inv_from_rng(a.params, rng);
    }
    for (i, chunk) in data.chunks(size_of::<u64>()).enumerate() {
        rest[i] = u64::from_ne_bytes(chunk.try_into().unwrap());
    }
    mat_sz_bytes_excl_first_row(a)
}

fn deserialize_vec_polymatrix_rng(
    a: &mut Vec<PolyMatrixRaw>,
    data: &[u8],
    rng: &mut ChaCha20Rng,
) -> usize {
    let mut chunks = data.chunks(mat_sz_bytes_excl_first_row(&a[0]));
    let mut bytes_read = 0;
    for i in 0..a.len() {
        bytes_read += deserialize_polymatrix_rng(&mut a[i], chunks.next().unwrap(), rng);
    }
    bytes_read
}

fn extract_excl_rng_data(v_buf: &[u64]) -> Vec<u64> {
    let mut out = Vec::new();
    for i in 0..v_buf.len() {
        if i % 2 == 1 {
            out.push(v_buf[i]);
        }
    }
    out
}

fn interleave_rng_data(params: &Params, v_buf: &[u64], rng: &mut ChaCha20Rng) -> Vec<u64> {
    let mut out = Vec::new();

    let mut reg_cts = Vec::new();
    for _ in 0..params.num_expanded() {
        let mut sigma = PolyMatrixRaw::zero(&params, 2, 1);
        for z in 0..params.poly_len {
            sigma.data[z] = get_inv_from_rng(params, rng);
        }
        reg_cts.push(sigma.ntt());
    }
    // reorient into server's preferred indexing
    let reg_cts_buf_words = params.num_expanded() * 2 * params.poly_len;
    let mut reg_cts_buf = vec![0u64; reg_cts_buf_words];
    reorient_reg_ciphertexts(params, reg_cts_buf.as_mut_slice(), &reg_cts);

    assert_eq!(reg_cts_buf_words, 2 * v_buf.len());

    for i in 0..v_buf.len() {
        out.push(reg_cts_buf[2 * i]);
        out.push(v_buf[i]);
    }
    out
}

pub struct PublicParameters<'a> {
    pub v_packing: Vec<PolyMatrixNTT<'a>>, // Ws
    pub v_expansion_left: Option<Vec<PolyMatrixNTT<'a>>>,
    pub v_expansion_right: Option<Vec<PolyMatrixNTT<'a>>>,
    pub v_conversion: Option<Vec<PolyMatrixNTT<'a>>>, // V
    pub seed: Option<Seed>,
}

impl<'a> PublicParameters<'a> {
    pub fn init(params: &'a Params) -> Self {
        if params.expand_queries {
            PublicParameters {
                v_packing: Vec::new(),
                v_expansion_left: Some(Vec::new()),
                v_expansion_right: Some(Vec::new()),
                v_conversion: Some(Vec::new()),
                seed: None,
            }
        } else {
            PublicParameters {
                v_packing: Vec::new(),
                v_expansion_left: None,
                v_expansion_right: None,
                v_conversion: None,
                seed: None,
            }
        }
    }

    fn from_ntt_alloc_vec(v: &Vec<PolyMatrixNTT<'a>>) -> Option<Vec<PolyMatrixRaw<'a>>> {
        Some(v.iter().map(from_ntt_alloc).collect())
    }

    fn from_ntt_alloc_opt_vec(
        v: &Option<Vec<PolyMatrixNTT<'a>>>,
    ) -> Option<Vec<PolyMatrixRaw<'a>>> {
        Some(v.as_ref()?.iter().map(from_ntt_alloc).collect())
    }

    fn to_ntt_alloc_vec(v: &Vec<PolyMatrixRaw<'a>>) -> Option<Vec<PolyMatrixNTT<'a>>> {
        Some(v.iter().map(to_ntt_alloc).collect())
    }

    pub fn to_raw(&self) -> Vec<Option<Vec<PolyMatrixRaw>>> {
        vec![
            Self::from_ntt_alloc_vec(&self.v_packing),
            Self::from_ntt_alloc_opt_vec(&self.v_expansion_left),
            Self::from_ntt_alloc_opt_vec(&self.v_expansion_right),
            Self::from_ntt_alloc_opt_vec(&self.v_conversion),
        ]
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        if self.seed.is_some() {
            let seed = self.seed.as_ref().unwrap();
            data.extend(seed);
        }
        for v in self.to_raw().iter() {
            if v.is_some() {
                serialize_vec_polymatrix_for_rng(&mut data, v.as_ref().unwrap());
            }
        }
        data
    }

    pub fn deserialize(params: &'a Params, data: &[u8]) -> Self {
        assert_eq!(params.setup_bytes(), data.len());

        let mut idx = 0;

        let seed = data[0..SEED_LENGTH].try_into().unwrap();
        let mut rng = ChaCha20Rng::from_seed(seed);
        idx += SEED_LENGTH;

        let mut v_packing = new_vec_raw(params, params.n, params.n + 1, params.t_conv);
        idx += deserialize_vec_polymatrix_rng(&mut v_packing, &data[idx..], &mut rng);

        if params.expand_queries {
            let mut v_expansion_left = new_vec_raw(params, params.g(), 2, params.t_exp_left);
            idx += deserialize_vec_polymatrix_rng(&mut v_expansion_left, &data[idx..], &mut rng);

            let mut v_expansion_right =
                new_vec_raw(params, params.stop_round() + 1, 2, params.t_exp_right);
            idx += deserialize_vec_polymatrix_rng(&mut v_expansion_right, &data[idx..], &mut rng);

            let mut v_conversion = new_vec_raw(params, 1, 2, 2 * params.t_conv);
            _ = deserialize_vec_polymatrix_rng(&mut v_conversion, &data[idx..], &mut rng);

            Self {
                v_packing: Self::to_ntt_alloc_vec(&v_packing).unwrap(),
                v_expansion_left: Self::to_ntt_alloc_vec(&v_expansion_left),
                v_expansion_right: Self::to_ntt_alloc_vec(&v_expansion_right),
                v_conversion: Self::to_ntt_alloc_vec(&v_conversion),
                seed: Some(seed),
            }
        } else {
            Self {
                v_packing: Self::to_ntt_alloc_vec(&v_packing).unwrap(),
                v_expansion_left: None,
                v_expansion_right: None,
                v_conversion: None,
                seed: Some(seed),
            }
        }
    }
}

pub struct Query<'a> {
    pub ct: Option<PolyMatrixRaw<'a>>,
    pub v_buf: Option<Vec<u64>>,
    pub v_ct: Option<Vec<PolyMatrixRaw<'a>>>,
    pub seed: Option<Seed>,
}

impl<'a> Query<'a> {
    pub fn empty() -> Self {
        Query {
            ct: None,
            v_ct: None,
            v_buf: None,
            seed: None,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        if self.seed.is_some() {
            let seed = self.seed.as_ref().unwrap();
            data.extend(seed);
        }
        if self.ct.is_some() {
            let ct = self.ct.as_ref().unwrap();
            serialize_polymatrix_for_rng(&mut data, &ct);
        }
        if self.v_buf.is_some() {
            let v_buf = self.v_buf.as_ref().unwrap();
            let v_buf_extracted = extract_excl_rng_data(&v_buf);
            data.extend(v_buf_extracted.iter().map(|x| x.to_ne_bytes()).flatten());
        }
        if self.v_ct.is_some() {
            let v_ct = self.v_ct.as_ref().unwrap();
            for x in v_ct {
                serialize_polymatrix_for_rng(&mut data, x);
            }
        }
        data
    }

    pub fn deserialize(params: &'a Params, mut data: &[u8]) -> Self {
        assert_eq!(params.query_bytes(), data.len());

        let mut out = Query::empty();
        let seed = data[0..SEED_LENGTH].try_into().unwrap();
        out.seed = Some(seed);
        let mut rng = ChaCha20Rng::from_seed(seed);
        data = &data[SEED_LENGTH..];
        if params.expand_queries {
            let mut ct = PolyMatrixRaw::zero(params, 2, 1);
            deserialize_polymatrix_rng(&mut ct, data, &mut rng);
            out.ct = Some(ct);
        } else {
            let v_buf_bytes = params.query_v_buf_bytes();
            let v_buf: Vec<u64> = (&data[..v_buf_bytes])
                .chunks(size_of::<u64>())
                .map(|x| u64::from_ne_bytes(x.try_into().unwrap()))
                .collect();
            let v_buf_interleaved = interleave_rng_data(params, &v_buf, &mut rng);
            out.v_buf = Some(v_buf_interleaved);

            let mut v_ct = new_vec_raw(params, params.db_dim_2, 2, 2 * params.t_gsw);
            deserialize_vec_polymatrix_rng(&mut v_ct, &data[v_buf_bytes..], &mut rng);
            out.v_ct = Some(v_ct);
        }
        out
    }
}

fn matrix_with_identity<'a>(p: &PolyMatrixRaw<'a>) -> PolyMatrixRaw<'a> {
    assert_eq!(p.cols, 1);
    let mut r = PolyMatrixRaw::zero(p.params, p.rows, p.rows + 1);
    r.copy_into(p, 0, 0);
    r.copy_into(&PolyMatrixRaw::identity(p.params, p.rows, p.rows), 0, 1);
    r
}

fn params_with_moduli(params: &Params, moduli: &Vec<u64>) -> Params {
    Params::init(
        params.poly_len,
        moduli,
        params.noise_width,
        params.n,
        params.pt_modulus,
        params.q2_bits,
        params.t_conv,
        params.t_exp_left,
        params.t_exp_right,
        params.t_gsw,
        params.expand_queries,
        params.db_dim_1,
        params.db_dim_2,
        params.instances,
        params.db_item_size,
    )
}

pub struct Client<'a> {
    params: &'a Params,
    sk_gsw: PolyMatrixRaw<'a>,
    sk_reg: PolyMatrixRaw<'a>,
    sk_gsw_full: PolyMatrixRaw<'a>,
    sk_reg_full: PolyMatrixRaw<'a>,
    dg: DiscreteGaussian,
}

impl<'a> Client<'a> {
    pub fn init(params: &'a Params) -> Self {
        let sk_gsw_dims = params.get_sk_gsw();
        let sk_reg_dims = params.get_sk_reg();
        let sk_gsw = PolyMatrixRaw::zero(params, sk_gsw_dims.0, sk_gsw_dims.1);
        let sk_reg = PolyMatrixRaw::zero(params, sk_reg_dims.0, sk_reg_dims.1);
        let sk_gsw_full = matrix_with_identity(&sk_gsw);
        let sk_reg_full = matrix_with_identity(&sk_reg);

        let dg = DiscreteGaussian::init(params.noise_width);

        Self {
            params,
            sk_gsw,
            sk_reg,
            sk_gsw_full,
            sk_reg_full,
            dg,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn get_sk_reg(&self) -> &PolyMatrixRaw<'a> {
        &self.sk_reg
    }

    fn get_fresh_gsw_public_key(
        &self,
        m: usize,
        rng: &mut ChaCha20Rng,
        rng_pub: &mut ChaCha20Rng,
    ) -> PolyMatrixRaw<'a> {
        let params = self.params;
        let n = params.n;

        let a = PolyMatrixRaw::random_rng(params, 1, m, rng_pub);
        let e = PolyMatrixRaw::noise(params, n, m, &self.dg, rng);
        let a_inv = -&a;
        let b_p = &self.sk_gsw.ntt() * &a.ntt();
        let b = &e.ntt() + &b_p;
        let p = stack(&a_inv, &b.raw());
        p
    }

    fn get_regev_sample(
        &self,
        rng: &mut ChaCha20Rng,
        rng_pub: &mut ChaCha20Rng,
    ) -> PolyMatrixNTT<'a> {
        let params = self.params;
        let a = PolyMatrixRaw::random_rng(params, 1, 1, rng_pub);
        let e = PolyMatrixRaw::noise(params, 1, 1, &self.dg, rng);
        let b_p = &self.sk_reg.ntt() * &a.ntt();
        let b = &e.ntt() + &b_p;
        let mut p = PolyMatrixNTT::zero(params, 2, 1);
        p.copy_into(&(-&a).ntt(), 0, 0);
        p.copy_into(&b, 1, 0);
        p
    }

    fn get_fresh_reg_public_key(
        &self,
        m: usize,
        rng: &mut ChaCha20Rng,
        rng_pub: &mut ChaCha20Rng,
    ) -> PolyMatrixNTT<'a> {
        let params = self.params;

        let mut p = PolyMatrixNTT::zero(params, 2, m);

        for i in 0..m {
            p.copy_into(&self.get_regev_sample(rng, rng_pub), 0, i);
        }
        p
    }

    fn encrypt_matrix_gsw(
        &self,
        ag: &PolyMatrixNTT<'a>,
        rng: &mut ChaCha20Rng,
        rng_pub: &mut ChaCha20Rng,
    ) -> PolyMatrixNTT<'a> {
        let mx = ag.cols;
        let p = self.get_fresh_gsw_public_key(mx, rng, rng_pub);
        let res = &(p.ntt()) + &(ag.pad_top(1));
        res
    }

    pub fn encrypt_matrix_reg(
        &self,
        a: &PolyMatrixNTT<'a>,
        rng: &mut ChaCha20Rng,
        rng_pub: &mut ChaCha20Rng,
    ) -> PolyMatrixNTT<'a> {
        let m = a.cols;
        let p = self.get_fresh_reg_public_key(m, rng, rng_pub);
        &p + &a.pad_top(1)
    }

    pub fn decrypt_matrix_reg(&self, a: &PolyMatrixNTT<'a>) -> PolyMatrixNTT<'a> {
        &self.sk_reg_full.ntt() * a
    }

    pub fn decrypt_matrix_gsw(&self, a: &PolyMatrixNTT<'a>) -> PolyMatrixNTT<'a> {
        &self.sk_gsw_full.ntt() * a
    }

    fn generate_expansion_params(
        &self,
        num_exp: usize,
        m_exp: usize,
        rng: &mut ChaCha20Rng,
        rng_pub: &mut ChaCha20Rng,
    ) -> Vec<PolyMatrixNTT<'a>> {
        let params = self.params;
        let g_exp = build_gadget(params, 1, m_exp);
        let g_exp_ntt = g_exp.ntt();
        let mut res = Vec::new();

        for i in 0..num_exp {
            let t = (params.poly_len / (1 << i)) + 1;
            let tau_sk_reg = automorph_alloc(&self.sk_reg, t);
            let prod = &tau_sk_reg.ntt() * &g_exp_ntt;
            let w_exp_i = self.encrypt_matrix_reg(&prod, rng, rng_pub);
            res.push(w_exp_i);
        }
        res
    }

    pub fn generate_keys_from_seed(&mut self, seed: Seed) -> PublicParameters<'a> {
        self.generate_keys_impl(&mut ChaCha20Rng::from_seed(seed))
    }

    pub fn generate_keys(&mut self) -> PublicParameters<'a> {
        self.generate_keys_impl(&mut ChaCha20Rng::from_entropy())
    }

    pub fn generate_secret_keys_from_seed(&mut self, seed: Seed) {
        self.generate_secret_keys_impl(&mut ChaCha20Rng::from_seed(seed))
    }

    pub fn generate_secret_keys(&mut self) {
        self.generate_secret_keys_impl(&mut ChaCha20Rng::from_entropy())
    }

    pub fn generate_keys_optional(
        &mut self,
        seed: Seed,
        generate_pub_params: bool,
    ) -> Option<Vec<u8>> {
        if generate_pub_params {
            Some(self.generate_keys_from_seed(seed).serialize())
        } else {
            self.generate_secret_keys_from_seed(seed);
            None
        }
    }

    fn generate_secret_keys_impl(&mut self, rng: &mut ChaCha20Rng) {
        self.dg.sample_matrix(&mut self.sk_gsw, rng);
        self.dg.sample_matrix(&mut self.sk_reg, rng);
        self.sk_gsw_full = matrix_with_identity(&self.sk_gsw);
        self.sk_reg_full = matrix_with_identity(&self.sk_reg);
    }

    fn generate_keys_impl(&mut self, rng: &mut ChaCha20Rng) -> PublicParameters<'a> {
        let params = self.params;

        self.generate_secret_keys_impl(rng);
        let sk_reg_ntt = to_ntt_alloc(&self.sk_reg);

        let mut rng = ChaCha20Rng::from_entropy();
        let mut pp = PublicParameters::init(params);
        let pp_seed = rng.gen();
        pp.seed = Some(pp_seed);
        let mut rng_pub = ChaCha20Rng::from_seed(pp_seed);

        // Params for packing
        let gadget_conv = build_gadget(params, 1, params.t_conv);
        let gadget_conv_ntt = to_ntt_alloc(&gadget_conv);
        for i in 0..params.n {
            let scaled = scalar_multiply_alloc(&sk_reg_ntt, &gadget_conv_ntt);
            let mut ag = PolyMatrixNTT::zero(params, params.n, params.t_conv);
            ag.copy_into(&scaled, i, 0);
            let w = self.encrypt_matrix_gsw(&ag, &mut rng, &mut rng_pub);
            pp.v_packing.push(w);
        }

        if params.expand_queries {
            // Params for expansion
            pp.v_expansion_left = Some(self.generate_expansion_params(
                params.g(),
                params.t_exp_left,
                &mut rng,
                &mut rng_pub,
            ));
            pp.v_expansion_right = Some(self.generate_expansion_params(
                params.stop_round() + 1,
                params.t_exp_right,
                &mut rng,
                &mut rng_pub,
            ));

            // Params for converison
            let g_conv = build_gadget(params, 2, 2 * params.t_conv);
            let sk_reg_ntt = self.sk_reg.ntt();
            let sk_reg_squared_ntt = &sk_reg_ntt * &sk_reg_ntt;
            pp.v_conversion = Some(Vec::from_iter(once(PolyMatrixNTT::zero(
                params,
                2,
                2 * params.t_conv,
            ))));
            for i in 0..2 * params.t_conv {
                let sigma;
                if i % 2 == 0 {
                    let val = g_conv.get_poly(0, i)[0];
                    sigma = &sk_reg_squared_ntt * &single_poly(params, val).ntt();
                } else {
                    let val = g_conv.get_poly(1, i)[0];
                    sigma = &sk_reg_ntt * &single_poly(params, val).ntt();
                }
                let ct = self.encrypt_matrix_reg(&sigma, &mut rng, &mut rng_pub);
                pp.v_conversion.as_mut().unwrap()[0].copy_into(&ct, 0, i);
            }
        }

        pp
    }

    pub fn generate_query(&self, idx_target: usize) -> Query<'a> {
        let params = self.params;
        let further_dims = params.db_dim_2;
        let idx_dim0 = idx_target / (1 << further_dims);
        let idx_further = idx_target % (1 << further_dims);
        let scale_k = params.modulus / params.pt_modulus;
        let bits_per = get_bits_per(params, params.t_gsw);

        let mut rng = ChaCha20Rng::from_entropy();

        let mut query = Query::empty();
        let query_seed = ChaCha20Rng::from_entropy().gen();
        query.seed = Some(query_seed);
        let mut rng_pub = ChaCha20Rng::from_seed(query_seed);
        if params.expand_queries {
            // pack query into single ciphertext
            let mut sigma = PolyMatrixRaw::zero(params, 1, 1);
            let inv_2_g_first = invert_uint_mod(1 << params.g(), params.modulus).unwrap();
            let inv_2_g_rest =
                invert_uint_mod(1 << (params.stop_round() + 1), params.modulus).unwrap();

            if params.db_dim_2 == 0 {
                for i in 0..(1 << params.db_dim_1) {
                    sigma.data[i].conditional_assign(&scale_k, (i as u64).ct_eq(&(idx_dim0 as u64)))
                }

                for i in 0..params.poly_len {
                    sigma.data[i] = multiply_uint_mod(sigma.data[i], inv_2_g_first, params.modulus);
                }
            } else {
                for i in 0..(1 << params.db_dim_1) {
                    sigma.data[2 * i]
                        .conditional_assign(&scale_k, (i as u64).ct_eq(&(idx_dim0 as u64)))
                }

                for i in 0..further_dims as u64 {
                    let mask = 1 << i;
                    let bit = ((idx_further as u64) & mask).ct_eq(&mask);
                    for j in 0..params.t_gsw {
                        let val = u64::conditional_select(&0, &(1u64 << (bits_per * j)), bit);
                        let idx = (i as usize) * params.t_gsw + (j as usize);
                        sigma.data[2 * idx + 1] = val;
                    }
                }

                for i in 0..params.poly_len / 2 {
                    sigma.data[2 * i] =
                        multiply_uint_mod(sigma.data[2 * i], inv_2_g_first, params.modulus);
                    sigma.data[2 * i + 1] =
                        multiply_uint_mod(sigma.data[2 * i + 1], inv_2_g_rest, params.modulus);
                }
            }

            query.ct = Some(from_ntt_alloc(&self.encrypt_matrix_reg(
                &to_ntt_alloc(&sigma),
                &mut rng,
                &mut rng_pub,
            )));
        } else {
            let num_expanded = 1 << params.db_dim_1;
            let mut sigma_v = Vec::<PolyMatrixNTT>::new();

            // generate regev ciphertexts
            let reg_cts_buf_words = num_expanded * 2 * params.poly_len;
            let mut reg_cts_buf = vec![0u64; reg_cts_buf_words];
            let mut reg_cts = Vec::<PolyMatrixNTT>::new();
            for i in 0..num_expanded {
                let value = ((i == idx_dim0) as u64) * scale_k;
                let sigma = PolyMatrixRaw::single_value(&params, value);
                reg_cts.push(self.encrypt_matrix_reg(
                    &to_ntt_alloc(&sigma),
                    &mut rng,
                    &mut rng_pub,
                ));
            }
            // reorient into server's preferred indexing
            reorient_reg_ciphertexts(self.params, reg_cts_buf.as_mut_slice(), &reg_cts);

            // generate GSW ciphertexts
            for i in 0..further_dims {
                let bit = ((idx_further as u64) & (1 << (i as u64))) >> (i as u64);
                let mut ct_gsw = PolyMatrixNTT::zero(&params, 2, 2 * params.t_gsw);

                for j in 0..params.t_gsw {
                    let value = (1u64 << (bits_per * j)) * bit;
                    let sigma = PolyMatrixRaw::single_value(&params, value);
                    let sigma_ntt = to_ntt_alloc(&sigma);

                    // important to rng in the right order here
                    let prod = &to_ntt_alloc(&self.sk_reg) * &sigma_ntt;
                    let ct = &self.encrypt_matrix_reg(&prod, &mut rng, &mut rng_pub);
                    ct_gsw.copy_into(ct, 0, 2 * j);

                    let ct = &self.encrypt_matrix_reg(&sigma_ntt, &mut rng, &mut rng_pub);
                    ct_gsw.copy_into(ct, 0, 2 * j + 1);
                }
                sigma_v.push(ct_gsw);
            }

            query.v_buf = Some(reg_cts_buf);
            query.v_ct = Some(sigma_v.iter().map(|x| from_ntt_alloc(x)).collect());
        }
        query
    }

    pub fn generate_full_query(&self, id: &str, idx_target: usize) -> Vec<u8> {
        assert_eq!(id.len(), UUID_V4_LEN);
        let query = self.generate_query(idx_target);
        let mut query_buf = query.serialize();
        let mut full_query_buf = id.as_bytes().to_vec();
        full_query_buf.append(&mut query_buf);
        full_query_buf
    }

    pub fn decode_response(&self, data: &[u8]) -> Vec<u8> {
        /*
            0. NTT over q2 the secret key

            1. read first row in q2_bit chunks
            2. read rest in q1_bit chunks
            3. NTT over q2 the first row
            4. Multiply the results of (0) and (3)
            5. Divide and round correctly
        */
        let params = self.params;
        let p = params.pt_modulus;
        let p_bits = log2_ceil(params.pt_modulus);
        let q1 = 4 * params.pt_modulus;
        let q1_bits = log2_ceil(q1) as usize;
        let q2 = Q2_VALUES[params.q2_bits as usize];
        let q2_bits = params.q2_bits as usize;

        let q2_params = params_with_moduli(params, &vec![q2]);

        // this only needs to be done during keygen
        let mut sk_gsw_q2 = PolyMatrixRaw::zero(&q2_params, params.n, 1);
        for i in 0..params.poly_len * params.n {
            sk_gsw_q2.data[i] = recenter(self.sk_gsw.data[i], params.modulus, q2);
        }
        let mut sk_gsw_q2_ntt = PolyMatrixNTT::zero(&q2_params, params.n, 1);
        to_ntt(&mut sk_gsw_q2_ntt, &sk_gsw_q2);

        let mut result = PolyMatrixRaw::zero(&params, params.instances * params.n, params.n);

        let mut bit_offs = 0;
        for instance in 0..params.instances {
            // this must be done during decoding
            let mut first_row = PolyMatrixRaw::zero(&q2_params, 1, params.n);
            let mut rest_rows = PolyMatrixRaw::zero(&params, params.n, params.n);
            for i in 0..params.n * params.poly_len {
                first_row.data[i] = read_arbitrary_bits(data, bit_offs, q2_bits);
                bit_offs += q2_bits;
            }
            for i in 0..params.n * params.n * params.poly_len {
                rest_rows.data[i] = read_arbitrary_bits(data, bit_offs, q1_bits);
                bit_offs += q1_bits;
            }

            let mut first_row_q2 = PolyMatrixNTT::zero(&q2_params, 1, params.n);
            to_ntt(&mut first_row_q2, &first_row);

            let sk_prod = (&sk_gsw_q2_ntt * &first_row_q2).raw();

            let q1_i64 = q1 as i64;
            let q2_i64 = q2 as i64;
            let p_i128 = p as i128;
            for i in 0..params.n * params.n * params.poly_len {
                let mut val_first = sk_prod.data[i] as i64;
                if val_first >= q2_i64 / 2 {
                    val_first -= q2_i64;
                }
                let mut val_rest = rest_rows.data[i] as i64;
                if val_rest >= q1_i64 / 2 {
                    val_rest -= q1_i64;
                }

                let denom = (q2 * (q1 / p)) as i64;

                let mut r = val_first * q1_i64;
                r += val_rest * q2_i64;

                // divide r by q2, rounding
                let sign: i64 = if r >= 0 { 1 } else { -1 };
                let mut res = ((r + sign * (denom / 2)) as i128) / (denom as i128);
                res = (res + (denom as i128 / p_i128) * (p_i128) + 2 * (p_i128)) % (p_i128);
                let idx = instance * params.n * params.n * params.poly_len + i;
                result.data[idx] = res as u64;
            }
        }

        // println!("{:?}", result.data.as_slice().to_vec());
        result.to_vec(p_bits as usize, params.modp_words_per_chunk())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_params() -> Params {
        get_short_keygen_params()
    }

    #[test]
    fn init_is_correct() {
        let params = get_params();
        let client = Client::init(&params);

        assert_eq!(*client.params, params);
    }

    #[test]
    fn keygen_is_correct() {
        let params = get_params();
        let mut client = Client::init(&params);

        _ = client.generate_keys();

        let threshold = (10.0 * params.noise_width) as u64;

        for i in 0..client.sk_gsw.data.len() {
            let val = client.sk_gsw.data[i];
            assert!((val < threshold) || ((params.modulus - val) < threshold));
        }
    }

    fn get_vec(v: &Vec<PolyMatrixNTT>) -> Vec<u64> {
        v.iter().map(|d| d.as_slice().to_vec()).flatten().collect()
    }

    fn public_parameters_serialization_is_correct_for_params(params: Params) {
        let mut client = Client::init(&params);
        let pub_params = client.generate_keys();

        let serialized1 = pub_params.serialize();
        let deserialized1 = PublicParameters::deserialize(&params, &serialized1);
        let serialized2 = deserialized1.serialize();

        assert_eq!(serialized1, serialized2);
        assert_eq!(
            get_vec(&pub_params.v_packing),
            get_vec(&deserialized1.v_packing)
        );

        println!(
            "packing mats (bytes) {}",
            get_vec(&pub_params.v_packing).len() * 8
        );
        println!("total size   (bytes) {}", serialized1.len());
        if pub_params.v_conversion.is_some() {
            let l1 = get_vec(&pub_params.v_conversion.unwrap());
            assert_eq!(l1, get_vec(&deserialized1.v_conversion.unwrap()));
            println!("conv mats (bytes) {}", l1.len() * 8);
        }
        if pub_params.v_expansion_left.is_some() {
            let l1 = get_vec(&pub_params.v_expansion_left.unwrap());
            assert_eq!(l1, get_vec(&deserialized1.v_expansion_left.unwrap()));
            println!("exp left (bytes) {}", l1.len() * 8);
        }
        if pub_params.v_expansion_right.is_some() {
            let l1 = get_vec(&pub_params.v_expansion_right.unwrap());
            assert_eq!(l1, get_vec(&deserialized1.v_expansion_right.unwrap()));
            println!("exp right (bytes) {}", l1.len() * 8);
        }
    }

    #[test]
    fn public_parameters_serialization_is_correct() {
        public_parameters_serialization_is_correct_for_params(get_params())
    }

    #[test]
    fn real_public_parameters_serialization_is_correct() {
        let cfg_expand = r#"
            {'n': 2,
            'nu_1': 10,
            'nu_2': 6,
            'p': 512,
            'q2_bits': 21,
            's_e': 85.83255142749422,
            't_gsw': 10,
            't_conv': 4,
            't_exp_left': 16,
            't_exp_right': 56,
            'instances': 11,
            'db_item_size': 100000 }
        "#;
        let cfg = cfg_expand.replace("'", "\"");
        let params = params_from_json(&cfg);
        public_parameters_serialization_is_correct_for_params(params)
    }

    #[test]
    fn real_public_parameters_2_serialization_is_correct() {
        let cfg = r#"
            { "n": 4,
            "nu_1": 9,
            "nu_2": 5,
            "p": 256,
            "q2_bits": 20,
            "t_gsw": 8,
            "t_conv": 4,
            "t_exp_left": 8,
            "t_exp_right": 56,
            "instances": 2,
            "db_item_size": 65536 }
        "#;
        let params = params_from_json(&cfg);
        public_parameters_serialization_is_correct_for_params(params)
    }

    #[test]
    fn no_expansion_public_parameters_serialization_is_correct() {
        public_parameters_serialization_is_correct_for_params(get_no_expansion_testing_params())
    }

    fn query_serialization_is_correct_for_params(params: Params) {
        let mut client = Client::init(&params);
        _ = client.generate_keys();
        let query = client.generate_query(1);

        let serialized1 = query.serialize();
        let deserialized1 = Query::deserialize(&params, &serialized1);
        let serialized2 = deserialized1.serialize();

        assert_eq!(serialized1.len(), serialized2.len());
        for i in 0..serialized1.len() {
            assert_eq!(serialized1[i], serialized2[i], "at {}", i);
        }
    }

    #[test]
    fn query_serialization_is_correct() {
        query_serialization_is_correct_for_params(get_params())
    }

    #[test]
    fn no_expansion_query_serialization_is_correct() {
        query_serialization_is_correct_for_params(get_no_expansion_testing_params())
    }
}
