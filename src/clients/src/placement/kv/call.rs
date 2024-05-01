use super::PlacementCenterInterface;
use crate::{
    placement::{retry_call, PlacementCenterService},
    poll::ClientPool,
};
use common_base::errors::RobustMQError;
use prost::Message as _;
use protocol::placement_center::generate::{
    common::CommonReply,
    kv::{DeleteRequest, ExistsReply, ExistsRequest, GetReply, GetRequest, SetRequest},
};
use std::sync::Arc;

pub async fn placement_set(
    client_poll: Arc<ClientPool>,
    addrs: Vec<String>,
    request: SetRequest,
) -> Result<CommonReply, RobustMQError> {
    let request_data = SetRequest::encode_to_vec(&request);
    match retry_call(
        PlacementCenterService::Kv,
        PlacementCenterInterface::Set,
        client_poll,
        addrs,
        request_data,
    )
    .await
    {
        Ok(data) => match CommonReply::decode(data.as_ref()) {
            Ok(da) => return Ok(da),
            Err(e) => return Err(RobustMQError::CommmonError(e.to_string())),
        },
        Err(e) => {
            return Err(e);
        }
    }
}

pub async fn placement_get(
    client_poll: Arc<ClientPool>,
    addrs: Vec<String>,
    request: GetRequest,
) -> Result<GetReply, RobustMQError> {
    let request_data = GetRequest::encode_to_vec(&request);
    match retry_call(
        PlacementCenterService::Kv,
        PlacementCenterInterface::Get,
        client_poll,
        addrs,
        request_data,
    )
    .await
    {
        Ok(data) => match GetReply::decode(data.as_ref()) {
            Ok(da) => return Ok(da),
            Err(e) => return Err(RobustMQError::CommmonError(e.to_string())),
        },
        Err(e) => {
            return Err(e);
        }
    }
}

pub async fn placement_delete(
    client_poll: Arc<ClientPool>,
    addrs: Vec<String>,
    request: DeleteRequest,
) -> Result<CommonReply, RobustMQError> {
    let request_data = DeleteRequest::encode_to_vec(&request);
    match retry_call(
        PlacementCenterService::Kv,
        PlacementCenterInterface::Delete,
        client_poll,
        addrs,
        request_data,
    )
    .await
    {
        Ok(data) => match CommonReply::decode(data.as_ref()) {
            Ok(da) => return Ok(da),
            Err(e) => return Err(RobustMQError::CommmonError(e.to_string())),
        },
        Err(e) => {
            return Err(e);
        }
    }
}

pub async fn placement_exists(
    client_poll: Arc<ClientPool>,
    addrs: Vec<String>,
    request: ExistsRequest,
) -> Result<ExistsReply, RobustMQError> {
    let request_data = ExistsRequest::encode_to_vec(&request);
    match retry_call(
        PlacementCenterService::Kv,
        PlacementCenterInterface::Exists,
        client_poll,
        addrs,
        request_data,
    )
    .await
    {
        Ok(data) => match ExistsReply::decode(data.as_ref()) {
            Ok(da) => return Ok(da),
            Err(e) => return Err(RobustMQError::CommmonError(e.to_string())),
        },
        Err(e) => {
            return Err(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::placement::kv::call::{placement_get, placement_set};
    use crate::poll::ClientPool;
    use protocol::placement_center::generate::kv::{GetRequest, SetRequest};

    #[tokio::test]
    async fn set() {
        let client_poll: Arc<ClientPool> = Arc::new(ClientPool::new(1));
        let addrs = vec!["127.0.0.1:1228".to_string()];
        let key = "test-sub-name".to_string();
        let value = "test-group-name".to_string();
        let request = SetRequest { key:key.clone(), value };
        match placement_set(client_poll.clone(), addrs.clone(), request).await {
            Ok(da) => {
                println!("{:?}", da);
                assert!(true)
            }
            Err(e) => {
                println!("{}", e.to_string());
                assert!(false)
            }
        }

        let get_req = GetRequest { key };
        match placement_get(client_poll, addrs, get_req).await {
            Ok(da) => {
                println!("{:?}", da);
                assert!(true)
            }
            Err(e) => {
                println!("{}", e.to_string());
                assert!(false)
            }
        }
    }
}
