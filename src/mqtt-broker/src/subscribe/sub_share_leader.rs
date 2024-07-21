use super::{
    sub_common::{
        loop_commit_offset, min_qos, publish_message_qos0, publish_message_to_client,
        qos2_send_publish, qos2_send_pubrel, wait_packet_ack,
    },
    subscribe_cache::SubscribeCacheManager,
};
use crate::{
    handler::cache_manager::{
        CacheManager, QosAckPackageData, QosAckPackageType, QosAckPacketInfo,
    },
    server::{connection_manager::ConnectionManager, packet::ResponsePackage},
    storage::message::MessageStorage,
    subscribe::subscriber::Subscriber,
};
use bytes::Bytes;
use common_base::{
    errors::RobustMQError,
    log::{error, info},
    tools::now_second,
};
use metadata_struct::mqtt::message::MQTTMessage;
use protocol::mqtt::common::{MQTTPacket, Publish, PublishProperties, QoS};
use std::{sync::Arc, time::Duration};
use storage_adapter::storage::StorageAdapter;
use tokio::{
    select,
    sync::broadcast::{self, Sender},
    time::sleep,
};

#[derive(Clone)]
pub struct SubscribeShareLeader<S> {
    pub subscribe_manager: Arc<SubscribeCacheManager>,
    message_storage: Arc<S>,
    connection_manager: Arc<ConnectionManager>,
    cache_manager: Arc<CacheManager>,
}

