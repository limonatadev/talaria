use talaria_core::config::SupabaseConfig;
use talaria_core::supabase::SupabaseClient;

#[test]
fn supabase_public_url_builds() {
    let cfg = SupabaseConfig {
        url: "https://example.supabase.co".into(),
        service_role_key: Some("sk_test".into()),
        bucket: "bucket".into(),
        public_base: None,
        upload_prefix: "talaria".into(),
    };
    let client = SupabaseClient::from_config(&cfg).unwrap();
    let url = client.public_url("talaria/image.jpg");
    assert_eq!(
        url,
        "https://example.supabase.co/storage/v1/object/public/bucket/talaria/image.jpg"
    );
}
