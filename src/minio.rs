use aws_config::BehaviorVersion;
use aws_sdk_s3::{config::Builder, Client};

pub async fn get_client(endpoint: &str) -> Client {
    let config_loader = aws_config::defaults(BehaviorVersion::latest()).endpoint_url(endpoint);
    let config = config_loader.load().await;
    let config = Builder::from(&config).force_path_style(true).build();
    Client::from_conf(config)
}

#[cfg(test)]
mod tests {
    use std::env::var;

    use super::*;

    #[tokio::test]
    async fn test_get_client() {
        let bucekt_name = var("IMAGES_BUCKET").expect("IMAGES_BUCKET is not set");
        let endpoint = var("S3_ENDPOINT").expect("S3_ENDPOINT is not set");
        let client = get_client(&endpoint).await;

        // バケット一覧を取得する
        let resp = client.list_buckets().send().await.unwrap();
        assert!(resp.buckets.is_some());
        println!("{:?}", resp.clone().buckets.unwrap());
        assert!(resp
            .buckets
            .unwrap()
            .iter()
            .any(|b| b.name == Some(bucekt_name.clone())));

        // オブジェクト一覧を取得する
        client
            .list_objects()
            .bucket(bucekt_name)
            .send()
            .await
            .unwrap();
    }
}
