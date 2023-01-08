extern crate redis;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use redis::{Client, Commands, Connection, RedisResult};
use crate::databases::database::{IDatabase, DbConfig, combine_key, MessageStatusDef, IdempotencyTransaction};
use std::convert::TryFrom;
use std::error::Error;

pub struct RedisClient{
    client: Client,
    con: Connection,
    config: DbConfig
}

#[async_trait]
impl IDatabase for RedisClient {

    async fn exists(&mut self, key: String, app_id: String) -> Result<IdempotencyTransaction, Box<dyn Error>> {
        let full_key = combine_key(key, app_id);
        let exists: bool = self.con.exists(&full_key, )?;
        if !exists {
            return Ok(IdempotencyTransaction::new_default_none());
        }
        // todo: get actual status
        let valString : String = self.con.get(&full_key)?;
        let deserialized: IdempotencyTransaction = serde_json::from_str(&valString).unwrap();
        return Ok(deserialized);
    }

    async fn delete (&mut self, key: String, app_id: String) -> Result<(), Box<dyn Error>> {
        self.con.del(combine_key(key, app_id))?;
        Ok(())
    }

    async fn put (&mut self, key: String, app_id: String, value: IdempotencyTransaction, ttl: Option<Duration>) -> Result<(), Box<dyn Error>>{
        let ttl_usize = usize::try_from(ttl.unwrap().as_secs()).unwrap();
        let _ : () = self.con.set_ex(combine_key(key, app_id), serde_json::to_string(&value).unwrap(), ttl_usize)?;
        Ok(())
    }

}

impl RedisClient {
    pub(crate) fn new (config: DbConfig) -> Box<dyn IDatabase> {
        let client = Client::open(config.url.clone()).unwrap();
        let con =  client.get_connection().unwrap();
        let r = RedisClient {
            client,
            con,
            config: config.clone()
        };
        return Box::new(r);
    }

}

#[cfg(test)]
mod tests {
    use crate::databases::database::DatabaseOption;
    use super::*;

    fn init_client() -> Box<dyn IDatabase> {
        let c = RedisClient::new(DbConfig{
            table_name: None,
            url: String::from("redis://default:redispw@localhost:49153"),
            keyspace: None,
            ttl: None,
            database_option: DatabaseOption::Redis
        });
        c
    }

    fn get_app_id() -> String {
        String::from("my_app")
    }

    #[tokio::test]
    pub async fn test_put() {
        let key = String::from("test_put");
        let mut c = init_client();
        c.delete(key.clone(), get_app_id()).await.unwrap();
        c.put(key.clone(), String::from("my_app"), IdempotencyTransaction {
            response: String::from("SomeString"),
            status: MessageStatusDef::Completed
        },Some(Duration::from_secs(30))).await.unwrap();
        let result = c.exists(key.clone(), get_app_id()).await.unwrap();
        assert_eq!(result.status, MessageStatusDef::Completed);
        assert_eq!(result.response, "SomeString");
    }

    #[tokio::test]
    pub async fn test_delete() {
        let mut c = init_client();
        let key = String::from("test_delete");
        c.put(key.clone(), get_app_id(), IdempotencyTransaction {
            response: String::from("SomeString"),
            status: MessageStatusDef::Completed
        },Some(Duration::from_secs(30))).await.unwrap();
        c.delete(key.clone(), String::from("my_app")).await.unwrap();
        let result = c.exists(key.clone(), get_app_id()).await.unwrap();
        assert_eq!(result.status, MessageStatusDef::None);
        assert_eq!(result.response, "");
    }


}