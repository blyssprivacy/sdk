use crate::{
    api::{http_get_string, private_read},
    error::Error,
};
use ruint::aliases::U256;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupCfg {
    pub bucket_url: String,
    pub api_key: String,
    pub cap_url: String,
    pub subtree_height: usize,
    pub cap_height: usize,
    pub tree_height: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofStep {
    pub value: String,
    pub pos: usize,
}

impl ProofStep {
    pub fn u256(&self) -> U256 {
        to_u256(&self.value)
    }
}

pub fn to_u256(value: &str) -> U256 {
    U256::from_be_bytes::<32>(hex::decode(&value[2..]).unwrap().try_into().unwrap())
}

pub fn to_str(value: &U256) -> String {
    format!("0x{}", hex::encode(value.to_be_bytes::<32>()))
}

fn get_subtree_indices(lookup_cfg: &LookupCfg, identity_idx: usize) -> Vec<String> {
    let mut keys_to_fetch = Vec::new();
    let mut cur_level = lookup_cfg.tree_height - lookup_cfg.subtree_height;
    while cur_level >= lookup_cfg.cap_height - 1 {
        let idx_within_level = identity_idx >> (lookup_cfg.tree_height - 1 - cur_level);
        let key = format!("{}-{}", cur_level, idx_within_level);
        keys_to_fetch.push(key);

        if cur_level >= lookup_cfg.subtree_height {
            cur_level -= lookup_cfg.subtree_height - 1;
        } else {
            break;
        }
    }

    keys_to_fetch
}

fn get_subproof(tree: &[String], tree_height: usize, idx: usize) -> Vec<ProofStep> {
    let mut out = Vec::new();
    for level in 1..tree_height {
        let mut idx_within_level = idx >> (tree_height - 1 - level);
        idx_within_level ^= 1; // flip low bit to get sibiling

        let tree_idx = (1 << level) - 1 + idx_within_level;
        out.push(ProofStep {
            value: tree[tree_idx].clone(),
            pos: idx_within_level & 1,
        });
    }
    out.reverse();
    return out;
}

fn construct_merkle_proof(
    lookup_cfg: &LookupCfg,
    identity_idx: usize,
    subtrees: &[Vec<String>],
) -> Vec<ProofStep> {
    let mut cur_level = lookup_cfg.tree_height - lookup_cfg.subtree_height;
    let mut outer_idx = 0;

    let mut proof = Vec::new();
    while cur_level >= lookup_cfg.cap_height - 1 {
        let subtree = &subtrees[outer_idx];
        outer_idx += 1;
        let idx_within_level = identity_idx >> (lookup_cfg.tree_height - 1 - cur_level);
        let idx_within_subtree = (identity_idx
            >> (lookup_cfg.tree_height - 1 - (cur_level + lookup_cfg.subtree_height - 1)))
            - idx_within_level * (1 << (lookup_cfg.subtree_height - 1));

        let proof_part = get_subproof(subtree, lookup_cfg.subtree_height, idx_within_subtree);
        proof.extend(proof_part.into_iter());

        if cur_level >= lookup_cfg.subtree_height {
            cur_level -= lookup_cfg.subtree_height - 1;
        } else {
            break;
        }
    }

    proof
}

pub async fn get_cap(url: &str) -> Result<Vec<String>, Error> {
    let val = http_get_string(url, "").await?;
    let cap: Vec<String> = serde_json::from_str(&val)?;
    Ok(cap)
}

fn get_idx_within_cap(idx: usize, tree_height: usize, cap_height: usize) -> usize {
    let idx_within_level = idx >> ((tree_height - 1) - (cap_height - 1));
    idx_within_level
}

pub async fn fetch_merkle_proof_at_idx(
    lookup_cfg: &LookupCfg,
    identity_idx: usize,
) -> Result<Vec<ProofStep>, Error> {
    let cap = get_cap(&lookup_cfg.cap_url).await?;
    let subtrees_to_query = get_subtree_indices(lookup_cfg, identity_idx);
    let subtrees = private_read(
        &lookup_cfg.bucket_url,
        &lookup_cfg.api_key,
        &subtrees_to_query,
    )
    .await?;
    let mut subtrees_as_strs = Vec::new();
    for s in subtrees {
        let s: Vec<String> = serde_json::from_slice(&s)?;
        subtrees_as_strs.push(s);
    }
    let mut proof = construct_merkle_proof(lookup_cfg, identity_idx, &subtrees_as_strs);
    let cap_proof_part = get_subproof(
        &cap,
        lookup_cfg.cap_height,
        get_idx_within_cap(identity_idx, lookup_cfg.tree_height, lookup_cfg.cap_height),
    );
    proof.extend(cap_proof_part.into_iter());

    Ok(proof)
}

pub async fn fetch_idx_for_identity(
    lookup_cfg: &LookupCfg,
    identity_commitment: &str,
) -> Result<usize, Error> {
    let result = private_read(
        &lookup_cfg.bucket_url,
        &lookup_cfg.api_key,
        &[identity_commitment.to_owned()],
    )
    .await?;
    let index: usize = serde_json::from_slice(&result[0])?;

    Ok(index)
}

pub async fn fetch_merkle_proof(
    lookup_cfg: &LookupCfg,
    identity_commitment: &str,
) -> Result<Vec<ProofStep>, Error> {
    let index = fetch_idx_for_identity(lookup_cfg, identity_commitment).await?;
    let proof = fetch_merkle_proof_at_idx(lookup_cfg, index).await?;
    Ok(proof)
}

#[cfg(test)]
mod tests {
    use super::*;

    use semaphore::poseidon;

    pub fn verify_proof(input: &str, proof: &[ProofStep], root: &str) {
        let mut cur_hash = to_u256(&input);
        for step in proof.iter() {
            let step_hash = step.u256();

            let new_hash = if step.pos == 0 {
                poseidon::hash2(step_hash, cur_hash)
            } else {
                poseidon::hash2(cur_hash, step_hash)
            };

            cur_hash = new_hash;
        }
        assert_eq!(to_str(&cur_hash), root);
    }

    #[test]
    fn proof_works() {
        let sample_proof = vec![
            ProofStep {
                value: "0x0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x1df013c70209502f348ea55e649fd86687163959177e3c64eb81101982d46e05"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x1f4a2f66f412222c3b1d6c5ade414a3d8e4b2ebcd4b7500f88ee8284914d3aa3"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x16fc91b5ffdf1e4be6b6c8ba467017e605aaa4edf68c5a0419876fb12f558fc4"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x04650054c0e366b4fd06d65b5fe0b96f3e7e9169f50c176fa25dd50e7c52852f"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x1cebe7a720c3454e5f2e9780f9da6f46bc82db4345bea44f2a816f6d988a6d7e"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x101af632d69b4d1a993b04a53775cf7f12d65cc751355c3bb5bb540548c8de47"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x2187d3e9d93033adb4f250a6a33da9b24223f27c3dd2962082b62cd04c01a6e2"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x1c1a0b78b55b21366a680c8f1c412fbe93864a286195de2ae069e229b4f35874"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x19346ce3cc2e5d46c37aa750f1fbd2363d8546b27ceec21903a7b3dc180cabbf"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x0ffb8ffc70abd907f6548d7043654334dcac5675450b79a9a949b6d68482ce53"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x133028e8db184a9ec1d21cf5617909b6ccf2002ea95bf1f5b70532fc9f217d12"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x21e5840d5e45d43dac5b39ca1620979655c31732be490ae3180baa9d94603ef3"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x2d63fd584ac6c7ecc9fde3eab063f40f789ddae1336b256487b4f0f42403250a"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x23285ed2421e0cd54cf7722ba0478e5386bfba97054f2c00b7ee8ff1e5ae0224"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x21fc8d14983eab9b40668e78f816a785568973e2f7c5c70a921398abb804377b"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x11117446426ca7b68db951a0e90b75fa3895dfdb1e18d76dc009e1446be9bddb"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x10a801cdd8b09b93c1c4763547416bf3a6a1b501af87f2e222e00689c82d3a6d"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x0ea484bbc862f222255634d1859c1c3e3602571e958b0e3acf29f1634f3b45a9"
                    .to_string(),
                pos: 1,
            },
            ProofStep {
                value: "0x1a8c640e78d2e23c36fca18cb69d1ed36ccfa691a26674f34c4077080cbbb16f"
                    .to_string(),
                pos: 1,
            },
        ];

        let root = "0x205aff5d8fc468b111f6fba374f5ba3bdaf02b37a741fd675fac334350f19880";
        verify_proof(
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            &sample_proof,
            root,
        );
    }
}
