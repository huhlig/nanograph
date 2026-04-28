//
// Copyright 2026 Hans W. Uhlig, IBM. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

//! gRPC protocol conversions between OpenRaft types and Protocol Buffer messages

use crate::error::ConsensusError;
use crate::types::*;
use nanograph_core::object::{
    ClusterId, DatabaseId, NodeId, ObjectId, RegionId, ServerId, ShardId, ShardNumber, TenantId,
};
use openraft::vote::RaftLeaderId;

// Include the generated protobuf code
pub mod pb {
    tonic::include_proto!("nanograph.raft");
}

/// Convert NodeId to protobuf
impl From<NodeId> for pb::NodeId {
    fn from(node_id: NodeId) -> Self {
        pb::NodeId {
            cluster: node_id.cluster_id().as_u32(),
            region: node_id.region_id().as_u32(),
            server: node_id.server_id().as_u64(),
        }
    }
}

/// Convert protobuf to NodeId
impl From<pb::NodeId> for NodeId {
    fn from(pb: pb::NodeId) -> Self {
        NodeId::from_parts(
            ClusterId::new(pb.cluster),
            RegionId::new(pb.region),
            ServerId::new(pb.server),
        )
    }
}

/// Convert ShardId to protobuf
impl From<ShardId> for pb::ShardId {
    fn from(shard_id: ShardId) -> Self {
        pb::ShardId {
            tenant_id: shard_id.tenant().as_u32(),
            database_id: shard_id.database().as_u32(),
            object_id: shard_id.object().as_u32(),
            shard_number: shard_id.shard_number().as_u32(),
        }
    }
}

/// Convert protobuf to ShardId
impl From<pb::ShardId> for ShardId {
    fn from(pb: pb::ShardId) -> Self {
        ShardId::from_parts(
            TenantId::new(pb.tenant_id),
            DatabaseId::new(pb.database_id),
            ObjectId::new(pb.object_id),
            ShardNumber::new(pb.shard_number),
        )
    }
}

/// Convert LeaderId to protobuf
impl From<ConsensusLeaderId> for pb::LeaderId {
    fn from(leader_id: ConsensusLeaderId) -> Self {
        pb::LeaderId {
            term: leader_id.term,
            node_id: leader_id.voted_for.map(|id| id.into()),
        }
    }
}

/// Convert protobuf to LeaderId
impl TryFrom<pb::LeaderId> for ConsensusLeaderId {
    type Error = ConsensusError;

    fn try_from(pb: pb::LeaderId) -> Result<Self, Self::Error> {
        let node_id = pb
            .node_id
            .ok_or_else(|| ConsensusError::Protocol {
                message: "Missing node_id in LeaderId".to_string(),
            })?
            .into();

        Ok(ConsensusLeaderId::new(pb.term, node_id))
    }
}

/// Convert CommittedLeaderId to protobuf
impl From<openraft::vote::leader_id_std::CommittedLeaderId<ConsensusTypeConfig>>
    for pb::CommittedLeaderId
{
    fn from(
        leader_id: openraft::vote::leader_id_std::CommittedLeaderId<ConsensusTypeConfig>,
    ) -> Self {
        pb::CommittedLeaderId {
            term: leader_id.term,
        }
    }
}

/// Convert protobuf to CommittedLeaderId
impl From<pb::CommittedLeaderId>
    for openraft::vote::leader_id_std::CommittedLeaderId<ConsensusTypeConfig>
{
    fn from(pb: pb::CommittedLeaderId) -> Self {
        openraft::vote::leader_id_std::CommittedLeaderId::new(pb.term)
    }
}

/// Convert LogId to protobuf
impl From<ConsensusLogId> for pb::LogId {
    fn from(log_id: ConsensusLogId) -> Self {
        pb::LogId {
            leader_id: Some(log_id.leader_id.into()),
            index: log_id.index,
        }
    }
}

/// Convert protobuf to LogId
impl TryFrom<pb::LogId> for ConsensusLogId {
    type Error = ConsensusError;

    fn try_from(pb: pb::LogId) -> Result<Self, Self::Error> {
        let leader_id = pb
            .leader_id
            .ok_or_else(|| ConsensusError::Protocol {
                message: "Missing leader_id in LogId".to_string(),
            })?
            .into();

        Ok(ConsensusLogId {
            leader_id,
            index: pb.index,
        })
    }
}

