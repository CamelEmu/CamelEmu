typed_instructions! {
    op_subgroup_ballot_khr(predicate) => SubgroupBallotKHR;
    op_subgroup_read_invocation_khr(value, index) => SubgroupReadInvocationKHR;
    op_subgroup_all_khr(predicate) => SubgroupAllKHR;
    op_subgroup_any_khr(predicate) => SubgroupAnyKHR;
    op_subgroup_all_equal_khr(predicate) => SubgroupAllEqualKHR;
    op_group_non_uniform_elect(scope) => GroupNonUniformElect;
    op_group_non_uniform_broadcast_first(scope, value) => GroupNonUniformBroadcastFirst;
    op_group_non_uniform_broadcast(scope, value, id) => GroupNonUniformBroadcast;
    op_group_non_uniform_shuffle(scope, value, id) => GroupNonUniformShuffle;
    op_group_non_uniform_shuffle_xor(scope, value, mask) => GroupNonUniformShuffleXor;
    op_group_non_uniform_all(scope, predicate) => GroupNonUniformAll;
    op_group_non_uniform_any(scope, predicate) => GroupNonUniformAny;
    op_group_non_uniform_all_equal(scope, value) => GroupNonUniformAllEqual;
    op_group_non_uniform_ballot(scope, predicate) => GroupNonUniformBallot;
}
