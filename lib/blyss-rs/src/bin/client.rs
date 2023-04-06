use blyss_rs::proof::fetch_merkle_proof;

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    // Fetches a proof for the "0x06eaa1..." identity commitment, at index 700000
    let proof = fetch_merkle_proof(
        "https://blyss-hints.s3.us-east-2.amazonaws.com/lookup-cfg.json",
        "0x06eaa1912c3c31b6c2063e397faaba5ad43052812d5051c9b731c5618fe02c6d",
    )
    .await;
    println!("{:?}", proof);
}