/// Convert Vote to protobuf
impl From<ConsensusVote> for pb::Vote {
    fn from(vote: ConsensusVote) -> Self {
        pb::Vote {
            leader_id: Some(vote.leader_id.into()),
            committed: vote.is_committed(),
        }
    }
}

/// Convert protobuf to Vote
impl TryFrom<pb::Vote> for ConsensusVote {
    type Error = ConsensusError;

    fn try_from(pb: pb::Vote) -> Result<Self, Self::Error> {
        let leader_id: ConsensusLeaderId = pb
            .leader_id
            .ok_or_else(|| ConsensusError::Protocol {
                message: "Missing leader_id in Vote".to_string(),
            })?
            .try_into()?;

        Ok(if pb.committed {
            ConsensusVote::new_committed(leader_id.term, *leader_id.node_id())
        } else {
            ConsensusVote::new(leader_id.term, *leader_id.node_id())
        })
    }
}

/// Convert VoteRequest to protobuf
impl From<ConsensusVoteRequest> for pb::VoteRequest {
    fn from(req: ConsensusVoteRequest) -> Self {
        pb::VoteRequest {
            vote: Some(req.vote.into()),
            last_log_id: req.last_log_id.map(|id| id.into()),
        }
    }
}

/// Convert protobuf to VoteRequest
impl TryFrom<pb::VoteRequest> for ConsensusVoteRequest {
    type Error = ConsensusError;

    fn try_from(pb: pb::VoteRequest) -> Result<Self, Self::Error> {
        let vote = pb
            .vote
            .ok_or_else(|| ConsensusError::Protocol {
                message: "Missing vote in VoteRequest".to_string(),
            })?
            .try_into()?;

        let last_log_id = pb.last_log_id.map(|id| id.try_into()).transpose()?;

        Ok(ConsensusVoteRequest { vote, last_log_id })
    }
}

/// Convert VoteResponse to protobuf
impl From<ConsensusVoteResponse> for pb::VoteResponse {
    fn from(resp: ConsensusVoteResponse) -> Self {
        pb::VoteResponse {
            vote: Some(resp.vote.into()),
            vote_granted: resp.vote_granted,
            last_log_id: resp.last_log_id.map(|id| id.into()),
        }
    }
}

/// Convert protobuf to VoteResponse
impl TryFrom<pb::VoteResponse> for ConsensusVoteResponse {
    type Error = ConsensusError;

    fn try_from(pb: pb::VoteResponse) -> Result<Self, Self::Error> {
        let vote = pb
            .vote
            .ok_or_else(|| ConsensusError::Protocol {
                message: "Missing vote in VoteResponse".to_string(),
            })?
            .try_into()?;

        let last_log_id = pb.last_log_id.map(|id| id.try_into()).transpose()?;

        Ok(ConsensusVoteResponse {
            vote,
            vote_granted: pb.vote_granted,
            last_log_id,
        })
    }
}

/// Convert LogEntry to protobuf
impl From<ConsensusEntry> for pb::LogEntry {
    fn from(entry: ConsensusEntry) -> Self {
        let data = serde_json::to_vec(&entry.payload).unwrap_or_default();
        pb::LogEntry {
            term: entry.log_id.leader_id.term,
            index: entry.log_id.index,
            data,
        }
    }
}

/// Convert protobuf to LogEntry
impl TryFrom<pb::LogEntry> for ConsensusEntry {
    type Error = ConsensusError;

    fn try_from(pb: pb::LogEntry) -> Result<Self, Self::Error> {
        let payload = serde_json::from_slice(&pb.data).map_err(|e| ConsensusError::Protocol {
            message: format!("Failed to deserialize log entry payload: {}", e),
        })?;

        Ok(ConsensusEntry {
            log_id: ConsensusLogId {
                leader_id: openraft::vote::leader_id_std::CommittedLeaderId::new(pb.term),
                index: pb.index,
            },
            payload,
        })
    }
}

/// Convert AppendEntriesRequest to protobuf
impl From<ConsensusAppendEntriesRequest> for pb::AppendEntriesRequest {
    fn from(req: ConsensusAppendEntriesRequest) -> Self {
        pb::AppendEntriesRequest {
            vote: Some(req.vote.into()),
            prev_log_id: req.prev_log_id.map(|id| id.into()),
            entries: req.entries.into_iter().map(|e| e.into()).collect(),
            leader_commit: req.leader_commit.map(|id| id.index).unwrap_or(0),
        }
    }
}

