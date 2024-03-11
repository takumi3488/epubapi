use aws_config::BehaviorVersion;
use aws_sdk_s3::{config::Builder, Client};
use std::env::var;

pub async fn get_client() -> Client {
    let s3_endpoint = var("S3_ENDPOINT").expect("S3_ENDPOINT is not set");
    let config_loader = aws_config::defaults(BehaviorVersion::latest()).endpoint_url(s3_endpoint);
    let config = config_loader.load().await;
    let config = Builder::from(&config).force_path_style(true).build();
    Client::from_conf(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_client() {
        let bucekt_name = var("IMAGES_BUCKET").expect("IMAGES_BUCKET is not set");
        let client = get_client().await;

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