impl<S> SubscribeShareLeader<S>
where
    S: StorageAdapter + Sync + Send + 'static + Clone,
{
    pub fn new(
        subscribe_manager: Arc<SubscribeCacheManager>,
        message_storage: Arc<S>,
        connection_manager: Arc<ConnectionManager>,
        cache_manager: Arc<CacheManager>,
    ) -> Self {
        return SubscribeShareLeader {
            subscribe_manager,
            message_storage,
            connection_manager,
            cache_manager,
        };
    }

    pub async fn start(&self) {
        loop {
            self.start_push_thread();
            self.try_thread_gc();
            sleep(Duration::from_secs(1)).await;
        }
    }

    pub fn try_thread_gc(&self) {
        // Periodically verify that a push task is running, but the subscribe task has stopped
        // If so, stop the process and clean up the data
        for (share_leader_key, sx) in self.subscribe_manager.share_leader_push_thread.clone() {
            if !self
                .subscribe_manager
                .share_leader_subscribe
                .contains_key(&share_leader_key)
            {
                match sx.send(true) {
                    Ok(_) => {
                        self.subscribe_manager
                            .share_leader_push_thread
                            .remove(&share_leader_key);
                    }
                    Err(err) => {
                        error(format!(
                            "stop sub share thread error, error message:{}",
                            err.to_string()
                        ));
                    }
                }
            }
        }
    }

    pub fn start_push_thread(&self) {
        // Periodically verify if any push tasks are not started. If so, the thread is started
        for (share_leader_key, sub_data) in self.subscribe_manager.share_leader_subscribe.clone() {
            if sub_data.sub_list.len() == 0 {
                if let Some(sx) = self
                    .subscribe_manager
                    .share_leader_push_thread
                    .get(&share_leader_key)
                {
                    match sx.send(true) {
                        Ok(_) => {
                            self.subscribe_manager
                                .share_leader_subscribe
                                .remove(&share_leader_key);
                        }
                        Err(_) => {}
                    }
                }
            }

            // start push data thread
            let subscribe_manager = self.subscribe_manager.clone();
            if !self
                .subscribe_manager
                .share_leader_push_thread
                .contains_key(&share_leader_key)
            {
                self.push_by_round_robin(
                    share_leader_key.clone(),
                    sub_data.group_name.clone(),
                    sub_data.topic_id.clone(),
                    sub_data.topic_name.clone(),
                    subscribe_manager,
                );
            }
        }
    }

    fn push_by_round_robin(
        &self,
        share_leader_key: String,
        group_name: String,
        topic_id: String,
        topic_name: String,
        subscribe_manager: Arc<SubscribeCacheManager>,
    ) {
        let (stop_sx, mut stop_rx) = broadcast::channel(1);
        self.subscribe_manager
            .share_leader_push_thread
            .insert(share_leader_key.clone(), stop_sx.clone());

        let connection_manager = self.connection_manager.clone();
        let cache_manager = self.cache_manager.clone();
        let message_storage = self.message_storage.clone();

        tokio::spawn(async move {
            info(format!(
                "Share leader push data thread for GroupName {},Topic [{}] was started successfully",
                group_name, topic_name
            ));

            let message_storage: MessageStorage<S> = MessageStorage::new(message_storage);
            let group_id = format!("system_sub_{}_{}", group_name, topic_id);

            let mut cursor_point = 0;
            let mut sub_list: Vec<Subscriber> =
                build_share_leader_sub_list(subscribe_manager.clone(), share_leader_key.clone());

            loop {
                select! {
                    val = stop_rx.recv() =>{
                        match val {
                            Ok(flag) => {
                                if flag {
                                    info(format!(
                                        "Share sub push data thread for GroupName {},Topic [{}] was stopped successfully",
                                        group_name, topic_name
                                    ));
                                    break;
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    (cp,sl) = read_message_process(
                        &share_leader_key,
                        &subscribe_manager,
                        &topic_id,
                        &topic_name,
                        &message_storage,
                        sub_list.clone(),
                        &group_id,
                        cursor_point,
                        &connection_manager,
                        &cache_manager,
                        &stop_sx
                    ) =>{
                        cursor_point = cp;
                        sub_list = sl;
                    }
                }
            }

            subscribe_manager
                .share_leader_push_thread
                .remove(&share_leader_key);
        });
    }
}

async fn read_message_process<S>(
    share_leader_key: &String,
    subscribe_manager: &Arc<SubscribeCacheManager>,
    topic_id: &String,
    topic_name: &String,
    message_storage: &MessageStorage<S>,
    mut sub_list: Vec<Subscriber>,
    group_id: &String,
    mut cursor_point: usize,
    connection_manager: &Arc<ConnectionManager>,
    cache_manager: &Arc<CacheManager>,
    stop_sx: &Sender<bool>,
) -> (usize, Vec<Subscriber>)
where
    S: StorageAdapter + Sync + Send + 'static + Clone,
{
    let max_wait_ms: u64 = 500;
    let record_num = calc_record_num(sub_list.len());
    match message_storage
        .read_topic_message(topic_id.clone(), group_id.clone(), record_num as u128)
        .await
    {
        Ok(results) => {
            if results.len() == 0 {
                sleep(Duration::from_millis(max_wait_ms)).await;
                return (cursor_point, sub_list.clone());
            }
            for record in results {
                let msg: MQTTMessage = match MQTTMessage::decode_record(record.clone()) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error(format!(
                            "Storage layer message Decord failed with error message :{}",
                            e.to_string()
                        ));
                        loop_commit_offset(message_storage, topic_id, group_id, record.offset)
                            .await;
                        return (cursor_point, sub_list);
                    }
                };
                let mut loop_times = 0;
                loop {
                    let current_point = if cursor_point < sub_list.len() {
                        cursor_point
                    } else {
                        sub_list = build_share_leader_sub_list(
                            subscribe_manager.clone(),
                            share_leader_key.clone(),
                        );
                        0
                    };
                    if sub_list.len() == 0 {
                        sub_list = build_share_leader_sub_list(
                            subscribe_manager.clone(),
                            share_leader_key.clone(),
                        );
                        sleep(Duration::from_micros(100)).await;
                        continue;
                    }

                    if loop_times > sub_list.len() {
                        break;
                    }

                    let subscribe = sub_list.get(current_point).unwrap();

                    cursor_point = current_point + 1;
                    if let Some((mut publish, properties)) = build_publish(
                        cache_manager.clone(),
                        subscribe.clone(),
                        topic_name.clone(),
                        msg.clone(),
                    ) {
                        match publish.qos {
                            QoS::AtMostOnce => {
                                publish_message_qos0(
                                    cache_manager,
                                    &subscribe.client_id,
                                    &publish,
                                    &Some(properties.clone()),
                                    connection_manager,
                                    stop_sx,
                                )
                                .await;

                                // commit offset
                                loop_commit_offset(
                                    message_storage,
                                    topic_id,
                                    group_id,
                                    record.offset,
                                )
                                .await;
                                break;
                            }

                            QoS::AtLeastOnce => {
                                let pkid: u16 = cache_manager.get_pkid(&subscribe.client_id).await;
                                publish.pkid = pkid;

                                let (wait_puback_sx, _) = broadcast::channel(1);
                                cache_manager.add_ack_packet(
                                    &subscribe.client_id,
                                    pkid,
                                    QosAckPacketInfo {
                                        sx: wait_puback_sx.clone(),
                                        create_time: now_second(),
                                    },
                                );

                                match share_leader_publish_message_qos1(
                                    cache_manager,
                                    &subscribe.client_id,
                                    &publish,
                                    &properties,
                                    pkid,
                                    connection_manager,
                                    &wait_puback_sx,
                                )
                                .await
                                {
                                    Ok(()) => {
                                        // commit offset
                                        loop_commit_offset(
                                            &message_storage,
                                            &topic_id,
                                            &group_id,
                                            record.offset,
                                        )
                                        .await;

                                        // remove data
                                        cache_manager.remove_pkid_info(&subscribe.client_id, pkid);
                                        cache_manager.remove_ack_packet(&subscribe.client_id, pkid);
                                        break;
                                    }
                                    Err(e) => {
                                        error(format!("SharSub Leader failed to send QOS1 message to {}, error message :{},
                                         trying to deliver the message to another client.",subscribe.client_id.clone(),e.to_string()));
                                        loop_times = loop_times + 1;
                                    }
                                }
                            }

                            QoS::ExactlyOnce => {
                                let pkid: u16 = cache_manager.get_pkid(&subscribe.client_id).await;
                                publish.pkid = pkid;

                                let (wait_ack_sx, _) = broadcast::channel(1);
                                cache_manager.add_ack_packet(
                                    &subscribe.client_id,
                                    pkid,
                                    QosAckPacketInfo {
                                        sx: wait_ack_sx.clone(),
                                        create_time: now_second(),
                                    },
                                );

                                match share_leader_publish_message_qos2(
                                    cache_manager,
                                    &subscribe.client_id,
                                    &publish,
                                    &properties,
                                    pkid,
                                    connection_manager,
                                    stop_sx,
                                    &wait_ack_sx,
                                    topic_id,
                                    group_id,
                                    record.offset,
                                    message_storage,
                                )
                                .await
                                {
                                    Ok(()) => {
                                        break;
                                    }
                                    Err(e) => {
                                        error(e.to_string());
                                        loop_times = loop_times + 1;
                                    }
                                }
                            }
                        };
                    } else {
                        break;
                    }
                }
            }
            return (cursor_point, sub_list);
        }
        Err(e) => {
            error(format!(
                "Failed to read message from storage, failure message: {},topic:{},group{}",
                e.to_string(),
                topic_id.clone(),
                group_id.clone()
            ));
            sleep(Duration::from_millis(max_wait_ms)).await;
            return (cursor_point, sub_list);
        }
    }
}

pub fn build_publish(
    metadata_cache: Arc<CacheManager>,
    subscribe: Subscriber,
    topic_name: String,
    msg: MQTTMessage,
) -> Option<(Publish, PublishProperties)> {
    let mut sub_id = Vec::new();
    if let Some(id) = subscribe.subscription_identifier {
        sub_id.push(id);
    }

    let cluster_qos = metadata_cache.get_cluster_info().max_qos();
    let qos = min_qos(cluster_qos, subscribe.qos);

    let retain = if subscribe.preserve_retain {
        msg.retain
    } else {
        false
    };

    if subscribe.nolocal && (subscribe.client_id == msg.client_id) {
        return None;
    }

    let publish = Publish {
        dup: false,
        qos: qos.clone(),
        pkid: 0,
        retain,
        topic: Bytes::from(topic_name.clone()),
        payload: msg.payload,
    };

    let properties = PublishProperties {
        payload_format_indicator: msg.format_indicator,
        message_expiry_interval: msg.expiry_interval,
        topic_alias: None,
        response_topic: msg.response_topic,
        correlation_data: msg.correlation_data,
        user_properties: msg.user_properties,
        subscription_identifiers: sub_id.clone(),
        content_type: msg.content_type,
    };
    return Some((publish, properties));
}

// To avoid messages that are not successfully pushed to the client. When the client Session expires,
// the push thread will exit automatically and will not attempt to push again.
async fn share_leader_publish_message_qos1(
    metadata_cache: &Arc<CacheManager>,
    client_id: &String,
    publish: &Publish,
    publish_properties: &PublishProperties,
    pkid: u16,
    connection_manager: &Arc<ConnectionManager>,
    wait_puback_sx: &broadcast::Sender<QosAckPackageData>,
) -> Result<(), RobustMQError> {
    let connect_id = if let Some(id) = metadata_cache.get_connect_id(&client_id) {
        id
    } else {
        return Err(RobustMQError::CommmonError(format!(
            "Client [{}] failed to get connect id, no connection available.",
            client_id.clone()
        )));
    };

    if let Some(conn) = metadata_cache.get_connection(connect_id) {
        if publish.payload.len() > (conn.max_packet_size as usize) {
            return Err(RobustMQError::PacketLenthError(publish.payload.len()));
        }
    }

    let resp = ResponsePackage {
        connection_id: connect_id,
        packet: MQTTPacket::Publish(publish.clone(), Some(publish_properties.clone())),
    };

    match publish_message_to_client(resp.clone(), connection_manager).await {
        Ok(_) => {
            if let Some(data) = wait_packet_ack(wait_puback_sx).await {
                if data.ack_type == QosAckPackageType::PubAck && data.pkid == pkid {
                    return Ok(());
                }
            }
            return Err(RobustMQError::CommmonError(
                "QOS1 publishes a message and waits for the PubAck packet to fail to be received"
                    .to_string(),
            ));
        }
        Err(e) => {
            return Err(RobustMQError::CommmonError(format!(
                "Failed to write QOS1 Publish message to response queue, failure message: {}",
                e.to_string()
            )));
        }
    }
}

// send publish message
// wait pubrec message
// send pubrel message
// wait pubcomp message
async fn share_leader_publish_message_qos2<S>(
    cache_manager: &Arc<CacheManager>,
    client_id: &String,
    publish: &Publish,
    publish_properties: &PublishProperties,
    pkid: u16,
    connection_manager: &Arc<ConnectionManager>,
    stop_sx: &broadcast::Sender<bool>,
    wait_ack_sx: &broadcast::Sender<QosAckPackageData>,
    topic_id: &String,
    group_id: &String,
    offset: u128,
    message_storage: &MessageStorage<S>,
) -> Result<(), RobustMQError>
where
    S: StorageAdapter + Sync + Send + 'static + Clone,
{
    // 1. send Publish to Client
    match qos2_send_publish(
        connection_manager,
        cache_manager,
        client_id,
        publish,
        &Some(publish_properties.clone()),
        stop_sx,
    )
    .await
    {
        Ok(()) => {}
        Err(e) => return Err(e),
    };

    // 2. wait pub rec
    loop {
        match stop_sx.subscribe().try_recv() {
            Ok(flag) => {
                if flag {
                    return Ok(());
                }
            }
            Err(_) => {}
        }
        if let Some(data) = wait_packet_ack(wait_ack_sx).await {
            if data.ack_type == QosAckPackageType::PubRec && data.pkid == pkid {
                // When sending a QOS2 message, as long as the pubrec is received, the offset can be submitted,
                // the pubrel is sent asynchronously, and the pubcomp is waited for. Push the next message at the same time.
                loop_commit_offset(message_storage, topic_id, group_id, offset).await;
                break;
            }
        } else {
            return Err(RobustMQError::SubPublishWaitPubRecTimeout(
                client_id.clone(),
            ));
        }
    }

    // async wait
    // 3. send pub rel
    qos2_send_pubrel(cache_manager, client_id, pkid, connection_manager, stop_sx).await;

    // 4. wait pub comp
    loop {
        match stop_sx.subscribe().try_recv() {
            Ok(flag) => {
                if flag {
                    break;
                }
            }
            Err(_) => {}
        }
        if let Some(data) = wait_packet_ack(wait_ack_sx).await {
            if data.ack_type == QosAckPackageType::PubComp && data.pkid == pkid {
                cache_manager.remove_pkid_info(&client_id, pkid);
                cache_manager.remove_ack_packet(&client_id, pkid);
                break;
            }
        } else {
            qos2_send_pubrel(cache_manager, client_id, pkid, connection_manager, stop_sx).await;
        }
    }

    return Ok(());
}

fn build_share_leader_sub_list(
    subscribe_manager: Arc<SubscribeCacheManager>,
    key: String,
) -> Vec<Subscriber> {
    let sub_list = if let Some(sub) = subscribe_manager.share_leader_subscribe.get(&key) {
        sub.sub_list.clone()
    } else {
        return Vec::new();
    };

    let mut result = Vec::new();
    for (_, sub) in sub_list {
        result.push(sub);
    }
    return result;
}

fn calc_record_num(sub_len: usize) -> usize {
    if sub_len == 0 {
        return 100;
    }

    let num = sub_len * 5;
    if num > 1000 {
        return 1000;
    }
    return num;
}

#[cfg(test)]
mod tests {}
