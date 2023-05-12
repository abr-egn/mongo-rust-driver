use mongodb::{
    bson::doc,
    Client, client_encryption::{ClientEncryption, MasterKey}, mongocrypt::ctx::KmsProvider, Namespace,
    error::Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    let c = ClientEncryption::new(
        Client::with_uri_str("mongodb://localhost:27017").await?,
        Namespace::new("keyvault", "datakeys"),
        [(KmsProvider::Azure, doc! { }, None)],
    )?;

    c.create_data_key(MasterKey::Azure {
        key_vault_endpoint: "https://keyvault-drivers-2411.vault.azure.net/keys/".to_string(),
        key_name: "KEY-NAME".to_string(),
        key_version: None,
    })
    .run()
    .await?;

    println!("Azure KMS integration test passed!");

    Ok(())
}