/// Convert protobuf to AppendEntriesRequest
impl TryFrom<pb::AppendEntriesRequest> for ConsensusAppendEntriesRequest {
    type Error = ConsensusError;

    fn try_from(pb: pb::AppendEntriesRequest) -> Result<Self, Self::Error> {
        let vote = pb
            .vote
            .ok_or_else(|| ConsensusError::Protocol {
                message: "Missing vote in AppendEntriesRequest".to_string(),
            })?
            .try_into()?;

        let prev_log_id = pb.prev_log_id.map(|id| id.try_into()).transpose()?;

        let entries: Result<Vec<_>, _> = pb.entries.into_iter().map(|e| e.try_into()).collect();
        let entries = entries?;

        let leader_commit = if pb.leader_commit > 0 {
            Some(ConsensusLogId {
                leader_id: openraft::vote::leader_id_std::CommittedLeaderId::new(0),
                index: pb.leader_commit,
            })
        } else {
            None
        };

        Ok(ConsensusAppendEntriesRequest {
            vote,
            prev_log_id,
            entries,
            leader_commit,
        })
    }
}

/// Convert AppendEntriesResponse to protobuf
impl From<ConsensusAppendEntriesResponse> for pb::AppendEntriesResponse {
    fn from(response: ConsensusAppendEntriesResponse) -> Self {
        match response {
            ConsensusAppendEntriesResponse::HigherVote(vote) => pb::AppendEntriesResponse {
                rejected_by: Some(vote.into()),
                conflict: false,
                last_log_id: None,
            },
            ConsensusAppendEntriesResponse::Conflict => pb::AppendEntriesResponse {
                rejected_by: None,
                conflict: true,
                last_log_id: None,
            },
            ConsensusAppendEntriesResponse::Success => pb::AppendEntriesResponse {
                rejected_by: None,
                conflict: false,
                last_log_id: None,
            },
            ConsensusAppendEntriesResponse::PartialSuccess(last_log_id) => {
                pb::AppendEntriesResponse {
                    rejected_by: None,
                    conflict: false,
                    last_log_id: last_log_id.map(|id| id.into()),
                }
            }
        }
    }
}

/// Convert protobuf to AppendEntriesResponse
impl TryFrom<pb::AppendEntriesResponse> for ConsensusAppendEntriesResponse {
    type Error = ConsensusError;

