// Copyright 2023 RobustMQ Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use common_base::error::common::CommonError;
use protocol::placement_center::placement_center_inner::{
    BindSchemaReply, BindSchemaRequest, ClusterStatusReply, ClusterStatusRequest,
    CreateSchemaReply, CreateSchemaRequest, DeleteIdempotentDataReply, DeleteIdempotentDataRequest,
    DeleteResourceConfigReply, DeleteResourceConfigRequest, DeleteSchemaReply, DeleteSchemaRequest,
    ExistsIdempotentDataReply, ExistsIdempotentDataRequest, GetOffsetDataReply,
    GetOffsetDataRequest, GetResourceConfigReply, GetResourceConfigRequest, HeartbeatReply,
    HeartbeatRequest, ListBindSchemaReply, ListBindSchemaRequest, ListSchemaReply,
    ListSchemaRequest, NodeListReply, NodeListRequest, RegisterNodeReply, RegisterNodeRequest,
    SaveOffsetDataReply, SaveOffsetDataRequest, SetIdempotentDataReply, SetIdempotentDataRequest,
    SetResourceConfigReply, SetResourceConfigRequest, UnBindSchemaReply, UnBindSchemaRequest,
    UnRegisterNodeReply, UnRegisterNodeRequest, UpdateSchemaReply, UpdateSchemaRequest,
};

use crate::pool::ClientPool;

macro_rules! generate_placement_service_call {
    ($fn_name:ident, $req_ty:ty, $rep_ty:ty, $variant:ident) => {
        pub async fn $fn_name(
            client_pool: &ClientPool,
            addrs: &[impl AsRef<str>],
            request: $req_ty,
        ) -> Result<$rep_ty, CommonError> {
            $crate::utils::retry_call(client_pool, addrs, request).await
        }
    };
}

generate_placement_service_call!(
    cluster_status,
    ClusterStatusRequest,
    ClusterStatusReply,
    ClusterStatus
);
generate_placement_service_call!(node_list, NodeListRequest, NodeListReply, ListNode);
generate_placement_service_call!(
    register_node,
    RegisterNodeRequest,
    RegisterNodeReply,
    RegisterNode
);
generate_placement_service_call!(
    unregister_node,
    UnRegisterNodeRequest,
    UnRegisterNodeReply,
    UnRegisterNode
);
generate_placement_service_call!(heartbeat, HeartbeatRequest, HeartbeatReply, Heartbeat);

generate_placement_service_call!(
    set_resource_config,
    SetResourceConfigRequest,
    SetResourceConfigReply,
    SetResourceConfig
);
generate_placement_service_call!(
    delete_resource_config,
    DeleteResourceConfigRequest,
    DeleteResourceConfigReply,
    DeleteResourceConfig
);
generate_placement_service_call!(
    get_resource_config,
    GetResourceConfigRequest,
    GetResourceConfigReply,
    GetResourceConfig
);
generate_placement_service_call!(
    set_idempotent_data,
    SetIdempotentDataRequest,
    SetIdempotentDataReply,
    SetIdempotentData
);
generate_placement_service_call!(
    delete_idempotent_data,
    DeleteIdempotentDataRequest,
    DeleteIdempotentDataReply,
    DeleteIdempotentData
);
generate_placement_service_call!(
    exists_idempotent_data,
    ExistsIdempotentDataRequest,
    ExistsIdempotentDataReply,
    ExistsIdempotentData
);

generate_placement_service_call!(
    save_offset_data,
    SaveOffsetDataRequest,
    SaveOffsetDataReply,
    SaveOffsetData
);

generate_placement_service_call!(list_schema, ListSchemaRequest, ListSchemaReply, ListSchema);

generate_placement_service_call!(
    create_schema,
    CreateSchemaRequest,
    CreateSchemaReply,
    CreateSchema
);

generate_placement_service_call!(
    update_schema,
    UpdateSchemaRequest,
    UpdateSchemaReply,
    UpdateSchema
);

generate_placement_service_call!(
    delete_schema,
    DeleteSchemaRequest,
    DeleteSchemaReply,
    DeleteSchema
);

generate_placement_service_call!(
    list_bind_schema,
    ListBindSchemaRequest,
    ListBindSchemaReply,
    ListBindSchema
);

generate_placement_service_call!(bind_schema, BindSchemaRequest, BindSchemaReply, BindSchema);

generate_placement_service_call!(
    un_bind_schema,
    UnBindSchemaRequest,
    UnBindSchemaReply,
    UnBindSchema
);

generate_placement_service_call!(
    get_offset_data,
    GetOffsetDataRequest,
    GetOffsetDataReply,
    GetOffsetData
);
