typed_instructions! {
    op_shift_right_logical(base, shift) => ShiftRightLogical;
    op_shift_right_arithmetic(base, shift) => ShiftRightArithmetic;
    op_shift_left_logical(base, shift) => ShiftLeftLogical;
    op_bitwise_or(operand_1, operand_2) => BitwiseOr;
    op_bitwise_xor(operand_1, operand_2) => BitwiseXor;
    op_bitwise_and(operand_1, operand_2) => BitwiseAnd;
    op_not(operand) => Not;
    op_bit_field_insert(base, insert, offset, count) => BitFieldInsert;
    op_bit_field_s_extract(base, offset, count) => BitFieldSExtract;
    op_bit_field_u_extract(base, offset, count) => BitFieldUExtract;
    op_bit_reverse(base) => BitReverse;
    op_bit_count(base) => BitCount;
}