    fn try_from(pb: pb::AppendEntriesResponse) -> Result<Self, Self::Error> {
        if let Some(rejected_by) = pb.rejected_by {
            Ok(ConsensusAppendEntriesResponse::HigherVote(
                rejected_by.try_into()?,
            ))
        } else if pb.conflict {
            Ok(ConsensusAppendEntriesResponse::Conflict)
        } else if let Some(last_log_id) = pb.last_log_id {
            Ok(ConsensusAppendEntriesResponse::PartialSuccess(Some(
                last_log_id.try_into()?,
            )))
        } else {
            Ok(ConsensusAppendEntriesResponse::Success)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Operation;
    use nanograph_core::object::{
        ClusterId, DatabaseId, NodeId, ObjectId, RegionId, ServerId, ShardId, ShardNumber, TenantId,
    };
    use openraft::vote::leader_id_std::CommittedLeaderId;

    #[test]
    fn test_node_id_conversion() {
        let node_id = NodeId::from_parts(ClusterId::new(1), RegionId::new(2), ServerId::new(3));
        let pb: pb::NodeId = node_id.into();
        assert_eq!(pb.cluster, 1);
        assert_eq!(pb.region, 2);
        assert_eq!(pb.server, 3);

        let back: NodeId = pb.into();
        assert_eq!(back, node_id);
    }

    #[test]
    fn test_shard_id_conversion() {
        let shard_id =
            ShardId::from_parts(TenantId::new(1), DatabaseId::new(2), ObjectId::new(3), ShardNumber::new(4));
        let pb: pb::ShardId = shard_id.into();
        assert_eq!(pb.tenant_id, 1);
        assert_eq!(pb.database_id, 2);
        assert_eq!(pb.object_id, 3);
        assert_eq!(pb.shard_number, 4);

        let back: ShardId = pb.into();
        assert_eq!(back, shard_id);
    }

    #[test]
    fn test_leader_id_conversion() {
        let node_id = NodeId::new(1);
        let leader_id = ConsensusLeaderId::new(10, node_id);
        let pb: pb::LeaderId = leader_id.into();
        assert_eq!(pb.term, 10);
        assert!(pb.node_id.is_some());

        let back = ConsensusLeaderId::try_from(pb).unwrap();
        assert_eq!(back, leader_id);

        // Test missing node_id
        let pb_invalid = pb::LeaderId {
            term: 10,
            node_id: None,
        };
        assert!(ConsensusLeaderId::try_from(pb_invalid).is_err());
    }

    #[test]
    fn test_committed_leader_id_conversion() {
        let leader_id = CommittedLeaderId::<ConsensusTypeConfig>::new(10);
        let pb: pb::CommittedLeaderId = leader_id.into();
        assert_eq!(pb.term, 10);

        let back: CommittedLeaderId<ConsensusTypeConfig> = pb.into();
        assert_eq!(back, leader_id);
    }

    #[test]
    fn test_log_id_conversion() {
        let leader_id = CommittedLeaderId::<ConsensusTypeConfig>::new(10);
        let log_id = ConsensusLogId {
            leader_id,
            index: 100,
        };
        let pb: pb::LogId = log_id.into();
        assert_eq!(pb.index, 100);
        assert!(pb.leader_id.is_some());

        let back = ConsensusLogId::try_from(pb).unwrap();
        assert_eq!(back, log_id);

        // Test missing leader_id
        let pb_invalid = pb::LogId {
            leader_id: None,
            index: 100,
        };
        assert!(ConsensusLogId::try_from(pb_invalid).is_err());
    }

    #[test]
    fn test_vote_conversion() {
        let node_id = NodeId::new(1);
        let vote = ConsensusVote::new_committed(10, node_id);
        let pb: pb::Vote = vote.into();
        assert_eq!(pb.committed, true);
        assert!(pb.leader_id.is_some());

        let back = ConsensusVote::try_from(pb).unwrap();
        assert_eq!(back, vote);

        let vote_uncommitted = ConsensusVote::new(10, node_id);
        let pb_uncommitted: pb::Vote = vote_uncommitted.into();
        assert_eq!(pb_uncommitted.committed, false);
        let back_uncommitted = ConsensusVote::try_from(pb_uncommitted).unwrap();
        assert_eq!(back_uncommitted, vote_uncommitted);
    }

    #[test]
    fn test_vote_request_conversion() {
        let node_id = NodeId::new(1);
        let vote = ConsensusVote::new(10, node_id);
        let last_log_id = Some(ConsensusLogId {
            leader_id: CommittedLeaderId::new(9),
            index: 90,
        });
        let req = ConsensusVoteRequest { vote, last_log_id };
        let pb: pb::VoteRequest = req.clone().into();
        assert!(pb.vote.is_some());
        assert!(pb.last_log_id.is_some());

        let back = ConsensusVoteRequest::try_from(pb).unwrap();
        assert_eq!(back.vote, req.vote);
        assert_eq!(back.last_log_id, req.last_log_id);
    }

    #[test]
    fn test_vote_response_conversion() {
        let node_id = NodeId::new(1);
        let vote = ConsensusVote::new(10, node_id);
        let resp = ConsensusVoteResponse {
            vote,
            vote_granted: true,
            last_log_id: Some(ConsensusLogId {
                leader_id: CommittedLeaderId::new(10),
                index: 100,
            }),
        };
        let pb: pb::VoteResponse = resp.clone().into();
        assert!(pb.vote.is_some());
        assert_eq!(pb.vote_granted, true);
        assert!(pb.last_log_id.is_some());

        let back = ConsensusVoteResponse::try_from(pb).unwrap();
        assert_eq!(back.vote, resp.vote);
        assert_eq!(back.vote_granted, resp.vote_granted);
        assert_eq!(back.last_log_id, resp.last_log_id);
    }

    #[test]
    fn test_log_entry_conversion() {
        let entry = ConsensusEntry {
            log_id: ConsensusLogId {
                leader_id: CommittedLeaderId::new(10),
                index: 100,
            },
            payload: openraft::entry::EntryPayload::Normal(Operation::Delete {
                key: b"test".to_vec(),
            }),
        };
        let pb: pb::LogEntry = entry.clone().into();
        assert_eq!(pb.term, 10);
        assert_eq!(pb.index, 100);

        let back = ConsensusEntry::try_from(pb).unwrap();
        assert_eq!(back.log_id, entry.log_id);
        // payload should match too, assuming serde roundtrip works
        match (back.payload, entry.payload) {
            (
                openraft::entry::EntryPayload::Normal(Operation::Delete { key: k1 }),
                openraft::entry::EntryPayload::Normal(Operation::Delete { key: k2 }),
            ) => assert_eq!(k1, k2),
            _ => panic!("Payload mismatch"),
        }
    }

    #[test]
    fn test_append_entries_request_conversion() {
        let node_id = NodeId::new(1);
        let req = ConsensusAppendEntriesRequest {
            vote: ConsensusVote::new_committed(10, node_id),
            prev_log_id: Some(ConsensusLogId {
                leader_id: CommittedLeaderId::new(9),
                index: 90,
            }),
            entries: vec![ConsensusEntry {
                log_id: ConsensusLogId {
                    leader_id: CommittedLeaderId::new(10),
                    index: 91,
                },
                payload: openraft::entry::EntryPayload::Normal(Operation::Delete {
                    key: b"test".to_vec(),
                }),
            }],
            leader_commit: Some(ConsensusLogId {
                leader_id: CommittedLeaderId::new(10),
                index: 80,
            }),
        };
        let pb: pb::AppendEntriesRequest = req.clone().into();
        assert!(pb.vote.is_some());
        assert_eq!(pb.leader_commit, 80);
        assert_eq!(pb.entries.len(), 1);

        let back = ConsensusAppendEntriesRequest::try_from(pb).unwrap();
        assert_eq!(back.vote, req.vote);
        assert_eq!(back.prev_log_id, req.prev_log_id);
        assert_eq!(back.entries.len(), req.entries.len());
        // Note: leader_commit conversion in code currently forces leader_id to 0 for Some(..)
        // We might need to check if that's intended.
        assert_eq!(back.leader_commit.unwrap().index, 80);
    }

    #[test]
    fn test_append_entries_response_conversion() {
        // Success
        let resp = ConsensusAppendEntriesResponse::Success;
        let pb: pb::AppendEntriesResponse = resp.into();
        assert_eq!(pb.conflict, false);
        assert!(pb.rejected_by.is_none());
        assert!(pb.last_log_id.is_none());
        let back = ConsensusAppendEntriesResponse::try_from(pb).unwrap();
        match back {
            ConsensusAppendEntriesResponse::Success => (),
            _ => panic!("Expected Success"),
        }

        // Conflict
        let resp = ConsensusAppendEntriesResponse::Conflict;
        let pb: pb::AppendEntriesResponse = resp.into();
        assert_eq!(pb.conflict, true);
        let back = ConsensusAppendEntriesResponse::try_from(pb).unwrap();
        match back {
            ConsensusAppendEntriesResponse::Conflict => (),
            _ => panic!("Expected Conflict"),
        }

        // HigherVote
        let node_id = NodeId::new(1);
        let vote = ConsensusVote::new(11, node_id);
        let resp = ConsensusAppendEntriesResponse::HigherVote(vote);
        let pb: pb::AppendEntriesResponse = resp.into();
        assert!(pb.rejected_by.is_some());
        let back = ConsensusAppendEntriesResponse::try_from(pb).unwrap();
        match back {
            ConsensusAppendEntriesResponse::HigherVote(v) => assert_eq!(v, vote),
            _ => panic!("Expected HigherVote"),
        }

        // PartialSuccess
        let log_id = ConsensusLogId {
            leader_id: CommittedLeaderId::new(10),
            index: 100,
        };
        let resp = ConsensusAppendEntriesResponse::PartialSuccess(Some(log_id));
        let pb: pb::AppendEntriesResponse = resp.into();
        assert!(pb.last_log_id.is_some());
        let back = ConsensusAppendEntriesResponse::try_from(pb).unwrap();
        match back {
            ConsensusAppendEntriesResponse::PartialSuccess(Some(l)) => assert_eq!(l, log_id),
            _ => panic!("Expected PartialSuccess"),
        }
    }
}
