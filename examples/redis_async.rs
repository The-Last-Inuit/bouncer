use redis::AsyncCommands;

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut connection = client.get_async_connection().await?;
    let _: () = connection.set("key1", b"foo").await?;
    let _: () = redis::cmd("SET")
        .arg(&["key2", "bar"])
        .query_async(&mut connection)
        .await?;
    let result = redis::cmd("MGET")
        .arg(&["key1", "key2"])
        .query_async(&mut connection)
        .await;
    assert_eq!(result, Ok(("foo".to_string(), b"bar".to_vec())));
    Ok(())
}
