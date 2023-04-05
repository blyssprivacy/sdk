use blyss_rs::proof::{fetch_merkle_proof, LookupCfg};

#[tokio::main]
pub async fn main() {
    let proof = fetch_merkle_proof(
        &LookupCfg {
            bucket_url: "https://beta.api.blyss.dev/global.wc-v3".to_owned(),
            api_key: "hsV7FEfprL7ASpNMKTZhoupZs21eEWz6A23BeYwi".to_owned(),
            cap_url: "https://blyss-hints.s3.us-east-2.amazonaws.com/wc-cap.json".to_owned(),
            tree_height: 21,
            subtree_height: 4,
            cap_height: 12,
        },
        "0x06eaa1912c3c31b6c2063e397faaba5ad43052812d5051c9b731c5618fe02c6d", // identity commitment, index 700000
    )
    .await;
    println!("{:?}", proof);
}
