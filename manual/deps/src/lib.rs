#[cfg(test)]
mod example;

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn examples() -> anyhow::Result<()> {
        super::example::local_rules::example().await?;
        super::example::server_side_enforcement::example().await?;
        super::example::automatic_queryable_encryption::example().await?;
        super::example::explicit_queryable_encryption::example().await?;
        super::example::explicit_encryption::example().await?;
        super::example::explicit_encryption_auto_decryption::example().await?;
        Ok(())
    }
}
